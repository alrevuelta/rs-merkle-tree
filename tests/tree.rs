// Copyright 2025 Bilinear Labs - MIT License

use rs_merkle_tree::hasher::Keccak256Hasher;
use rs_merkle_tree::{to_node, MerkleTree, Node, Store};
use std::fs;
use std::path::Path;

#[cfg(feature = "file_store")]
use rs_merkle_tree::stores::FileStore;
#[cfg(feature = "memory_store")]
use rs_merkle_tree::stores::MemoryStore;
#[cfg(feature = "rocksdb_store")]
use rs_merkle_tree::stores::RocksDbStore;
#[cfg(feature = "sled_store")]
use rs_merkle_tree::stores::SledStore;
#[cfg(feature = "sqlite_store")]
use rs_merkle_tree::stores::SqliteStore;
#[cfg(feature = "memory_store")]
use rs_merkle_tree::tree::MerkleTree32;

fn dir_size(path: &Path) -> u64 {
    if path.is_file() {
        return path.metadata().map(|m| m.len()).unwrap_or(0);
    }
    let mut size = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            size += dir_size(&entry.path());
        }
    }
    size
}

#[cfg(feature = "memory_store")]
#[test]
fn test_merkle_tree_keccak_32_memory() {
    let mut tree: MerkleTree32 = MerkleTree::new(Keccak256Hasher, MemoryStore::default());

    // create 10k leaves.
    let leaves = (0..10_000)
        .map(|i| to_node!(format!("0x{:064x}", i).as_str()))
        .collect::<Vec<Node>>();

    // create a tree with 10k leaves.
    for i in &leaves {
        tree.add_leaves(&[*i]).unwrap();
    }

    assert_eq!(tree.num_leaves(), 10_000);
    assert_eq!(
        tree.root().unwrap(),
        to_node!("0x532c79f3ea0f4873946d1b14770eaa1c157255a003e73da987b858cc287b0482")
    );

    // reset the tree.
    let mut tree: MerkleTree32 = MerkleTree::new(Keccak256Hasher, MemoryStore::default());

    // same but add them in batches of 1_000.
    for batch in leaves.chunks(1_000) {
        tree.add_leaves(&batch).unwrap();
    }

    assert_eq!(tree.num_leaves(), 10_000);
    assert_eq!(
        tree.root().unwrap(),
        to_node!("0x532c79f3ea0f4873946d1b14770eaa1c157255a003e73da987b858cc287b0482")
    );

    // Get proofs for each leaf and verify them.
    for i in 0..10_000 {
        let proof = tree.proof(i).unwrap();
        assert_eq!(proof.proof.len(), 32);
        assert_eq!(tree.verify_proof(&proof).unwrap(), true);
    }

    // TODO: Once async is implemented, ensure proofs are always consistent.
}

#[cfg(all(feature = "file_store", feature = "memory_store"))]
#[test]
fn test_merkle_tree_keccak_32_file() {
    let dir = std::env::temp_dir().join(format!("rs-merkle-tree-golden-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let path = dir.to_str().expect("utf-8 temp path").to_owned();

    let leaves = (0..10_001)
        .map(|i| to_node!(format!("0x{:064x}", i).as_str()))
        .collect::<Vec<Node>>();

    // The same golden root as the memory store, reached through the on-disk
    // layout. FileStore rejects any batch that is not one contiguous run per
    // level, so this also pins the shape `add_leaves` emits at all 33 levels.
    let mut tree: MerkleTree<Keccak256Hasher, FileStore, 32> =
        MerkleTree::new(Keccak256Hasher, FileStore::new(&path));
    for batch in leaves[..10_000].chunks(1_000) {
        tree.add_leaves(batch).unwrap();
    }

    assert_eq!(tree.num_leaves(), 10_000);
    assert_eq!(
        tree.root().unwrap(),
        to_node!("0x532c79f3ea0f4873946d1b14770eaa1c157255a003e73da987b858cc287b0482")
    );

    // Appending after a reopen is the only path that reads left partners back
    // from disk rather than from nodes this process just wrote.
    drop(tree);
    let mut tree: MerkleTree<Keccak256Hasher, FileStore, 32> =
        MerkleTree::new(Keccak256Hasher, FileStore::new(&path));
    assert_eq!(tree.num_leaves(), 10_000);
    tree.add_leaves(&leaves[10_000..]).unwrap();

    let mut expected: MerkleTree32 = MerkleTree::new(Keccak256Hasher, MemoryStore::default());
    expected.add_leaves(&leaves).unwrap();

    assert_eq!(tree.num_leaves(), 10_001);
    assert_eq!(tree.root().unwrap(), expected.root().unwrap());

    for i in [0, 1, 4_999, 9_999, 10_000] {
        let proof = tree.proof(i).unwrap();
        assert_eq!(proof.leaf, leaves[i as usize]);
        assert!(tree.verify_proof(&proof).unwrap());
    }

    fs::remove_dir_all(&dir).unwrap();
}

#[cfg(any(
    feature = "sled_store",
    feature = "sqlite_store",
    feature = "rocksdb_store",
    feature = "file_store"
))]
#[test]
#[ignore = "run it on demand, slow and takes some disk space"]
fn test_disk_space() {
    // Run as: cargo test --release test_disk_space -- --ignored --no-capture
    // Not a unit-test per-se; rather, it benchmarks the on-disk size of a depth-32
    // tree containing one million leaves for each backing store.

    // Insert NUM_BATCHES * BATCH_SIZE leaves.
    const NUM_BATCHES: usize = 1_000;
    const BATCH_SIZE: usize = 1_000;

    // Clean up any previous runs.
    ["sled.db", "sqlite.db", "rocksdb.db", "file.db"]
        .into_iter()
        .for_each(|p| {
            fs::remove_dir_all(p).ok();
            fs::remove_file(p).ok();
        });

    // Helper that creates the tree with a given store and inserts `NUM_BATCHES * BATCH_SIZE` leaves.
    fn bench_store<S, F>(db_name: &str, new_store: F)
    where
        S: Store,
        F: FnOnce() -> S,
    {
        let mut tree: MerkleTree<Keccak256Hasher, S, 32> =
            MerkleTree::new(Keccak256Hasher, new_store());

        for _ in 0..NUM_BATCHES {
            let leaves: Vec<Node> = (0..BATCH_SIZE).map(|_| Node::random()).collect();
            tree.add_leaves(&leaves).unwrap();
        }

        print_size(db_name, db_name, (NUM_BATCHES * BATCH_SIZE) as u64);
    }

    #[cfg(feature = "sled_store")]
    bench_store::<SledStore, _>("sled.db", || SledStore::new("sled.db", false));
    #[cfg(feature = "sqlite_store")]
    bench_store::<SqliteStore, _>("sqlite.db", || SqliteStore::new("sqlite.db"));
    #[cfg(feature = "rocksdb_store")]
    bench_store::<RocksDbStore, _>("rocksdb.db", || RocksDbStore::new("rocksdb.db"));
    #[cfg(feature = "file_store")]
    bench_store::<FileStore, _>("file.db", || FileStore::new("file.db"));
}

fn print_size(name: &str, file: &str, num_leaves: u64) {
    // Hardcoded depth for now.
    let depth = 32;

    let size_bytes = dir_size(Path::new(file));
    println!(
        "store {} depth {} num_leaves {} size: {:.2} MiB",
        name,
        depth,
        num_leaves,
        size_bytes as f64 / (1024.0 * 1024.0)
    );
    fs::remove_dir_all(file).ok();
}
