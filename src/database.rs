use rusqlite::Connection;
use std::path::Path;
use std::fs;
use std::process::Command;
use crate::utils::{Result, COUNT_QUERY, DELETE_QUERY};
use tokio::sync::mpsc;
use crate::zen_garden::ZenEvent;

// Windows file permission handling for database files
#[cfg(windows)]
struct DatabasePermissions {
    was_readonly: bool,
}

#[cfg(windows)]
impl DatabasePermissions {
    fn backup_and_make_writable(file_path: &Path) -> Result<Self> {
        let metadata = fs::metadata(file_path)?;
        let was_readonly = metadata.permissions().readonly();

        if was_readonly {
            // Remove read-only attribute
            let _ = Command::new("attrib").args(["-R", &file_path.to_string_lossy()]).status();

            // Also update permissions
            let mut permissions = metadata.permissions();
            permissions.set_readonly(false);
            fs::set_permissions(file_path, permissions)?;
        }

        Ok(DatabasePermissions { was_readonly })
    }

    fn restore(&self, file_path: &Path) -> Result<()> {
        if self.was_readonly {
            // Restore read-only attribute
            let _ = Command::new("attrib").args(["+R", &file_path.to_string_lossy()]).status();

            // Also update permissions
            let mut permissions = fs::metadata(file_path)?.permissions();
            permissions.set_readonly(true);
            fs::set_permissions(file_path, permissions)?;
        }
        Ok(())
    }
}

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
