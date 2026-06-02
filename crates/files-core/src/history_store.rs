use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};

use crate::config::config_dir;

const CYBER_FILES_DB_FILE: &str = "cyber_files.db";
const TABLE_PATH_HISTORY: &str = "path_history";
const TABLE_SEARCH_HISTORY: &str = "search_history";

#[derive(Debug, Clone, Copy)]
pub(crate) enum HistoryKind {
    Path,
    Search,
}

impl HistoryKind {
    fn table_name(self) -> &'static str {
        match self {
            Self::Path => TABLE_PATH_HISTORY,
            Self::Search => TABLE_SEARCH_HISTORY,
        }
    }
}

pub(crate) fn list(kind: HistoryKind, limit: usize) -> Vec<String> {
    list_impl(kind, limit).unwrap_or_default()
}

pub(crate) fn record(kind: HistoryKind, value: &str, limit: usize) -> anyhow::Result<()> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    let conn = open_history_db()?;
    let tx = conn.unchecked_transaction()?;
    let table = kind.table_name();
    tx.execute(&format!("DELETE FROM {table} WHERE value = ?1"), params![trimmed])?;
    tx.execute(
        &format!("INSERT INTO {table} (value, updated_at) VALUES (?1, ?2)"),
        params![trimmed, now_unix_ts()],
    )?;
    tx.execute(
        &format!(
            "DELETE FROM {table}
             WHERE id NOT IN (
                 SELECT id FROM {table}
                 ORDER BY updated_at DESC, id DESC
                 LIMIT ?1
             )"
        ),
        params![limit as i64],
    )?;
    tx.commit()?;
    Ok(())
}

fn list_impl(kind: HistoryKind, limit: usize) -> anyhow::Result<Vec<String>> {
    let conn = open_history_db()?;
    let mut stmt = conn.prepare(&format!(
        "SELECT value FROM {}
         ORDER BY updated_at DESC, id DESC
         LIMIT ?1",
        kind.table_name()
    ))?;
    let rows = stmt.query_map(params![limit as i64], |row| row.get::<_, String>(0))?;
    let mut values = Vec::new();
    for row in rows {
        values.push(row?);
    }
    Ok(values)
}

fn open_history_db() -> anyhow::Result<Connection> {
    let path = history_db_path().ok_or_else(|| anyhow::anyhow!("config directory unavailable"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    initialize_schema(&conn)?;
    Ok(conn)
}

fn history_db_path() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join(CYBER_FILES_DB_FILE))
}

fn initialize_schema(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS path_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            value TEXT NOT NULL UNIQUE,
            updated_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS search_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            value TEXT NOT NULL UNIQUE,
            updated_at INTEGER NOT NULL
        );
        ",
    )?;
    Ok(())
}

fn now_unix_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}
