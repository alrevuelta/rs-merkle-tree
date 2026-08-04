// Copyright 2025 Bilinear Labs - MIT License

use rs_merkle_tree::{node::Node, to_node, Store};
#[cfg(any(feature = "rocksdb_store", feature = "file_store"))]
use std::fs;
use temp_file::TempFile;

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

// Builds a unique temporary directory path for the file store and removes the
// placeholder file so `FileStore::new` can create a directory there.
#[cfg(feature = "file_store")]
fn temp_file_store_path() -> String {
    let temp = TempFile::with_suffix("_filestore").unwrap();
    let path = temp
        .path()
        .as_os_str()
        .to_str()
        .expect("Failed to build path for FileStore")
        .to_owned();
    temp.cleanup()
        .expect("Failed to cleanup FileStore placeholder");
    path
}

#[test]
fn test_stores_single() {
    let temp_file_sqlite = TempFile::with_suffix("_sqlite.db").unwrap();
    let path_sqlite = temp_file_sqlite
        .path()
        .as_os_str()
        .to_str()
        .expect("Failed to build path for SQLite");
    println!("SQLite path: {}", path_sqlite);
    let temp_file_rockdb = TempFile::with_suffix("_rocksdb.db").unwrap();
    let path_rocksdb = temp_file_rockdb
        .path()
        .as_os_str()
        .to_str()
        .expect("Failed to build path for RocksDB")
        .to_owned();
    // RocksDB expects the file to not exists, so we make a temp name and force the cleanup of the file.
    temp_file_rockdb
        .cleanup()
        .expect("Failed to cleanup RocksDB");
    println!("RocksDB path: {}", path_rocksdb);

    #[cfg(feature = "file_store")]
    let path_file = temp_file_store_path();

    // Test all implemented stores
    let mut stores: Vec<Box<dyn Store>> = Vec::new();

    #[cfg(feature = "memory_store")]
    stores.push(Box::new(MemoryStore::default()));
    #[cfg(feature = "file_store")]
    stores.push(Box::new(FileStore::new(&path_file)));
    #[cfg(feature = "sled_store")]
    stores.push(Box::new(SledStore::new("/tmp/sled1.db", true)));
    #[cfg(feature = "sqlite_store")]
    stores.push(Box::new(SqliteStore::new(path_sqlite)));
    #[cfg(feature = "rocksdb_store")]
    stores.push(Box::new(RocksDbStore::new(&path_rocksdb)));

    for mut store in stores {
        store.put(0, 0, &[Node::ZERO]).unwrap();
        store.put(0, 1, &[Node::ZERO]).unwrap();
        store.put(0, 2, &[Node::ZERO]).unwrap();
        store
            .put(
                0,
                3,
                &[to_node!(
                    "0x1230000000000000000000000000000000000000000000000000000000000000"
                )],
            )
            .unwrap();

        assert_eq!(store.get_num_leaves(), 4);
        assert_eq!(
            store.get(&[0], &[0]).unwrap().pop().unwrap(),
            Some(Node::ZERO)
        );
        assert_eq!(
            store.get(&[0], &[1]).unwrap().pop().unwrap(),
            Some(Node::ZERO)
        );
        assert_eq!(
            store.get(&[0], &[2]).unwrap().pop().unwrap(),
            Some(Node::ZERO)
        );
        assert_eq!(
            store.get(&[0], &[3]).unwrap().pop().unwrap(),
            Some(to_node!(
                "0x1230000000000000000000000000000000000000000000000000000000000000"
            ))
        );
    }

    // Now delete the RocksDB directory.
    #[cfg(feature = "rocksdb_store")]
    fs::remove_dir_all(path_rocksdb).expect("Failed to delete RocksDB file");

    // Delete the FileStore directory.
    #[cfg(feature = "file_store")]
    fs::remove_dir_all(path_file).expect("Failed to delete FileStore directory");
}

