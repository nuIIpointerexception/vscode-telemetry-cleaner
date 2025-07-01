use rusqlite::Connection;
use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;
use sha2::{Sha256, Digest};
use sysinfo::{System, ProcessesToUpdate};
use sysinfo::Signal;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const MACHINE_ID: &str = "machineId";
const KEYS: [&str; 3] = ["telemetry.machineId", "telemetry.devDeviceId", "telemetry.macMachineId"];
const COUNT_QUERY: &str = "SELECT COUNT(*) FROM ItemTable WHERE key LIKE '%augment%';";
const DELETE_QUERY: &str = "DELETE FROM ItemTable WHERE key LIKE '%augment%'";
const VSCODE_PROCESSES: [&str; 14] = [
    "code", "code.exe", "Code", "Code.exe", 
    "code-insiders", "code-insiders.exe",
    "cursor", "cursor.exe", "Cursor", "Cursor.exe",
    "windsurf", "windsurf.exe", "trae", "trae.exe"
];

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn find_dirs() -> Vec<PathBuf> {
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    
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
        .filter(|p| p.exists() && seen.insert(p.clone()))
        .collect()
}


fn has_augment_data() -> bool {
    let dirs = find_dirs();
    for dir in dirs {
        let db_path = dir.join("state.vscdb");
        if db_path.exists() {
            if let Ok(conn) = Connection::open(&db_path) {
                if let Ok(count) = conn.query_row(COUNT_QUERY, [], |row| row.get::<_, i64>(0)) {
                    if count > 0 {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn kill_vscode() -> Result<()> {
    if !has_augment_data() {
        println!("No augment data found, skipping process termination");
        return Ok(());
    }

    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, sysinfo::ProcessRefreshKind::everything());
    let current_pid = std::process::id();

    let mut killed = 0;
    for (_, process) in sys.processes() {
        if process.pid().as_u32() == current_pid {
            continue;
        }
        
        let name = process.name().to_string_lossy();
        let exe_path = process.exe().map(|p| p.to_string_lossy().to_lowercase()).unwrap_or_default();
        
        let is_vscode_process = VSCODE_PROCESSES.iter().any(|&vs_name| {
            name.eq_ignore_ascii_case(vs_name)
        }) || exe_path.contains("microsoft vs code") 
           || exe_path.contains("cursor") 
           || exe_path.contains("code-insiders")
           || exe_path.contains("windsurf")
           || exe_path.contains("trae")
           || (exe_path.contains("code") && exe_path.contains("electron"));
        
        if is_vscode_process {
            println!("Terminating process: {} ({})", name, process.pid());
            if process.kill_with(Signal::Term).unwrap_or(false) {
                killed += 1;
            } else if process.kill_with(Signal::Kill).unwrap_or(false) {
                killed += 1;
            }
        }
    }

    if killed > 0 {
        println!("Successfully terminated {} processes", killed);
        std::thread::sleep(std::time::Duration::from_secs(2));
    } else {
        println!("No target processes found");
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
        let data: Map<String, Value> = fs::read_to_string(&storage_path)
            .ok()
            .and_then(|content| serde_json::from_str(&content).ok())
            .unwrap_or_default();

        let mut updated_data = data.clone();
        let mut changes = 0;
        
        for &key in &KEYS {
            let new_value = if key == "telemetry.devDeviceId" {
                Uuid::new_v4().to_string()
            } else {
                format!("{:x}", Sha256::digest(Uuid::new_v4().as_bytes()))
            };
            updated_data.insert(key.to_string(), Value::String(new_value));
            changes += 1;
        }
        
        if changes > 0 {
            println!("Updated {} telemetry keys in: {}", changes, storage_path.display());
            fs::write(&storage_path, serde_json::to_string_pretty(&updated_data)?)?
        }
    }

    if dir.is_file() {
        let new_uuid = Uuid::new_v4().to_string();
        println!("Updating machine ID file: {}", dir.display());
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
    let affected = conn.execute(DELETE_QUERY, [])?;
    
    if affected > 0 {
        println!("Removed {} telemetry entries from: {}", affected, db_path.display());
    }

    Ok(())
}
