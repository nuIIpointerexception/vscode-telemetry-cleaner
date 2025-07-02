use serde_json::{Map, Value};
use sha2::{Sha256, Digest};
use std::fs;
use std::path::Path;
use std::process::Command;
use uuid::Uuid;
use crate::utils::{Result, TELEMETRY_KEYS};
use tokio::sync::mpsc;
use crate::zen_garden::ZenEvent;

pub fn update_vscode_storage(directory: &Path, tx: &mpsc::UnboundedSender<ZenEvent>) -> Result<()> {
    update_storage_json(directory, tx)?;

    if directory.is_file() {
        update_machine_id_file(directory, tx)?;
    }

    Ok(())
}

fn update_storage_json(directory: &Path, tx: &mpsc::UnboundedSender<ZenEvent>) -> Result<()> {
    let storage_path = directory.join("storage.json");
    if !storage_path.exists() { return Ok(()); }

    let _ = tx.send(ZenEvent::LogMessage(format!("harmonizing energy patterns in: {}", storage_path.display())));

    let content = fs::read_to_string(&storage_path).unwrap_or_default();
    let mut data: Map<String, Value> = serde_json::from_str(&content).unwrap_or_default();

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
    }

    let json_content = serde_json::to_string_pretty(&data)?;
    fs::write(&storage_path, json_content)?;

    let _ = tx.send(ZenEvent::LogMessage("energy patterns successfully harmonized in storage".to_string()));
    Ok(())
}

fn update_machine_id_file(file_path: &Path, tx: &mpsc::UnboundedSender<ZenEvent>) -> Result<()> {
    let _ = tx.send(ZenEvent::LogMessage(format!("harmonizing essence in: {}", file_path.display())));

    if file_path.exists() {
        let old_uuid = fs::read_to_string(file_path).unwrap_or_default();
        if !old_uuid.is_empty() {
            let _ = tx.send(ZenEvent::LogMessage(format!("releasing old essence: {}", old_uuid.trim())));
        }
    }

    let new_uuid = Uuid::new_v4().to_string();
    let _ = tx.send(ZenEvent::LogMessage(format!("manifesting new essence: {}", new_uuid)));

    if file_path.exists() {
        let _ = fs::remove_file(file_path);
    }

    fs::write(file_path, &new_uuid)?;
    lock_file_permissions(file_path)?;

    let _ = tx.send(ZenEvent::LogMessage("essence successfully harmonized and protected".to_string()));
    Ok(())
}

pub fn lock_file_permissions(file_path: &Path) -> Result<()> {
    // Note: This function is called from update_machine_id_file, so no logging needed here

    if !file_path.exists() {
        return Err(format!("File doesn't exist, can't lock: {}", file_path.display()).into());
    }

    if cfg!(windows) {
        let _ = Command::new("attrib").args(["+R", &file_path.to_string_lossy()]).status();
    } else {
        let _ = Command::new("chmod").args(["444", &file_path.to_string_lossy()]).status();

        #[cfg(target_os = "macos")]
        let _ = Command::new("sudo").args(["chflags", "uchg", &file_path.to_string_lossy()]).status();
    }

    #[cfg(not(target_os = "macos"))]
    {
        let mut permissions = fs::metadata(file_path)?.permissions();
        permissions.set_readonly(true);
        fs::set_permissions(file_path, permissions)?;
    }

    // File successfully locked (no logging needed)
    Ok(())
}


