// Copyright 2025 Bilinear Labs - MIT License

//! Insertion benchmark for every store backend.
//!
//! This deliberately does not use Criterion. Criterion times a closure it may
//! call any number of times and treats the results as independent samples of a
//! stationary distribution. Inserting into a Merkle tree satisfies neither
//! condition: the tree only ever grows, so the work per call changes as the run
//! progresses, and the iteration count Criterion derives from its wall-clock
//! budget makes the amount of data inserted a function of how fast the machine
//! is, which is exactly what a benchmark must not depend on.

//! TODO: Poseidon is not covered yet, leaves hash to values above the field prime.

use rs_merkle_tree::hasher::Keccak256Hasher;
use rs_merkle_tree::stores::{FileStore, MemoryStore, RocksDbStore, SledStore, SqliteStore};
use rs_merkle_tree::{node::Node, tree::MerkleTree, Store};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use std::{fs, io};

const DEPTH: usize = 32;
const HASH: &str = "keccak256";

/// Root holding every store the benchmark writes. Removed before and after a run.
const ROOT: &str = "bench_stores";

/// Leaves inserted into every store. Fixed rather than configurable so that two
/// runs, on any machine, always describe the same workload.
const LEAVES: u64 = 1_000_000;

const DEFAULT_BATCH: u64 = 1_000;

/// Equal slices the run is cut into for the per-slice throughput reported in
/// JSON. A decreasing series is how a store that degrades as it fills shows up.
const SLICES: usize = 10;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

const USAGE: &str = "\
Usage: insertions [options]

Inserts a fixed number of leaves into every store and reports the throughput,
the per-batch latency distribution, and the resulting size on disk.

Options:
  --batch <n>     Leaves per add_leaves call, must divide 1000000 (default 1000)
  --format table  Markdown table, the default
  --format json   Full reports as JSON, including per-slice throughput
  --store <name>  Measure only this store. Used internally to give every
                  store a process of its own.
  -h, --help      Show this message
";

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = Args::parse(std::env::args().skip(1))?;

    // A single store, which is also how the parent invokes itself per store.
    if let Some(kind) = args.store {
        return emit(&[measure(kind, args.batch)?], &args.format);
    }

    remove(Path::new(ROOT))?;

    let mut reports = Vec::with_capacity(StoreKind::ALL.len());
    for kind in StoreKind::ALL {
        eprintln!(
            "measuring {} store, {LEAVES} leaves in batches of {}",
            kind.name(),
            args.batch
        );
        reports.extend(measure_in_child(kind, &args)?);
    }

    remove(Path::new(ROOT))?;

    reports.sort_by(|a, b| b.leaves_per_sec.total_cmp(&a.leaves_per_sec));
    emit(&reports, &args.format)
}

fn emit(reports: &[Report], format: &Format) -> Result<()> {
    match format {
        Format::Table => print!("{}", table(reports)),
        Format::Json => println!("{}", serde_json::to_string_pretty(reports)?),
    }
    Ok(())
}

