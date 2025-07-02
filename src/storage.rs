use serde_json::{Map, Value};
use sha2::{Sha256, Digest};
use std::fs;
use std::path::Path;
use std::process::Command;
use uuid::Uuid;
use crate::utils::{Result, CleanerError, ErrorCollector, TELEMETRY_KEYS};
use tokio::sync::mpsc;
use crate::zen_garden::ZenEvent;

pub struct FilePermissions {
    was_readonly: bool,
    #[cfg(unix)]
    original_mode: u32,
}

impl FilePermissions {
    pub fn backup_and_make_writable(file_path: &Path) -> Result<Self> {
        let metadata = fs::metadata(file_path)?;
        let was_readonly = metadata.permissions().readonly();

        #[cfg(unix)]
        let original_mode = {
            use std::os::unix::fs::PermissionsExt;
            metadata.permissions().mode()
        };

        if was_readonly {
            #[cfg(windows)]
            {
                if Command::new("attrib").args(["-R", &file_path.to_string_lossy()]).status().is_err() {
                }
            }

            let mut permissions = metadata.permissions();
            permissions.set_readonly(false);
            fs::set_permissions(file_path, permissions)?;
        }

        Ok(FilePermissions {
            was_readonly,
            #[cfg(unix)]
            original_mode,
        })
    }

    pub fn restore(&self, file_path: &Path) -> Result<()> {
        if self.was_readonly {
            #[cfg(windows)]
            {
                if Command::new("attrib").args(["+R", &file_path.to_string_lossy()]).status().is_err() {
                }
            }

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let permissions = fs::Permissions::from_mode(self.original_mode);
                if fs::set_permissions(file_path, permissions).is_err() {
                }
            }

            #[cfg(not(unix))]
            {
                if let Ok(metadata) = fs::metadata(file_path) {
                    let mut permissions = metadata.permissions();
                    permissions.set_readonly(true);
                    if fs::set_permissions(file_path, permissions).is_err() {
                    }
                }
            }
        }
        Ok(())
    }
}

pub fn update_vscode_storage(directory: &Path, tx: &mpsc::UnboundedSender<ZenEvent>) -> Result<()> {
    let mut error_collector = ErrorCollector::new();

    // try to update storage.json
    if let Err(e) = update_storage_json(directory, tx) {
        let error = CleanerError::FileSystem {
            operation: "updating storage.json".to_string(),
            path: directory.join("storage.json").display().to_string(),
            source: e.to_string(),
        };
        error_collector.add_error(error.clone());
        let _ = tx.send(ZenEvent::DetailedError(error));
    }

    // try to update machine id file if it's a file
    if directory.is_file() {
        if let Err(e) = update_machine_id_file(directory, tx) {
            let error = CleanerError::FileSystem {
                operation: "updating machine id file".to_string(),
                path: directory.display().to_string(),
                source: e.to_string(),
            };
            error_collector.add_error(error.clone());
            let _ = tx.send(ZenEvent::DetailedError(error));
        }
    }

    // send error summary if there were any errors
    if error_collector.has_errors() {
        let _ = tx.send(ZenEvent::ErrorSummary(error_collector));
        return Err("storage update encountered errors".into());
    }

    Ok(())
}

