// Copyright 2025 Bilinear Labs - MIT License

//! SQLite store implementation.

#[cfg(feature = "sqlite_store")]
use crate::{MerkleError, Node, Store};
#[cfg(feature = "sqlite_store")]
use rusqlite::{params, Connection, OptionalExtension};

#[cfg(feature = "sqlite_store")]
pub struct SqliteStore {
    conn: Connection,
    // Keeping an in-memory counter to avoid querying on every access.
    num_leaves: u64,
}

#[cfg(feature = "sqlite_store")]
const MAX_PARAMS: usize = 256;

#[cfg(feature = "sqlite_store")]
impl SqliteStore {
    const KEY_NUM_LEAVES: &'static str = "NUM_LEAVES";

    fn db_error<E: std::fmt::Display>(err: E) -> MerkleError {
        MerkleError::StoreError(err.to_string())
    }

    fn decode_node(bytes: &[u8]) -> Result<Node, MerkleError> {
        let arr: [u8; Node::LEN] = bytes
            .try_into()
            .map_err(|_| MerkleError::StoreError("invalid node length".into()))?;
        Ok(Node::from(arr))
    }

    // Simple tool to help generating the sql query.
    // If the input is levels=[33, 34], indices=[10, 20], the output will be:
    // sql="(?, ?, ?),(?, ?, ?)"
    // binds=[33, 10, 0, 34, 20, 1]
    // Note that in binds the 0 and 1 which is just a monotonic counter.
    fn build_values_sql_and_binds(levels: &[u32], indices: &[u64]) -> (String, Vec<i64>) {
        const PARAMS_PER_KEY: usize = 3;
        let mut binds = Vec::with_capacity(levels.len() * PARAMS_PER_KEY);

        let sql = levels
            .iter()
            .zip(indices)
            .enumerate()
            .map(|(ord, (&lvl, &idx))| {
                binds.extend([lvl as i64, idx as i64, ord as i64]);
                "(?, ?, ?)".to_string()
            })
            .collect::<Vec<_>>()
            .join(",");

        (sql, binds)
    }

    // Use ":memory:" for in-memory database.
    pub fn new(file_path: &str) -> Self {
        let conn = Connection::open(file_path).expect("failed to open sqlite DB");

        conn.execute_batch("PRAGMA journal_mode = WAL;\nPRAGMA synchronous = NORMAL;")
            .expect("failed to set WAL mode and synchronous pragma");

        // Create schema if not exists.
        conn.execute_batch(
            "BEGIN;
             CREATE TABLE IF NOT EXISTS nodes (
                 level INTEGER NOT NULL,
                 idx   INTEGER NOT NULL,
                 node  BLOB    NOT NULL CHECK(length(node) = 32),
                 PRIMARY KEY(level, idx)
             );
             CREATE TABLE IF NOT EXISTS metadata (
                 key   TEXT PRIMARY KEY,
                 value BLOB NOT NULL
             );
             COMMIT;",
        )
        .expect("failed to create tables");

        // Load persisted leaf count
        let num_leaves: u64 = conn
            .query_row(
                "SELECT value FROM metadata WHERE key = ?1",
                params![Self::KEY_NUM_LEAVES],
                |row| {
                    let bytes: [u8; 8] = row
                        .get::<_, Vec<u8>>(0)?
                        .as_slice()
                        .try_into()
                        .map_err(|_| rusqlite::Error::InvalidQuery)?;
                    Ok(u64::from_be_bytes(bytes))
                },
            )
            .optional()
            .expect("failed to query num leaves")
            .unwrap_or(0);

        // If the count is 0, clear the db, just in case.
        if num_leaves == 0 {
            conn.execute_batch("DELETE FROM nodes; DELETE FROM metadata;")
                .expect("failed to clear inconsistent DB state");
        }

        Self { conn, num_leaves }
    }
}

#[cfg(feature = "sqlite_store")]
impl Store for SqliteStore {
    fn get(&self, levels: &[u32], indices: &[u64]) -> Result<Vec<Option<Node>>, MerkleError> {
        if levels.len() != indices.len() {
            return Err(MerkleError::LengthMismatch {
                levels: levels.len(),
                indices: indices.len(),
            });
        }

        if levels.is_empty() {
            return Ok(Vec::new());
        }

        // Restrict to 256 elements to avoid SQLite parameter limit.
        // Practically this should never happen.
        if levels.len() > MAX_PARAMS {
            return Err(MerkleError::StoreError(format!(
                "levels length must be less than {}",
                MAX_PARAMS
            )));
        }

        let (values_sql, binds) = Self::build_values_sql_and_binds(levels, indices);

        // This query allows two things.
        // 1. It allows to query multiple levels/indeces in a single query.
        // 2. It returns the results in the same order as the input levels/indices.
        let sql = format!(
            "WITH req(level, idx, ord) AS (VALUES {values}) \
             SELECT node FROM req LEFT JOIN nodes USING(level, idx) ORDER BY ord",
            values = values_sql
        );

        let mut stmt = self.conn.prepare_cached(&sql).map_err(Self::db_error)?;

        let rows = stmt
            .query_map(rusqlite::params_from_iter(binds), |row| {
                row.get::<_, Option<Vec<u8>>>(0)
            })
            .map_err(Self::db_error)?;

        rows.map(|row| {
            row.map_err(Self::db_error)
                .and_then(|opt_blob| opt_blob.map(|b| Self::decode_node(&b)).transpose())
        })
        .collect::<Result<Vec<_>, _>>()
    }

    fn put(&mut self, level: u32, start: u64, nodes: &[Node]) -> Result<(), MerkleError> {
        if nodes.is_empty() {
            return Ok(());
        }

        let tx = self.conn.transaction().map_err(Self::db_error)?;

        {
            let mut insert_stmt = tx
                .prepare_cached(
                    "INSERT OR REPLACE INTO nodes (level, idx, node) VALUES (?1, ?2, ?3)",
                )
                .map_err(Self::db_error)?;

            for (offset, node) in nodes.iter().enumerate() {
                insert_stmt
                    .execute(params![
                        level as i64,
                        (start + offset as u64) as i64,
                        node.as_ref()
                    ])
                    .map_err(Self::db_error)?;
            }
        }

        let num_leaves = if level == 0 {
            let num_leaves = self.num_leaves.max(start + nodes.len() as u64);
            tx.execute(
                "INSERT OR REPLACE INTO metadata (key, value) VALUES (?1, ?2)",
                params![Self::KEY_NUM_LEAVES, num_leaves.to_be_bytes().to_vec()],
            )
            .map_err(Self::db_error)?;
            num_leaves
        } else {
            self.num_leaves
        };

        tx.commit().map_err(Self::db_error)?;
        self.num_leaves = num_leaves;
        Ok(())
    }

    fn get_num_leaves(&self) -> u64 {
        self.num_leaves
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_values_helper_smoke() {
        let (sql, binds) = SqliteStore::build_values_sql_and_binds(&[33, 34], &[10, 20]);
        assert_eq!(sql, "(?, ?, ?),(?, ?, ?)");
        assert_eq!(binds, vec![33, 10, 0, 34, 20, 1]);
    }
}