/// Measures one store in a child process.
///
/// Stores must not share a process. Background compaction and page cache
/// writeback caused by one store carry on into the measurement of the next one,
/// which is enough to make a store report several times less throughput than it
/// actually sustains.
fn measure_in_child(kind: StoreKind, args: &Args) -> Result<Vec<Report>> {
    let output = Command::new(std::env::current_exe()?)
        .args(["--store", kind.name()])
        .args(["--batch", &args.batch.to_string()])
        .args(["--format", "json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?
        .wait_with_output()?;

    if !output.status.success() {
        return Err(format!("{} store failed: {}", kind.name(), output.status).into());
    }

    Ok(serde_json::from_slice(&output.stdout)?)
}

fn measure(kind: StoreKind, batch: u64) -> Result<Report> {
    let Some(dir) = kind.dir() else {
        return insert(kind, MemoryStore::default(), batch);
    };

    remove(&dir)?;
    fs::create_dir_all(&dir)?;
    let path = dir
        .join("db")
        .to_str()
        .map(str::to_owned)
        .ok_or("non-utf8 store path")?;

    let report = match kind {
        StoreKind::Sqlite => insert(kind, SqliteStore::new(&path), batch),
        StoreKind::Sled => insert(kind, SledStore::new(&path, false), batch),
        StoreKind::RocksDb => insert(kind, RocksDbStore::new(&path), batch),
        StoreKind::File => insert(kind, FileStore::new(&path), batch),
        StoreKind::Memory => unreachable!("the in-memory store has no directory"),
    };

    // Removed as soon as it has been measured, so that a run needs room for one
    // store at a time and leaves nothing behind when invoked for a single store.
    remove(&dir)?;
    report
}

fn insert<S: Store>(kind: StoreKind, store: S, batch: u64) -> Result<Report> {
    let mut tree: MerkleTree<Keccak256Hasher, S, DEPTH> = MerkleTree::new(Keccak256Hasher, store);
    let batches = LEAVES / batch;
    let mut latencies = Vec::with_capacity(batches as usize);

    for _ in 0..batches {
        // Generated outside the timed section. The subject is the store, not
        // the random number generator.
        let nodes: Vec<Node> = (0..batch).map(|_| Node::random()).collect();

        let start = Instant::now();
        tree.add_leaves(&nodes)?;
        latencies.push(start.elapsed());
    }

    assert_eq!(tree.num_leaves(), LEAVES, "store lost leaves");

    // Closed before its size is read, so what is measured is what the store
    // decided to keep rather than whatever happened to be written so far.
    drop(tree);

    let inserting: Duration = latencies.iter().sum();
    let disk_bytes = match kind.dir() {
        Some(dir) => disk_usage(&dir)?,
        None => 0,
    };
    let slices = latencies
        .chunks((batches as usize).div_ceil(SLICES))
        .map(|slice| rate(batch * slice.len() as u64, slice.iter().sum()))
        .collect();

    latencies.sort_unstable();
    Ok(Report {
        store: kind.name().to_owned(),
        depth: DEPTH,
        hash: HASH.to_owned(),
        leaves: LEAVES,
        batch,
        inserting_secs: inserting.as_secs_f64(),
        leaves_per_sec: rate(LEAVES, inserting),
        p50_ms: millis(percentile(&latencies, 50.0)),
        p99_ms: millis(percentile(&latencies, 99.0)),
        max_ms: millis(*latencies.last().expect("at least one batch")),
        disk_bytes,
        slice_leaves_per_sec: slices,
    })
}

/// Result of inserting `leaves` leaves into one store.
#[derive(Serialize, Deserialize)]
struct Report {
    store: String,
    depth: usize,
    hash: String,
    leaves: u64,
    batch: u64,
    /// Time inside `add_leaves`, excluding leaf generation.
    inserting_secs: f64,
    /// Leaves per second, from the time spent inside `add_leaves`.
    leaves_per_sec: f64,
    p50_ms: f64,
    p99_ms: f64,
    max_ms: f64,
    /// Bytes on disk once the store is closed, zero for in-memory.
    disk_bytes: u64,
    /// Throughput of each equal slice of the run, in leaves per second.
    slice_leaves_per_sec: Vec<f64>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StoreKind {
    Memory,
    Sqlite,
    Sled,
    RocksDb,
    File,
}

impl StoreKind {
    const ALL: [Self; 5] = [
        Self::Memory,
        Self::Sqlite,
        Self::Sled,
        Self::RocksDb,
        Self::File,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Memory => "memory",
            Self::Sqlite => "sqlite",
            Self::Sled => "sled",
            Self::RocksDb => "rocksdb",
            Self::File => "file",
        }
    }

    fn parse(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.name() == name)
            .ok_or_else(|| format!("unknown store {name:?}").into())
    }

    /// Directory holding everything the store writes, `None` when in-memory.
    ///
    /// One directory per store keeps sibling files, such as the SQLite WAL, in
    /// scope for both cleanup and the size measurement.
    fn dir(self) -> Option<PathBuf> {
        match self {
            Self::Memory => None,
            _ => Some(Path::new(ROOT).join(self.name())),
        }
    }
}

struct Args {
    store: Option<StoreKind>,
    batch: u64,
    format: Format,
}

enum Format {
    Table,
    Json,
}

impl Args {
    fn parse(argv: impl Iterator<Item = String>) -> Result<Self> {
        let mut args = Self {
            store: None,
            batch: DEFAULT_BATCH,
            format: Format::Table,
        };

        let mut argv = argv.peekable();
        while let Some(flag) = argv.next() {
            let mut value = || -> Result<String> {
                argv.next()
                    .ok_or_else(|| format!("{flag} needs a value").into())
            };

            match flag.as_str() {
                "--store" => args.store = Some(StoreKind::parse(&value()?)?),
                "--batch" => args.batch = value()?.parse()?,
                "--format" => {
                    args.format = match value()?.as_str() {
                        "table" => Format::Table,
                        "json" => Format::Json,
                        other => return Err(format!("unknown format {other:?}").into()),
                    }
                }
                // `cargo bench` passes this to every harness-less bench target.
                "--bench" => {}
                "-h" | "--help" => {
                    print!("{USAGE}");
                    std::process::exit(0);
                }
                other => return Err(format!("unrecognised argument {other:?}\n\n{USAGE}").into()),
            }
        }

        if args.batch == 0 || !LEAVES.is_multiple_of(args.batch) {
            return Err(format!("--batch {} does not divide {LEAVES}", args.batch).into());
        }
        Ok(args)
    }
}

fn table(reports: &[Report]) -> String {
    let mut out = String::from(
        "| Depth | Hash | Leaves | Batch | Store | Throughput | p50 batch | p99 batch | Disk |\n\
         |---|---|---|---|---|---|---|---|---|\n",
    );

    for report in reports {
        let disk = if report.disk_bytes == 0 {
            "-".to_owned()
        } else {
            bytes(report.disk_bytes)
        };

        writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            report.depth,
            report.hash,
            report.leaves,
            report.batch,
            report.store,
            elems_per_sec(report.leaves_per_sec),
            duration(report.p50_ms),
            duration(report.p99_ms),
            disk,
        )
        .expect("writing to a String cannot fail");
    }
    out
}

