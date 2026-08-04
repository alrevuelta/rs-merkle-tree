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

### `add_leaves` throughput

| Depth | Hash | Leaves | Batch | Store | Throughput | p50 batch | p99 batch | Disk |
|---|---|---|---|---|---|---|---|---|
| 32 | keccak256 | 5000000 | 100000 | memory | 27.874 Melem/s | 3.578 ms | 4.866 ms | - |
| 32 | keccak256 | 5000000 | 100000 | file | 25.317 Melem/s | 3.664 ms | 5.240 ms | 305.18 MiB |
| 32 | keccak256 | 5000000 | 100000 | rocksdb | 4.379 Melem/s | 22.681 ms | 27.794 ms | 434.44 MiB |
| 32 | keccak256 | 5000000 | 100000 | sqlite | 893.300 Kelem/s | 111.967 ms | 134.071 ms | 590.39 MiB |
| 32 | keccak256 | 5000000 | 100000 | sled | 238.879 Kelem/s | 414.618 ms | 487.329 ms | 1.55 GiB |

The stores run one after another in a single process, so by the time the flat
file store runs the page cache is already under pressure from the memory store.
Measured on its own it reaches 27.7 Melem/s, and repeated runs of the table above
put it anywhere between 23.7 and 25.3.

Throughput also depends strongly on how many leaves are handed over per call,
because a call hashes each level once and writes each level once regardless of
how much of the level it fills. On the flat file store, measured alone:

| Batch | Throughput |
|---|---|
| 1000 | 2.85 Melem/s |
| 10000 | 10.91 Melem/s |
| 100000 | 27.67 Melem/s |

### `proof` time

| Depth | Hash | Store | Time |
|---|---|---|---|
| 32 | keccak256 | memory | 199.360 ns |
| 32 | keccak256 | file | 5.130 µs |
| 32 | keccak256 | sled | 7.566 µs |
| 32 | keccak256 | sqlite | 11.914 µs |
| 32 | keccak256 | rocksdb | 14.988 µs |

## License

[MIT License](https://github.com/bilinearlabs/rs-merkle-tree/blob/main/LICENSE)
