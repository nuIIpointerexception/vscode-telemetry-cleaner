use rusqlite::Connection;
use std::path::Path;
use crate::utils::{Result, COUNT_QUERY, DELETE_QUERY};
use tokio::sync::mpsc;
use crate::zen_garden::ZenEvent;

pub fn clean_vscode_databases(directory: &Path, tx: &mpsc::UnboundedSender<ZenEvent>) -> Result<()> {
    clean_database_file(directory, "state.vscdb", tx)?;
    clean_database_file(directory, "state.vscdb.backup", tx)?;
    Ok(())
}

fn clean_database_file(directory: &Path, filename: &str, tx: &mpsc::UnboundedSender<ZenEvent>) -> Result<()> {
    let db_path = directory.join(filename);
    if !db_path.exists() { return Ok(()); }

    let conn = Connection::open(&db_path)?;
    let count: i64 = conn.prepare(COUNT_QUERY)?.query_row([], |row| row.get(0))?;

    if count > 0 {
        let display_name = db_path.file_name().unwrap_or_default().to_string_lossy();
        let _ = tx.send(ZenEvent::LogMessage(format!("discovered {} restless data spirits in '{}'", count, display_name)));

        conn.execute(DELETE_QUERY, [])?;

        let _ = tx.send(ZenEvent::LogMessage(format!("peacefully guided {} data spirits to rest in '{}'", count, display_name)));
    }

    Ok(())
}
