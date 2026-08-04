// Copyright 2025 Bilinear Labs - MIT License

//! Flat file store implementation.
//!
//! Stores the tree as one flat file per level inside a directory. Since the Merkle tree
//! is append only, all files at each level can be append only. Inserts are batched to only
//! trigger a syscall per level.
//!
//! TODO: There is no WAL nor recovery strategy if the file goes corrupt.

use crate::{MerkleError, Node, Store};
use std::fs::{create_dir_all, File, OpenOptions};
use std::os::unix::fs::FileExt;
use std::path::PathBuf;

/// Flat file store: one file per level.
pub struct FileStore {
    /// Directory where the store is placed
    dir: PathBuf,
    /// Open file handle per level.
    files: Vec<File>,
    /// Number of nodes stored at each level, i.e. `file_len / Node::LEN`.
    /// `counts[0]` is the number of leaves, bottom level.
    counts: Vec<u64>,
}

fn io_err<E: std::fmt::Display>(err: E) -> MerkleError {
    MerkleError::StoreError(err.to_string())
}

fn level_file_name(level: usize) -> String {
    format!("level_{:03}", level)
}

impl FileStore {
    /// Opens (creating if needed) a file store rooted at `dir`.
    pub fn new(dir: &str) -> Self {
        let dir = PathBuf::from(dir);
        create_dir_all(&dir).expect("failed to create file store directory");

        let node_len = Node::LEN as u64;
        let mut files = Vec::new();
        let mut counts = Vec::new();

        // Levels are written as a contiguous path 0..=DEPTH, so existing level
        // files form a contiguous run. Probe level_000, level_001, etc
        loop {
            let path = dir.join(level_file_name(files.len()));
            if !path.exists() {
                break;
            }
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path)
                .expect("failed to open level file");
            let len = file
                .metadata()
                .expect("failed to read level file metadata")
                .len();
            files.push(file);

            counts.push(len / node_len);
        }

        Self { dir, files, counts }
    }

    /// Ensures a file (and count slot) exists for every level up to `level`.
    fn ensure_level(&mut self, level: usize) -> Result<(), MerkleError> {
        while self.files.len() <= level {
            let path = self.dir.join(level_file_name(self.files.len()));
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(&path)
                .map_err(io_err)?;
            let count = file.metadata().map_err(io_err)?.len() / Node::LEN as u64;
            self.counts.push(count);
            self.files.push(file);
        }
        Ok(())
    }
}

impl Store for FileStore {
    fn get(&self, levels: &[u32], indices: &[u64]) -> Result<Vec<Option<Node>>, MerkleError> {
        if levels.len() != indices.len() {
            return Err(MerkleError::LengthMismatch {
                levels: levels.len(),
                indices: indices.len(),
            });
        }

        levels
            .iter()
            .zip(indices)
            .map(|(&lvl, &idx)| {
                let level = lvl as usize;
                // A node is present if its index is within the stored prefix.
                // This also makes reads beyond the tree (allowed by `proof`)
                // return `None`, which the tree maps to `zeros[level]`.
                if level >= self.counts.len() || idx >= self.counts[level] {
                    return Ok(None);
                }
                let mut bytes = [0u8; Node::LEN];
                self.files[level]
                    .read_exact_at(&mut bytes, idx * Node::LEN as u64)
                    .map_err(io_err)?;
                Ok(Some(Node::from(bytes)))
            })
            .collect()
    }

    /// One positional write per call, straight out of the caller's slice.
    ///
    /// A run is already laid out exactly as the level file holds it, so there is
    /// nothing to serialise: `Node::as_bytes` reinterprets it and the kernel
    /// copies it once. The only validation left is that the run does not start
    /// past the stored prefix, which is a single comparison per call rather than
    /// per node.
    fn put(&mut self, level: u32, start: u64, nodes: &[Node]) -> Result<(), MerkleError> {
        if nodes.is_empty() {
            return Ok(());
        }

        let level = level as usize;
        self.ensure_level(level)?;

        if start > self.counts[level] {
            return Err(io_err(format!(
                "put at level {level} starts at index {start}, past the stored prefix of {}",
                self.counts[level]
            )));
        }

        self.files[level]
            .write_all_at(Node::as_bytes(nodes), start * Node::LEN as u64)
            .map_err(io_err)?;
        self.counts[level] = self.counts[level].max(start + nodes.len() as u64);

        Ok(())
    }

