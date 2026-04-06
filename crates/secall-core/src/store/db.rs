use std::path::Path;

use anyhow::Result;
use rusqlite::Connection;

use super::schema::{
    CURRENT_SCHEMA_VERSION, CREATE_CONFIG, CREATE_INDEXES, CREATE_INGEST_LOG,
    CREATE_SESSIONS, CREATE_TURNS, CREATE_TURNS_FTS,
};

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    pub fn migrate(&self) -> Result<()> {
        // Ensure config table exists first
        self.conn.execute_batch(CREATE_CONFIG)?;

        let version: Option<u32> = self
            .conn
            .query_row(
                "SELECT value FROM config WHERE key = 'schema_version'",
                [],
                |row| {
                    let v: String = row.get(0)?;
                    Ok(v.parse::<u32>().unwrap_or(0))
                },
            )
            .ok();

        let current = version.unwrap_or(0);

        if current < CURRENT_SCHEMA_VERSION {
            self.apply_v1()?;
            self.conn.execute(
                "INSERT OR REPLACE INTO config(key, value) VALUES ('schema_version', ?1)",
                [CURRENT_SCHEMA_VERSION.to_string()],
            )?;
        }

        Ok(())
    }

    fn apply_v1(&self) -> Result<()> {
        self.conn.execute_batch(CREATE_SESSIONS)?;
        self.conn.execute_batch(CREATE_TURNS)?;
        self.conn.execute_batch(CREATE_TURNS_FTS)?;
        self.conn.execute_batch(CREATE_INGEST_LOG)?;
        self.conn.execute_batch(CREATE_INDEXES)?;
        Ok(())
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Execute a closure within a SQLite transaction.
    /// Commits on Ok, rolls back on Err.
    pub fn with_transaction<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce() -> Result<T>,
    {
        self.conn.execute_batch("BEGIN")?;
        match f() {
            Ok(val) => {
                self.conn.execute_batch("COMMIT")?;
                Ok(val)
            }
            Err(e) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }

    /// Get database statistics
    pub fn get_stats(&self) -> Result<DbStats> {
        let session_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))?;
        let turn_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM turns", [], |r| r.get(0))?;
        let vector_count: i64 = {
            let exists: i64 = self.conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='turn_vectors'",
                [],
                |r| r.get(0),
            )?;
            if exists > 0 {
                self.conn.query_row("SELECT COUNT(*) FROM turn_vectors", [], |r| r.get(0))?
            } else {
                0
            }
        };

        let mut stmt = self.conn.prepare(
            "SELECT il.session_id, s.agent, il.timestamp
             FROM ingest_log il
             LEFT JOIN sessions s ON il.session_id = s.id
             WHERE il.action = 'ingest'
             ORDER BY il.id DESC LIMIT 5",
        )?;
        let recent_ingests = stmt
            .query_map([], |row| {
                let sid: String = row.get(0)?;
                let agent: Option<String> = row.get(1)?;
                let ts: String = row.get(2)?;
                Ok(IngestLogEntry {
                    session_id_prefix: sid[..sid.len().min(8)].to_string(),
                    agent: agent.unwrap_or_else(|| "unknown".to_string()),
                    timestamp: ts[..ts.len().min(10)].to_string(),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(DbStats {
            session_count,
            turn_count,
            vector_count,
            recent_ingests,
        })
    }

    /// Get a specific turn by session_id and turn_index
    pub fn get_turn(&self, session_id: &str, turn_index: u32) -> Result<TurnRow> {
        self.conn.query_row(
            "SELECT turn_index, role, content FROM turns WHERE session_id = ?1 AND turn_index = ?2",
            rusqlite::params![session_id, turn_index as i64],
            |row| {
                Ok(TurnRow {
                    turn_index: row.get::<_, i64>(0)? as u32,
                    role: row.get(1)?,
                    content: row.get(2)?,
                })
            },
        )
        .map_err(Into::into)
    }

    pub fn count_sessions(&self) -> Result<i64> {
        let count = self
            .conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))?;
        Ok(count)
    }

    pub fn list_projects(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT project FROM sessions WHERE project IS NOT NULL",
        )?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn list_agents(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT DISTINCT agent FROM sessions")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn has_embeddings(&self) -> Result<bool> {
        let exists: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='turn_vectors'",
            [],
            |r| r.get(0),
        )?;
        if exists == 0 {
            return Ok(false);
        }
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM turn_vectors", [], |r| r.get(0))?;
        Ok(count > 0)
    }

    // ─── Lint helpers ────────────────────────────────────────────────────────

    /// Return (session_id, vault_path) for all sessions
    pub fn list_session_vault_paths(&self) -> Result<Vec<(String, Option<String>)>> {
        let mut stmt = self.conn.prepare("SELECT id, vault_path FROM sessions")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Count rows in the turns_fts virtual table
    pub fn count_fts_rows(&self) -> Result<i64> {
        let count = self
            .conn
            .query_row("SELECT COUNT(*) FROM turns_fts", [], |r| r.get(0))?;
        Ok(count)
    }

    /// Count rows in the turns table
    pub fn count_turns(&self) -> Result<i64> {
        let count = self
            .conn
            .query_row("SELECT COUNT(*) FROM turns", [], |r| r.get(0))?;
        Ok(count)
    }

    /// Sessions that have no rows in turn_vectors
    pub fn find_sessions_without_vectors(&self) -> Result<Vec<String>> {
        let table_exists: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='turn_vectors'",
            [],
            |r| r.get(0),
        )?;

        let query = if table_exists == 0 {
            "SELECT id FROM sessions"
        } else {
            "SELECT id FROM sessions WHERE id NOT IN (SELECT DISTINCT session_id FROM turn_vectors)"
        };

        let mut stmt = self.conn.prepare(query)?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Vector rows whose session_id does not exist in sessions
    pub fn find_orphan_vectors(&self) -> Result<Vec<(i64, String)>> {
        let table_exists: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='turn_vectors'",
            [],
            |r| r.get(0),
        )?;

        if table_exists == 0 {
            return Ok(Vec::new());
        }

        let mut stmt = self.conn.prepare(
            "SELECT id, session_id FROM turn_vectors WHERE session_id NOT IN (SELECT id FROM sessions)",
        )?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Count sessions per agent
    pub fn agent_counts(&self) -> Result<std::collections::HashMap<String, usize>> {
        let mut stmt = self
            .conn
            .prepare("SELECT agent, COUNT(*) FROM sessions GROUP BY agent")?;
        let rows = stmt.query_map([], |row| {
            let agent: String = row.get(0)?;
            let count: i64 = row.get(1)?;
            Ok((agent, count as usize))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Return all session IDs in the database
    pub fn list_all_session_ids(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT id FROM sessions")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Find session IDs ingested more than once in ingest_log
    pub fn find_duplicate_ingest_entries(&self) -> Result<Vec<(String, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT session_id, COUNT(*) as cnt FROM ingest_log WHERE action='ingest' GROUP BY session_id HAVING cnt > 1",
        )?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// 기존 절대경로 vault_path를 상대경로로 변환 (one-time migration)
    pub fn migrate_vault_paths_to_relative(&self, vault_root: &Path) -> Result<usize> {
        let vault_root_str = vault_root.to_string_lossy();
        let prefix = format!("{}/", vault_root_str.trim_end_matches('/'));

        let mut stmt = self.conn.prepare(
            "SELECT id, vault_path FROM sessions WHERE vault_path IS NOT NULL",
        )?;
        let rows: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();

        let mut migrated = 0;
        for (session_id, vault_path) in &rows {
            if vault_path.starts_with(&prefix) {
                let relative = &vault_path[prefix.len()..];
                self.conn.execute(
                    "UPDATE sessions SET vault_path = ?1 WHERE id = ?2",
                    rusqlite::params![relative, session_id],
                )?;
                migrated += 1;
            }
        }
        Ok(migrated)
    }

    #[cfg(test)]
    pub fn schema_version(&self) -> Result<u32> {
        let v: String = self.conn.query_row(
            "SELECT value FROM config WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )?;
        Ok(v.parse()?)
    }

    #[cfg(test)]
    pub fn table_exists(&self, name: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [name],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }
}

#[derive(Debug)]
pub struct DbStats {
    pub session_count: i64,
    pub turn_count: i64,
    pub vector_count: i64,
    pub recent_ingests: Vec<IngestLogEntry>,
}

#[derive(Debug)]
pub struct IngestLogEntry {
    pub session_id_prefix: String,
    pub agent: String,
    pub timestamp: String,
}

#[derive(Debug)]
pub struct TurnRow {
    pub turn_index: u32,
    pub role: String,
    pub content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_memory_success() {
        let db = Database::open_memory().unwrap();
        assert!(db.table_exists("sessions").unwrap());
    }

    #[test]
    fn test_migrate_creates_sessions_table() {
        let db = Database::open_memory().unwrap();
        assert!(db.table_exists("sessions").unwrap());
    }

    #[test]
    fn test_migrate_creates_turns_fts() {
        let db = Database::open_memory().unwrap();
        // FTS tables appear as 'table' in sqlite_master
        let count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name='turns_fts'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(count > 0);
    }

    #[test]
    fn test_schema_version_stored() {
        let db = Database::open_memory().unwrap();
        assert_eq!(db.schema_version().unwrap(), 1);
    }

    #[test]
    fn test_migrate_idempotent() {
        let db = Database::open_memory().unwrap();
        // Second migrate call should not error
        db.migrate().unwrap();
        assert_eq!(db.schema_version().unwrap(), 1);
    }
}
