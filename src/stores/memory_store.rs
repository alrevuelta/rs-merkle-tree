// Copyright 2025 Bilinear Labs - MIT License

//! Simple in-memory store implementation.

use crate::{MerkleError, Node, Store};
use std::{collections::HashMap, sync::RwLock};

/// Simple in-memory store implementation using a `HashMap`.
#[derive(Default)]
pub struct MemoryStore {
    inner: RwLock<MemoryStoreInner>,
}

#[derive(Default)]
struct MemoryStoreInner {
    store: HashMap<(u32, u64), Node>,
    num_leaves: u64,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Store for MemoryStore {
    fn get(&self, levels: &[u32], indices: &[u64]) -> Result<Vec<Option<Node>>, MerkleError> {
        if levels.len() != indices.len() {
            return Err(MerkleError::LengthMismatch {
                levels: levels.len(),
                indices: indices.len(),
            });
        }
        let inner = self.inner.read().map_err(|e| {
            MerkleError::LockPoisoned(format!("Failed to acquire read lock on MemoryStore: {}", e))
        })?;
        let result = levels
            .iter()
            .zip(indices)
            .map(|(&lvl, &idx)| inner.store.get(&(lvl, idx)).cloned())
            .collect();
        Ok(result)
    }

    fn put(&mut self, items: &[(u32, u64, Node)]) -> Result<(), MerkleError> {
        let mut inner = self.inner.write().map_err(|e| {
            MerkleError::LockPoisoned(format!(
                "Failed to acquire write lock on MemoryStore: {}",
                e
            ))
        })?;
        for (level, index, hash) in items {
            inner.store.insert((*level, *index), *hash);
        }
        let counter = items.iter().filter(|(level, _, _)| *level == 0).count();
        inner.num_leaves += counter as u64;
        Ok(())
    }
    fn get_num_leaves(&self) -> u64 {
        // For get_num_leaves, we use expect since it's a simple getter and lock poisoning
        // would indicate a serious bug. Using expect provides a clearer panic message.
        self.inner.read()
            .expect("MemoryStore lock was poisoned - this indicates a panic occurred while holding the lock")
            .num_leaves
    }
}
