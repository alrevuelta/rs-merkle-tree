//! Proof retrieval benchmarks.

use criterion::black_box;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use rand::random;
use rs_merkle_tree::stores::{FileStore, MemoryStore, RocksDbStore, SledStore, SqliteStore};
use rs_merkle_tree::{hasher::Keccak256Hasher, node::Node, tree::MerkleTree};

// Constants for the benchmarks
const BATCH_SIZE: u64 = 1000;
const NUM_BATCHES: u64 = 10;
const SAMPLE_SIZE: u64 = 10;

/// Everything the benchmarked stores write, including the sibling files SQLite
/// keeps next to the database. A stale WAL would be replayed into the new
/// database, so it has to go too.
const STORE_PATHS: [&str; 6] = [
    "sqlite.db",
    "sqlite.db-wal",
    "sqlite.db-shm",
    "sled.db",
    "rocksdb.db",
    "file.db",
];

/// Deletes every store, whether it is a file or a directory.
///
/// Leftover state is not a cosmetic problem: the stores persist their leaf
/// count, so a surviving database makes the next run resume from it and measure
/// insertions into an already large tree.
fn cleanup_stores() {
    for path in STORE_PATHS {
        let result = if std::path::Path::new(path).is_dir() {
            std::fs::remove_dir_all(path)
        } else {
            std::fs::remove_file(path)
        };

        match result {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => panic!("failed to remove {path}: {err}"),
        }
    }
}

fn bench_get_proof(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_proof");

    group
        .sample_size(SAMPLE_SIZE as usize)
        .warm_up_time(std::time::Duration::from_millis(500));

    cleanup_stores();

    let mut memory_tree: MerkleTree<Keccak256Hasher, MemoryStore, 32> =
        MerkleTree::new(Keccak256Hasher, MemoryStore::default());
    let mut sqlite_tree: MerkleTree<Keccak256Hasher, SqliteStore, 32> =
        MerkleTree::new(Keccak256Hasher, SqliteStore::new("sqlite.db"));
    let mut sled_tree: MerkleTree<Keccak256Hasher, SledStore, 32> =
        MerkleTree::new(Keccak256Hasher, SledStore::new("sled.db", false));
    let mut rocksdb_tree: MerkleTree<Keccak256Hasher, RocksDbStore, 32> =
        MerkleTree::new(Keccak256Hasher, RocksDbStore::new("rocksdb.db"));
    let mut file_tree: MerkleTree<Keccak256Hasher, FileStore, 32> =
        MerkleTree::new(Keccak256Hasher, FileStore::new("file.db"));

    for _ in 0..NUM_BATCHES {
        let leaves: Vec<Node> = (0..BATCH_SIZE)
            .map(|_| black_box(Node::random()))
            .collect::<Vec<Node>>();
        memory_tree.add_leaves(&leaves).unwrap();
        sqlite_tree.add_leaves(&leaves).unwrap();
        sled_tree.add_leaves(&leaves).unwrap();
        rocksdb_tree.add_leaves(&leaves).unwrap();
        file_tree.add_leaves(&leaves).unwrap();
    }

    group.bench_function(BenchmarkId::new("memory_store", "depth32_keccak256"), |b| {
        b.iter(|| {
            let i = random::<u64>() % (BATCH_SIZE * NUM_BATCHES);
            memory_tree.proof(i).unwrap();
        });
    });

    group.bench_function(BenchmarkId::new("sqlite_store", "depth32_keccak256"), |b| {
        b.iter(|| {
            let i = random::<u64>() % (BATCH_SIZE * NUM_BATCHES);
            sqlite_tree.proof(i).unwrap();
        });
    });
    group.bench_function(BenchmarkId::new("sled_store", "depth32_keccak256"), |b| {
        b.iter(|| {
            let i = random::<u64>() % (BATCH_SIZE * NUM_BATCHES);
            sled_tree.proof(i).unwrap();
        });
    });
    group.bench_function(
        BenchmarkId::new("rocksdb_store", "depth32_keccak256"),
        |b| {
            b.iter(|| {
                let i = random::<u64>() % (BATCH_SIZE * NUM_BATCHES);
                rocksdb_tree.proof(i).unwrap();
            });
        },
    );
    group.bench_function(BenchmarkId::new("file_store", "depth32_keccak256"), |b| {
        b.iter(|| {
            let i = random::<u64>() % (BATCH_SIZE * NUM_BATCHES);
            file_tree.proof(i).unwrap();
        });
    });

    // Cleanup
    cleanup_stores();

    group.finish();
}

criterion_group!(benches, bench_get_proof);
criterion_main!(benches);