#[test]
fn test_stores_multiple() {
    let temp_file_sqlite = TempFile::with_suffix("_sqlite.db").unwrap();
    let path_sqlite = temp_file_sqlite
        .path()
        .as_os_str()
        .to_str()
        .expect("Failed to build path for SQLite");
    println!("SQLite path: {}", path_sqlite);
    let temp_file_rockdb = TempFile::with_suffix("_rocksdb.db").unwrap();
    let path_rocksdb = temp_file_rockdb
        .path()
        .as_os_str()
        .to_str()
        .expect("Failed to build path for RocksDB")
        .to_owned();
    // RocksDB expects the file to not exists, so we make a temp name and force the cleanup of the file.
    temp_file_rockdb
        .cleanup()
        .expect("Failed to cleanup RocksDB");
    println!("RocksDB path: {}", path_rocksdb);

    #[cfg(feature = "file_store")]
    let path_file = temp_file_store_path();

    // Test all implemented stores
    let mut stores: Vec<Box<dyn Store>> = Vec::new();

    #[cfg(feature = "memory_store")]
    stores.push(Box::new(MemoryStore::default()));
    #[cfg(feature = "file_store")]
    stores.push(Box::new(FileStore::new(&path_file)));
    #[cfg(feature = "sled_store")]
    stores.push(Box::new(SledStore::new("/tmp/sled2.db", true)));
    #[cfg(feature = "sqlite_store")]
    stores.push(Box::new(SqliteStore::new(path_sqlite)));
    #[cfg(feature = "rocksdb_store")]
    stores.push(Box::new(RocksDbStore::new(&path_rocksdb)));

    // Simulates a simple merkle tree with 8 leaves.
    //        (root)
    //          |
    //       /     \
    //    (n)       (n)
    //     |         |
    //  /    \     /   \
    // (n)  (n)  (n)   (n)
    //  |    |    |    |
    // / \  / \  / \  / \
    // L L  L L  L L  L L

    // First we create some hashes for each level. We store the to assert later.
    // that they are fetched ok.

    // 8 leaves at the bottom
    let level_0 = (0u8..8)
        .map(|i| Node::from([(i + 1) * 0x11; Node::LEN]))
        .collect::<Vec<Node>>();

    // 4 nodes at the second level
    let level_1 = (0u8..4)
        .map(|i| Node::from([(i + 1) * 0x11; Node::LEN]))
        .collect::<Vec<Node>>();

    // 2 nodes at the third level
    let level_2 = (0u8..2)
        .map(|i| Node::from([(i + 1) * 0x11; Node::LEN]))
        .collect::<Vec<Node>>();

    // 1 node at the root
    let level_3 = (0u8..1)
        .map(|i| Node::from([(i + 1) * 0x11; Node::LEN]))
        .collect::<Vec<Node>>();

    // All 8+4+2+1 = 15 nodes, one run per level.
    let runs = [&level_0, &level_1, &level_2, &level_3];

    for mut store in stores {
        for (level, run) in runs.iter().enumerate() {
            store.put(level as u32, 0, run).unwrap();
        }

        assert_eq!(store.get_num_leaves(), 8);

        // Fetch some nodes the first 4 leaves at the bottom
        assert_eq!(
            store.get(&[0, 0, 0, 0], &[0, 1, 2, 3]).unwrap(),
            level_0
                .iter()
                .take(4)
                .map(|i| Some(*i))
                .collect::<Vec<Option<Node>>>()
        );

        // Fetch the last 4 leaves at the bottom
        assert_eq!(
            store.get(&[0, 0, 0, 0], &[4, 5, 6, 7]).unwrap(),
            level_0
                .iter()
                .skip(4)
                .map(|i| Some(*i))
                .collect::<Vec<Option<Node>>>()
        );

        // Fetch the leaves at the bottom. Every second one.
        assert_eq!(
            store.get(&[0, 0, 0, 0], &[0, 2, 4, 6]).unwrap(),
            level_0
                .iter()
                .step_by(2)
                .map(|i| Some(*i))
                .collect::<Vec<Option<Node>>>()
        );

        // Same for first level 1.
        assert_eq!(
            store.get(&[1, 1, 1, 1], &[0, 1, 2, 3]).unwrap(),
            level_1
                .iter()
                .map(|i| Some(*i))
                .collect::<Vec<Option<Node>>>()
        );

        // Level 1 but skipping the first one.
        assert_eq!(
            store.get(&[1, 1, 1], &[1, 2, 3]).unwrap(),
            level_1
                .iter()
                .skip(1)
                .map(|i| Some(*i))
                .collect::<Vec<Option<Node>>>()
        );

        // Same for level 2.
        assert_eq!(
            store.get(&[2, 2], &[0, 1]).unwrap(),
            level_2
                .iter()
                .map(|i| Some(*i))
                .collect::<Vec<Option<Node>>>()
        );

        // Same for level 3.
        assert_eq!(
            store.get(&[3], &[0]).unwrap(),
            level_3
                .iter()
                .map(|i| Some(*i))
                .collect::<Vec<Option<Node>>>()
        );

        // Fetch 4 levels/indexes that dont have any content.
        assert_eq!(
            store.get(&[4, 5, 6, 7], &[111, 13, 22, 99]).unwrap(),
            vec![None, None, None, None]
        );
    }

    // Now delete the RocksDB directory.
    #[cfg(feature = "rocksdb_store")]
    fs::remove_dir_all(path_rocksdb).expect("Failed to delete RocksDB file");

    // Delete the FileStore directory.
    #[cfg(feature = "file_store")]
    fs::remove_dir_all(path_file).expect("Failed to delete FileStore directory");
}
