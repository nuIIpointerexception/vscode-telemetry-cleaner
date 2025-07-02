use rusqlite::Connection;
use std::path::Path;
use std::fs;
use std::process::Command;
use crate::utils::{Result, CleanerError, ErrorCollector, COUNT_QUERY, DELETE_QUERY};
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
    let mut error_collector = ErrorCollector::new();

    // try to clean both database files, collecting errors instead of stopping
    if let Err(e) = clean_database_file(directory, "state.vscdb", tx) {
        let error = CleanerError::Database {
            operation: "cleaning state.vscdb".to_string(),
            path: directory.join("state.vscdb").display().to_string(),
            source: e.to_string(),
        };
        error_collector.add_error(error.clone());
        let _ = tx.send(ZenEvent::DetailedError(error));
    }

    if let Err(e) = clean_database_file(directory, "state.vscdb.backup", tx) {
        let error = CleanerError::Database {
            operation: "cleaning state.vscdb.backup".to_string(),
            path: directory.join("state.vscdb.backup").display().to_string(),
            source: e.to_string(),
        };
        error_collector.add_error(error.clone());
        let _ = tx.send(ZenEvent::DetailedError(error));
    }

    // send error summary if there were any errors
    if error_collector.has_errors() {
        let _ = tx.send(ZenEvent::ErrorSummary(error_collector));
        return Err("database cleaning encountered errors".into());
    }

    Ok(())
}

fn clean_database_file(directory: &Path, filename: &str, tx: &mpsc::UnboundedSender<ZenEvent>) -> Result<()> {
    let db_path = directory.join(filename);
    if !db_path.exists() {
        let _ = tx.send(ZenEvent::LogMessage(format!("database file '{}' not found - already at peace", filename)));
        return Ok(());
    }

    let display_name = db_path.file_name().unwrap_or_default().to_string_lossy();
    let _ = tx.send(ZenEvent::LogMessage(format!("examining data spirits in '{}'", display_name)));

    // handle Windows read-only files
    #[cfg(windows)]
    let _permissions = match DatabasePermissions::backup_and_make_writable(&db_path) {
        Ok(perms) => Some(perms),
        Err(e) => {
            let _ = tx.send(ZenEvent::Warning(format!("could not modify permissions for '{}': {}", display_name, e)));
            None
        }
    };

    let conn = match Connection::open(&db_path) {
        Ok(conn) => conn,
        Err(e) => {
            return Err(format!("failed to open database '{}': {}", display_name, e).into());
        }
    };

    let count: i64 = match conn.prepare(COUNT_QUERY).and_then(|mut stmt| stmt.query_row([], |row| row.get(0))) {
        Ok(count) => count,
        Err(e) => {
            return Err(format!("failed to count entries in '{}': {}", display_name, e).into());
        }
    };

    if count > 0 {
        let _ = tx.send(ZenEvent::LogMessage(format!("discovered {} restless data spirits in '{}'", count, display_name)));

        if let Err(e) = conn.execute(DELETE_QUERY, []) {
            return Err(format!("failed to delete entries from '{}': {}", display_name, e).into());
        }

        let _ = tx.send(ZenEvent::LogMessage(format!("peacefully guided {} data spirits to rest in '{}'", count, display_name)));
    } else {
        let _ = tx.send(ZenEvent::LogMessage(format!("no restless spirits found in '{}' - already harmonious", display_name)));
    }

    // restore permissions if we modified them
    #[cfg(windows)]
    if let Some(permissions) = _permissions {
        if let Err(e) = permissions.restore(&db_path) {
            let _ = tx.send(ZenEvent::Warning(format!("could not restore permissions for '{}': {}", display_name, e)));
        }
    }

    Ok(())
}
