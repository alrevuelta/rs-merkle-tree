# rs-merkle-tree

![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/bilinearlabs/rs-merkle-tree/rust_main_ci.yml?style=flat-square)
![Codecov (with branch)](https://img.shields.io/codecov/c/github/bilinearlabs/rs-merkle-tree/main?token=1PIHE7U7XQ&style=flat-square)
![GitHub License](https://img.shields.io/github/license/bilinearlabs/rs-merkle-tree?style=flat-square)
[![Join our Discord](https://img.shields.io/badge/Discord-5865F2?logo=discord&logoColor=white&style=flat-square)](https://discord.gg/Et8BTnVBZS)

Merkle tree implementation in Rust with the following features:
* Fixed depth: All proofs have a constant size equal to the `Depth`.
* Append-only: Leaves are added sequentially starting at index `0`. Once added, a leaf cannot be modified.
* Optimized for Merkle proof retrieval: Intermediate leaves are stored so that Merkle proofs can be fetched from memory without needing to be calculated lazily, resulting in very fast retrieval times.
* Configurable storage backends to store the bottom and intermediate leaves up the root.
* Configurable hash functions to hash nodes.
* Simple and easy to use interface: `add_leaves`, `root`, `num_leaves`, `proof`.


Add `rs-merkle-tree` as a dependency to your Rust `Cargo.toml`.

```toml
[dependencies]
rs-merkle-tree = "0.1.0"
```

You can create a Merkle tree, add leaves, get the number of leaves and get the Merkle proof of a given index as follows. This creates a simple merkle tree using keccak256 hashing algorithm, a memory storage and a depth 32.

```rust
use rs_merkle_tree::to_node;
use rs_merkle_tree::tree::MerkleTree32;

fn main() {
    let mut tree = MerkleTree32::default();
    tree.add_leaves(&[to_node!(
        "0x532c79f3ea0f4873946d1b14770eaa1c157255a003e73da987b858cc287b0482"
    )])
    .unwrap();

    println!("root: {:?}", tree.root().unwrap());
    println!("num leaves: {:?}", tree.num_leaves());
    println!("proof: {:?}", tree.proof(0).unwrap().proof);
}
```

You can customize your tree by choosing a different store, hash function, and depth as follows. Note that you have to modify the `feature` for the stores. This avoids importing the stuff you don't need. See the following examples.

**Depth: 32 | Hashing: Keccak | Store: sled**

```toml
[dependencies]
rs-merkle-tree = { version = "0.1.0", features = ["sled_store"] }
```

```rust
use rs_merkle_tree::hasher::Keccak256Hasher;
use rs_merkle_tree::stores::SledStore;
use rs_merkle_tree::tree::MerkleTree;

fn main() {
    let mut tree: MerkleTree<Keccak256Hasher, SledStore, 32> =
        MerkleTree::new(Keccak256Hasher, SledStore::new("sled.db", true));
}
```

**Depth: 32 | Hashing: Poseidon | Store: rocksdb**
```toml
rs-merkle-tree = { version = "0.1.0", features = ["rocksdb_store"] }
```

```rust
use rs_merkle_tree::hasher::PoseidonHasher;
use rs_merkle_tree::stores::RocksDbStore;
use rs_merkle_tree::tree::MerkleTree;

fn main() {
    let mut tree: MerkleTree<PoseidonHasher, RocksDbStore, 32> =
        MerkleTree::new(PoseidonHasher, RocksDbStore::new("rocksdb.db"));
}

```

**Depth: 32 | Hashing: Poseidon | Store: sqlite**

```toml
rs-merkle-tree = { version = "0.1.0", features = ["sqlite_store"] }
```

```rust
use rs_merkle_tree::hasher::PoseidonHasher;
use rs_merkle_tree::stores::SqliteStore;
use rs_merkle_tree::tree::MerkleTree;

fn main() {
    let mut tree: MerkleTree<PoseidonHasher, SqliteStore, 32> =
        MerkleTree::new(PoseidonHasher, SqliteStore::new("tree.db"));
}
```

**Depth: 32 | Hashing: Keccak | Store: file**

```toml
rs-merkle-tree = { version = "0.1.0", features = ["file_store"] }
```

```rust
use rs_merkle_tree::hasher::Keccak256Hasher;
use rs_merkle_tree::stores::FileStore;
use rs_merkle_tree::tree::MerkleTree;

fn main() {
    // Stores one flat file per level inside the given directory.
    let mut tree: MerkleTree<Keccak256Hasher, FileStore, 32> =
        MerkleTree::new(Keccak256Hasher, FileStore::new("filestore.db"));
}
```

## Stores

The following stores are supported:
* [rusqlite](https://github.com/rusqlite/rusqlite)
* [rocksdb](https://github.com/rust-rocksdb/rust-rocksdb)
* [sled](https://github.com/spacejam/sled)
* `file`: a flat store (no database engine) that keeps one file per level in a directory.

## Hash functions

The following hash functions are supported:
* [keccak256](https://github.com/debris/tiny-keccak)
* [Poseidon BN254 Circom T3](https://github.com/Lightprotocol/light-poseidon/)

## Benchmarks

The following benchmarks measure in a MacBook Pro M4 24GB the following:
* Consumed disk size
* Leaf insertion throughput in thousands per second.
* Merkle proof generation times.

You can run them with
```
cargo bench --features=all
```

And you can generate the following table with this.
```
python benchmarks.py
```

### Disk space usage

| Store | Depth | Leaves | Size (MiB) |
|---|---|---|---|
| sled | 32 | 1000000 | 290.00 |
| sqlite | 32 | 1000000 | 159.18 |
| rocksdb | 32 | 1000000 | 183.27 |
| file | 32 | 1000000 | 61.04 |

### `add_leaves` throughput

| Depth | Hash | Store | Throughput (Kelem/s) |
|---|---|---|---|
| 32 | keccak256 | rocksdb | 18.047 |
| 32 | keccak256 | sqlite | 31.300 |
| 32 | keccak256 | sled | 62.861 |
| 32 | keccak256 | file | 135.790 |
| 32 | keccak256 | memory | 142.940 |

### `proof` time

| Depth | Hash | Store | Time |
|---|---|---|---|
| 32 | keccak256 | memory | 188.280 ns |
| 32 | keccak256 | file | 5.150 µs |
| 32 | keccak256 | sled | 6.439 µs |
| 32 | keccak256 | sqlite | 11.341 µs |
| 32 | keccak256 | rocksdb | 41.998 µs |

## License

[MIT License](https://github.com/bilinearlabs/rs-merkle-tree/blob/main/LICENSE)