fn percentile(sorted: &[Duration], percent: f64) -> Duration {
    let last = sorted.len() - 1;
    sorted[(percent / 100.0 * last as f64).round() as usize]
}

fn rate(leaves: u64, elapsed: Duration) -> f64 {
    leaves as f64 / elapsed.as_secs_f64()
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

/// Formats a rate with an SI prefix, keeping the units Criterion used.
fn elems_per_sec(per_sec: f64) -> String {
    for (factor, unit) in [(1e9, "Gelem/s"), (1e6, "Melem/s"), (1e3, "Kelem/s")] {
        if per_sec >= factor {
            return format!("{:.3} {unit}", per_sec / factor);
        }
    }
    format!("{per_sec:.3} elem/s")
}

fn duration(millis: f64) -> String {
    if millis >= 1000.0 {
        format!("{:.3} s", millis / 1000.0)
    } else if millis >= 1.0 {
        format!("{millis:.3} ms")
    } else {
        format!("{:.3} µs", millis * 1000.0)
    }
}

fn bytes(count: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut value = count as f64;
    for unit in UNITS {
        if value < 1024.0 || unit == "GiB" {
            return format!("{value:.2} {unit}");
        }
        value /= 1024.0;
    }
    unreachable!("the loop returns on the last unit")
}

fn disk_usage(dir: &Path) -> io::Result<u64> {
    let mut total = 0;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        total += if metadata.is_dir() {
            disk_usage(&entry.path())?
        } else {
            metadata.len()
        };
    }
    Ok(total)
}

/// Removes a path whether it is a file or a directory, ignoring absence.
fn remove(path: &Path) -> io::Result<()> {
    let result = if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };

    match result {
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}
