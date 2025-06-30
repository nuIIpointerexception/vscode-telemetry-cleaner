use rusqlite::Connection;
use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;
use sha2::{Sha256, Digest};
use sysinfo::{System, Signal, ProcessesToUpdate};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const MACHINE_ID: &str = "machineId";
const KEYS: [&str; 3] = ["telemetry.machineId", "telemetry.devDeviceId", "telemetry.macMachineId"];
const COUNT_QUERY: &str = "SELECT COUNT(*) FROM ItemTable WHERE key LIKE '%augment%';";
const DELETE_QUERY: &str = "DELETE FROM ItemTable WHERE key LIKE '%augment%';";
const VSCODE_PROCESSES: [&str; 6] = ["code", "code.exe", "Code", "Code.exe", "Visual Studio Code", "code-insiders"];

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn find_dirs() -> Vec<PathBuf> {
    [dirs::config_dir(), dirs::home_dir(), dirs::data_dir()]
        .into_iter()
        .filter_map(|d| d)
        .flat_map(|base| {
            fs::read_dir(base).ok().into_iter().flatten()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map_or(false, |ft| ft.is_dir()))
                .flat_map(|e| {
                    [
                        e.path().join("User/globalStorage").join(MACHINE_ID),
                        e.path().join("data/User/globalStorage").join(MACHINE_ID),
                        e.path().join(MACHINE_ID),
                        e.path().join("data").join(MACHINE_ID),
                    ]
                })
        })
        .filter(|p| p.exists())
        .collect()
}

fn kill_vscode() -> Result<()> {
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, sysinfo::ProcessRefreshKind::everything());

    let mut killed = 0;
    for (_, process) in sys.processes() {
        let name = process.name().to_string_lossy().to_lowercase();
        if VSCODE_PROCESSES.iter().any(|&vs_name| {
            name.contains(&vs_name.to_lowercase()) ||
            (name.contains("code") && (name.contains("visual") || name.contains("studio")))
        }) {
            if process.kill_with(Signal::Term).unwrap_or(false) {
                killed += 1;
            } else if process.kill_with(Signal::Kill).unwrap_or(false) {
                killed += 1;
            }
        }
    }

    if killed > 0 {
        std::thread::sleep(std::time::Duration::from_secs(2));
    }

    Ok(())
}

fn run() -> Result<()> {
    kill_vscode()?;

    let dirs = find_dirs();
    if dirs.is_empty() {
        return Err("No VSCode installations found".into());
    }

    for dir in dirs {
        update_storage(&dir)?;
        clean_db(&dir)?;
    }
    Ok(())
}

fn update_storage(dir: &Path) -> Result<()> {
    let storage_path = dir.join("storage.json");

    if storage_path.exists() {
        let mut data: Map<String, Value> = fs::read_to_string(&storage_path)
            .ok()
            .and_then(|content| serde_json::from_str(&content).ok())
            .unwrap_or_default();

        for &key in &KEYS {
            let new_value = if key == "telemetry.devDeviceId" {
                Uuid::new_v4().to_string()
            } else {
                format!("{:x}", Sha256::digest(Uuid::new_v4().as_bytes()))
            };
            data.insert(key.to_string(), Value::String(new_value));
        }

        fs::write(&storage_path, serde_json::to_string_pretty(&data)?)?;
    }

    if dir.is_file() {
        let new_uuid = Uuid::new_v4().to_string();
        fs::write(dir, &new_uuid)?;

        let mut perms = fs::metadata(dir)?.permissions();
        perms.set_readonly(true);
        fs::set_permissions(dir, perms)?;
    }

    Ok(())
}

fn clean_db(dir: &Path) -> Result<()> {
    let db_path = dir.join("state.vscdb");

    if !db_path.exists() {
        return Ok(());
    }

    let conn = Connection::open(&db_path)?;
    let count: i64 = conn.query_row(COUNT_QUERY, [], |row| row.get(0))?;

    if count > 0 {
        conn.execute(DELETE_QUERY, [])?;
    }

    Ok(())
}
