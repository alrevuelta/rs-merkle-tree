// Copyright 2025 Bilinear Labs - MIT License

//! RocksDB store implementation.

#[cfg(feature = "rocksdb_store")]
use crate::{MerkleError, Node, Store};

#[cfg(feature = "rocksdb_store")]
pub struct RocksDbStore {
    db: rocksdb::DB,
    num_leaves: u64,
}

#[cfg(feature = "rocksdb_store")]
impl RocksDbStore {
    const KEY_NUM_LEAVES: &'static [u8] = b"NUM_LEAVES";

    fn db_error<E: std::fmt::Display>(err: E) -> MerkleError {
        MerkleError::StoreError(err.to_string())
    }

    fn encode_key(level: u32, index: u64) -> [u8; 12] {
        let mut key = [0u8; 12];
        key[..4].copy_from_slice(&level.to_be_bytes());
        key[4..].copy_from_slice(&index.to_be_bytes());
        key
    }

    fn decode_node(bytes: &[u8]) -> Result<Node, MerkleError> {
        let arr: [u8; Node::LEN] = bytes
            .try_into()
            .map_err(|_| MerkleError::StoreError("invalid node length".into()))?;
        Ok(Node::from(arr))
    }

    pub fn new(path: &str) -> Self {
        use rocksdb::{Options, DB};
        let mut opts = Options::default();
        opts.create_if_missing(true);
        let db = DB::open(&opts, path).expect("failed to open rocksdb");

        let num_leaves = db
            .get(Self::KEY_NUM_LEAVES)
            .expect("failed to get num_leaves")
            .map(|v| {
                let slice: &[u8] = &v;
                let bytes: [u8; 8] = slice.try_into().expect("invalid num_leaves length");
                u64::from_be_bytes(bytes)
            })
            .unwrap_or(0);

        if num_leaves == 0 {
            db.flush().expect("failed to flush");
            // TODO: unsure if ok
        }

        Self { db, num_leaves }
    }
}

#[cfg(feature = "rocksdb_store")]
impl Store for RocksDbStore {
    fn get(&self, levels: &[u32], indices: &[u64]) -> Result<Vec<Option<Node>>, MerkleError> {
        if levels.len() != indices.len() {
            return Err(MerkleError::LengthMismatch {
                levels: levels.len(),
                indices: indices.len(),
            });
        }

        let keys: Vec<[u8; 12]> = levels
            .iter()
            .zip(indices)
            .map(|(&lvl, &idx)| Self::encode_key(lvl, idx))
            .collect();

        // TODO: The use of multi_get to do batch reads doesn not really improve the
        // performance. Check if there is some fine tuning in rocks db that can spped this up.
        let result: Result<Vec<Option<Node>>, MerkleError> = self
            .db
            .multi_get(keys.iter())
            .into_iter()
            .map(|res| match res {
                Ok(Some(slice)) => Self::decode_node(&slice).map(Some),
                Ok(None) => Ok(None),
                Err(e) => Err(Self::db_error(e)),
            })
            .collect();

        result
    }

    fn put(&mut self, level: u32, start: u64, nodes: &[Node]) -> Result<(), MerkleError> {
        if nodes.is_empty() {
            return Ok(());
        }

        use rocksdb::WriteBatch;
        let mut batch = WriteBatch::default();
        for (offset, node) in nodes.iter().enumerate() {
            let key = Self::encode_key(level, start + offset as u64);
            batch.put(key, node.as_ref());
        }

        let num_leaves = if level == 0 {
            self.num_leaves.max(start + nodes.len() as u64)
        } else {
            self.num_leaves
        };
        batch.put(Self::KEY_NUM_LEAVES, num_leaves.to_be_bytes().as_ref());

        self.db.write(batch).map_err(Self::db_error)?;
        self.num_leaves = num_leaves;
        Ok(())
    }

    fn get_num_leaves(&self) -> u64 {
        self.num_leaves
    }
}
