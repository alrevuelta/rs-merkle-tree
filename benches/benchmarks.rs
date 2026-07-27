use criterion::black_box;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::random;
use rs_merkle_tree::stores::{FileStore, MemoryStore, RocksDbStore, SledStore, SqliteStore};
use rs_merkle_tree::{hasher::Keccak256Hasher, node::Node, tree::MerkleTree};

// Constants for the benchmarks
const BATCH_SIZE: u64 = 1000;
const NUM_BATCHES: u64 = 10;
const SAMPLE_SIZE: u64 = 10;

fn bench_insertions(c: &mut Criterion) {
    let mut group = c.benchmark_group("inserts");

    group
        .sample_size(SAMPLE_SIZE as usize)
        .warm_up_time(std::time::Duration::from_millis(500));

    // TODO: Add benchmarks for different batch sizes

    // TODO: Improve the cleanups.
    let _ = std::fs::remove_file("sqlite.db");
    let _ = std::fs::remove_dir_all("sled.db");
    let _ = std::fs::remove_file("rocksdb.db");
    let _ = std::fs::remove_dir_all("file.db");

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

    // Depth 32 benchmarks Keccak256
    group.throughput(Throughput::Elements((NUM_BATCHES * BATCH_SIZE) as u64));
    group.bench_function(BenchmarkId::new("memory_store", "depth32_keccak256"), |b| {
        b.iter(|| {
            for _ in 0..NUM_BATCHES {
                let leaves: Vec<Node> = (0..BATCH_SIZE)
                    .map(|_| black_box(Node::random()))
                    .collect::<Vec<Node>>();
                memory_tree.add_leaves(&leaves).unwrap();
            }
        });
    });
    group.throughput(Throughput::Elements((NUM_BATCHES * BATCH_SIZE) as u64));
    group.bench_function(BenchmarkId::new("sqlite_store", "depth32_keccak256"), |b| {
        b.iter(|| {
            for _ in 0..NUM_BATCHES {
                let leaves: Vec<Node> = (0..BATCH_SIZE)
                    .map(|_| black_box(Node::random()))
                    .collect::<Vec<Node>>();
                sqlite_tree.add_leaves(&leaves).unwrap();
            }
        });
    });
    group.throughput(Throughput::Elements((NUM_BATCHES * BATCH_SIZE) as u64));
    group.bench_function(BenchmarkId::new("sled_store", "depth32_keccak256"), |b| {
        b.iter(|| {
            for _ in 0..NUM_BATCHES {
                let leaves: Vec<Node> = (0..BATCH_SIZE)
                    .map(|_| black_box(Node::random()))
                    .collect::<Vec<Node>>();
                sled_tree.add_leaves(&leaves).unwrap();
            }
        });
    });
    group.throughput(Throughput::Elements((NUM_BATCHES * BATCH_SIZE) as u64));
    group.bench_function(
        BenchmarkId::new("rocksdb_store", "depth32_keccak256"),
        |b| {
            b.iter(|| {
                for _ in 0..NUM_BATCHES {
                    let leaves: Vec<Node> = (0..BATCH_SIZE)
                        .map(|_| black_box(Node::random()))
                        .collect::<Vec<Node>>();
                    rocksdb_tree.add_leaves(&leaves).unwrap();
                }
            });
        },
    );
    group.throughput(Throughput::Elements((NUM_BATCHES * BATCH_SIZE) as u64));
    group.bench_function(BenchmarkId::new("file_store", "depth32_keccak256"), |b| {
        b.iter(|| {
            for _ in 0..NUM_BATCHES {
                let leaves: Vec<Node> = (0..BATCH_SIZE)
                    .map(|_| black_box(Node::random()))
                    .collect::<Vec<Node>>();
                file_tree.add_leaves(&leaves).unwrap();
            }
        });
    });

    // Depth 32 benchmarks Poseidon
    // TODO: Benchmarks not working due to inputs being bigger than the prime
    /*
    group.throughput(Throughput::Elements((NUM_BATCHES * BATCH_SIZE) as u64));
    group.bench_function(BenchmarkId::new("memory_store", "depth32_poseidon"), |b| {
        bench_store::<PoseidonHasher, MemoryStore, 32, _, _>(
            b,
            || MemoryStore::new(),
            || PoseidonHasher,
        )
    });
    group.throughput(Throughput::Elements((NUM_BATCHES * BATCH_SIZE) as u64));
    group.bench_function(BenchmarkId::new("sqlite_store", "depth32_poseidon"), |b| {
        let _ = std::fs::remove_file("sqlite.db");
        bench_store::<PoseidonHasher, SqliteStore, 32, _, _>(
            b,
            || SqliteStore::new("sqlite.db"),
            || PoseidonHasher,
        )
    });
    group.throughput(Throughput::Elements((NUM_BATCHES * BATCH_SIZE) as u64));
    group.bench_function(BenchmarkId::new("sled_store", "depth32_poseidon"), |b| {
        let _ = std::fs::remove_dir_all("sled.db");
        bench_store::<PoseidonHasher, SledStore, 32, _, _>(
            b,
            || SledStore::new("sled.db", false),
            || PoseidonHasher,
        )
    });
    group.throughput(Throughput::Elements((NUM_BATCHES * BATCH_SIZE) as u64));
    group.bench_function(BenchmarkId::new("rocksdb_store", "depth32_poseidon"), |b| {
        let _ = std::fs::remove_file("rocksdb.db");
        bench_store::<PoseidonHasher, RocksDbStore, 32, _, _>(
            b,
            || RocksDbStore::new("rocksdb.db"),
            || PoseidonHasher,
        )
    });
     */

    // Cleanup
    let _ = std::fs::remove_file("sqlite.db");
    let _ = std::fs::remove_dir_all("sled.db");
    let _ = std::fs::remove_file("rocksdb.db");
    let _ = std::fs::remove_dir_all("file.db");

    group.finish();
}

fn bench_get_proof(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_proof");

    group
        .sample_size(SAMPLE_SIZE as usize)
        .warm_up_time(std::time::Duration::from_millis(500));

    // TODO: Improve the cleanups.
    let _ = std::fs::remove_file("sqlite.db");
    let _ = std::fs::remove_dir_all("sled.db");
    let _ = std::fs::remove_file("rocksdb.db");
    let _ = std::fs::remove_dir_all("file.db");

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
    let _ = std::fs::remove_file("sqlite.db");
    let _ = std::fs::remove_dir_all("sled.db");
    let _ = std::fs::remove_file("rocksdb.db");
    let _ = std::fs::remove_dir_all("file.db");

    group.finish();
}

criterion_group!(benches, bench_insertions, bench_get_proof);
criterion_main!(benches);