fn update_storage_json(directory: &Path, tx: &mpsc::UnboundedSender<ZenEvent>) -> Result<()> {
    let storage_path = directory.join("storage.json");
    if !storage_path.exists() {
        let _ = tx.send(ZenEvent::LogMessage(format!("storage.json not found in {} - already pure", directory.display())));
        return Ok(());
    }

    let _ = tx.send(ZenEvent::LogMessage(format!("harmonizing energy patterns in: {}", storage_path.display())));

    let _permissions = match FilePermissions::backup_and_make_writable(&storage_path) {
        Ok(perms) => Some(perms),
        Err(e) => {
            let _ = tx.send(ZenEvent::Warning(format!("could not modify permissions for storage.json: {}", e)));
            None
        }
    };

    let content = match fs::read_to_string(&storage_path) {
        Ok(content) => content,
        Err(e) => {
            return Err(format!("failed to read storage.json: {}", e).into());
        }
    };

    let mut data: Map<String, Value> = match serde_json::from_str(&content) {
        Ok(data) => data,
        Err(e) => {
            let _ = tx.send(ZenEvent::Warning(format!("storage.json contains invalid json, creating new structure: {}", e)));
            Map::new()
        }
    };

    let mut updated_keys = 0;
    for &key in &TELEMETRY_KEYS {
        if let Some(old_value) = data.get(key) {
            let _ = tx.send(ZenEvent::LogMessage(format!("releasing old {}: {}", key, old_value.as_str().unwrap_or_default())));
        }

        let new_value = if key == "telemetry.devDeviceId" {
            Uuid::new_v4().to_string()
        } else {
            format!("{:x}", Sha256::digest(Uuid::new_v4().as_bytes()))
        };
        let _ = tx.send(ZenEvent::LogMessage(format!("manifesting new {}: {}", key, new_value)));
        data.insert(key.to_string(), Value::String(new_value));
        updated_keys += 1;
    }

    let json_content = match serde_json::to_string_pretty(&data) {
        Ok(content) => content,
        Err(e) => {
            return Err(format!("failed to serialize updated storage.json: {}", e).into());
        }
    };

    if let Err(e) = fs::write(&storage_path, json_content) {
        return Err(format!("failed to write updated storage.json: {}", e).into());
    }

    if let Some(permissions) = _permissions {
        if let Err(e) = permissions.restore(&storage_path) {
            let _ = tx.send(ZenEvent::Warning(format!("could not restore permissions for storage.json: {}", e)));
        }
    }

    let _ = tx.send(ZenEvent::LogMessage(format!("energy patterns successfully harmonized in storage ({} keys updated)", updated_keys)));
    Ok(())
}

fn update_machine_id_file(file_path: &Path, tx: &mpsc::UnboundedSender<ZenEvent>) -> Result<()> {
    let _ = tx.send(ZenEvent::LogMessage(format!("harmonizing essence in: {}", file_path.display())));

    let _permissions = if file_path.exists() {
        Some(FilePermissions::backup_and_make_writable(file_path)?)
    } else {
        None
    };

    if file_path.exists() {
        let old_uuid = fs::read_to_string(file_path).unwrap_or_default();
        if !old_uuid.is_empty() {
            let _ = tx.send(ZenEvent::LogMessage(format!("releasing old essence: {}", old_uuid.trim())));
        }
        let _ = fs::remove_file(file_path);
    }

    let new_uuid = Uuid::new_v4().to_string();
    let _ = tx.send(ZenEvent::LogMessage(format!("manifesting new essence: {}", new_uuid)));

    fs::write(file_path, &new_uuid)?;
    lock_file_permissions(file_path)?;

    let _ = tx.send(ZenEvent::LogMessage("essence successfully harmonized and protected".to_string()));
    Ok(())
}

pub fn lock_file_permissions(file_path: &Path) -> Result<()> {
    if !file_path.exists() {
        return Err(format!("File doesn't exist, can't lock: {}", file_path.display()).into());
    }

    let mut permissions = fs::metadata(file_path)?.permissions();
    permissions.set_readonly(true);

    if let Err(e) = fs::set_permissions(file_path, permissions) {
        return Err(format!("Failed to set readonly permissions: {}", e).into());
    }

    #[cfg(windows)]
    {
        if Command::new("attrib").args(["+R", &file_path.to_string_lossy()]).status().is_err() {
        }
    }

    #[cfg(unix)]
    {
        if Command::new("chmod").args(["444", &file_path.to_string_lossy()]).status().is_err() {
        }
    }

    Ok(())
}


