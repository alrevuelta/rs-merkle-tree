// Copyright 2025 Bilinear Labs - MIT License

/*!
Merkle tree implementation in Rust with configurable storage backends and hash functions. This Merkle tree
implementation features:

* Fixed depth: All proofs have a constant size equal to the `Depth`.
* Append-only: Leaves are added sequentially starting at index `0`. Once added, a leaf cannot be modified.
* Optimized for Merkle proof retrieval: Intermediate leaves are stored so that Merkle proofs can be fetched
  from memory without needing to be calculated lazily, resulting in very fast retrieval times.

Add `rs-merkle-tree` as a dependency to your Rust `Cargo.toml`.

```toml
[dependencies]
rs-merkle-tree = { git = "https://github.com/bilinearlabs/rs-merkle-tree.git" }
```

You can create a Merkle tree, add leaves, get the number of leaves and get the Merkle proof of a given index as
follows. This creates a simple Merkle tree using **keccak256** hashing algorithm, a memory storage and a depth 32.

```rust,ignore
use rs_merkle_tree::to_node;
use rs_merkle_tree::tree::MerkleTree32;

fn main() {
    let mut tree = MerkleTree32::default();
    tree.add_leaves(&[to_node!(
        "0x532c79f3ea0f4873946d1b14770eaa1c157255a003e73da987b858cc287b0482"
    )])
    .unwrap();

    println!("root: {:?}", tree.root().unwrap());
    println!("num leaves: {:?}", tree.num_leaves().unwrap());
    println!("proof: {:?}", tree.proof(0).unwrap().read().unwrap().proof);
}
```

Note: This example requires the `memory_store` feature to be enabled.

You can customize your tree by choosing a different store, hash function, and depth as follows. This tree also uses
keccak256 but persists the leaves in a key-value sled store and has a depth of 32.

```rust,ignore
use rs_merkle_tree::stores::SledStore;
use rs_merkle_tree::tree::MerkleTree;
use rs_merkle_tree::hasher::Keccak256Hasher;

fn main() {
    let mut tree: MerkleTree<Keccak256Hasher, SledStore, 32> =
        MerkleTree::new(Keccak256Hasher, SledStore::new("sled.db", true));
}
```

Note: This example requires the `sled_store` feature to be enabled.

*/

pub mod errors;
pub mod hasher;
pub mod node;
pub mod tree;

pub mod stores {
    #[cfg(feature = "memory_store")]
    mod memory_store;
    pub mod store;
    #[cfg(feature = "memory_store")]
    pub use memory_store::MemoryStore;
    #[cfg(feature = "sled_store")]
    mod sled_store;
    #[cfg(feature = "sled_store")]
    pub use sled_store::SledStore;
    #[cfg(feature = "sqlite_store")]
    mod sqlite_store;
    #[cfg(feature = "sqlite_store")]
    pub use sqlite_store::SqliteStore;
    #[cfg(feature = "rocksdb_store")]
    mod rocksdb_store;
    #[cfg(feature = "rocksdb_store")]
    pub use rocksdb_store::RocksDbStore;
}

// Re-export the store module for easier access
pub use stores::store;

pub use errors::MerkleError;
pub use node::Node;
pub use stores::store::Store;

// Re-export MerkleTree and MerkleTree32 based on available features
#[cfg(feature = "memory_store")]
pub use tree::MerkleTree32;

// Re-export the generic MerkleTree for all store types
pub use tree::MerkleTree;