    fn get_num_leaves(&self) -> u64 {
        self.counts.first().copied().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(byte: u8) -> Node {
        Node::from([byte; Node::LEN])
    }

    fn temp_dir(name: &str) -> String {
        let dir = std::env::temp_dir().join(format!(
            "rs_merkle_file_store_{name}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir.to_str().expect("utf-8 temp path").to_owned()
    }

    #[test]
    fn put_get_roundtrip() {
        let dir = temp_dir("roundtrip");
        let mut store = FileStore::new(&dir);

        store.put(0, 0, &[node(0x0a), node(0x0b)]).unwrap();
        store.put(1, 0, &[node(0xff)]).unwrap();
        // Rewriting inside the prefix, as the tree does to the right spine.
        store.put(1, 0, &[node(0x1a)]).unwrap();

        assert_eq!(store.get_num_leaves(), 2);
        assert_eq!(
            store.get(&[0, 0, 1], &[0, 1, 0]).unwrap(),
            vec![Some(node(0x0a)), Some(node(0x0b)), Some(node(0x1a))]
        );

        // A run that both rewrites the tail of the prefix and extends it.
        store.put(1, 0, &[node(0x1c), node(0x1b)]).unwrap();
        store.put(0, 2, &[node(0x0c)]).unwrap();
        assert_eq!(
            store.get(&[0, 1, 1], &[2, 0, 1]).unwrap(),
            vec![Some(node(0x0c)), Some(node(0x1c)), Some(node(0x1b))]
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn out_of_range_reads_are_none() {
        let dir = temp_dir("out_of_range");
        let mut store = FileStore::new(&dir);
        store.put(0, 0, &[node(0x0a)]).unwrap();
        store.put(1, 0, &[node(0x1a)]).unwrap();

        // Beyond the stored prefix of an existing level and beyond all levels.
        assert_eq!(
            store.get(&[0, 1, 7], &[1, 9, 0]).unwrap(),
            vec![None, None, None]
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn runs_starting_past_the_prefix_are_rejected() {
        let dir = temp_dir("non_contiguous");
        let mut store = FileStore::new(&dir);
        store.put(0, 0, &[node(0x0a)]).unwrap();

        // A run starting past the stored prefix would leave a hole that reads
        // back as though it held a node.
        assert!(store.put(0, 2, &[node(0x0b)]).is_err());
        assert!(store.put(0, u64::MAX, &[node(0x0c)]).is_err());
        assert!(store.put(3, 1, &[node(0x0d)]).is_err());

        // The store stays usable and unchanged after rejections.
        assert_eq!(store.get_num_leaves(), 1);
        store.put(0, 1, &[node(0x0b)]).unwrap();
        assert_eq!(store.get_num_leaves(), 2);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn empty_runs_are_accepted_and_change_nothing() {
        let dir = temp_dir("empty");
        let mut store = FileStore::new(&dir);
        store.put(0, 0, &[]).unwrap();
        store.put(9, 7, &[]).unwrap();
        assert_eq!(store.get_num_leaves(), 0);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reopen_keeps_everything() {
        let dir = temp_dir("reopen");
        {
            let mut store = FileStore::new(&dir);
            store.put(0, 0, &[node(0x0a), node(0x0b)]).unwrap();
            store.put(1, 0, &[node(0x1a)]).unwrap();
        }

        let mut store = FileStore::new(&dir);
        assert_eq!(store.get_num_leaves(), 2);
        assert_eq!(store.get(&[1], &[0]).unwrap(), vec![Some(node(0x1a))]);

        store.put(0, 2, &[node(0x0c)]).unwrap();
        store.put(1, 1, &[node(0x1b)]).unwrap();
        assert_eq!(store.get_num_leaves(), 3);
        assert_eq!(
            store.get(&[0, 1], &[2, 1]).unwrap(),
            vec![Some(node(0x0c)), Some(node(0x1b))]
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
