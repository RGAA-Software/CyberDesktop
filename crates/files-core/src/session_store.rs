use std::fs;
use std::path::PathBuf;

use rusqlite::{params, Connection};

use crate::config::config_dir;

const CYBER_FILES_DB_FILE: &str = "cyber_files.db";

pub fn load_session_tabs() -> Vec<String> {
    let Some(path) = session_db_path() else {
        return Vec::new();
    };
    if !path.exists() {
        return Vec::new();
    }
    load_session_tabs_impl().unwrap_or_default()
}

pub fn save_session_tabs(tabs: &[String]) -> anyhow::Result<()> {
    let conn = open_session_db()?;
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM session_tabs", [])?;
    for (position, target) in tabs.iter().enumerate() {
        tx.execute(
            "INSERT INTO session_tabs (position, target) VALUES (?1, ?2)",
            params![position as i64, target],
        )?;
    }
    tx.commit()?;
    Ok(())
}

fn load_session_tabs_impl() -> anyhow::Result<Vec<String>> {
    let conn = open_session_db()?;
    let mut stmt = conn.prepare(
        "SELECT target
         FROM session_tabs
         ORDER BY position ASC",
    )?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut tabs = Vec::new();
    for row in rows {
        tabs.push(row?);
    }
    Ok(tabs)
}

fn open_session_db() -> anyhow::Result<Connection> {
    let path = session_db_path().ok_or_else(|| anyhow::anyhow!("config directory unavailable"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    initialize_schema(&conn)?;
    Ok(conn)
}

fn session_db_path() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join(CYBER_FILES_DB_FILE))
}

fn initialize_schema(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS session_tabs (
            position INTEGER PRIMARY KEY,
            target TEXT NOT NULL
        );
        ",
    )?;
    Ok(())
}
