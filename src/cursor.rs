use std::fs;
use std::path::PathBuf;
use serde_json::{json, Value};
use uuid::Uuid;
use crate::utils::{Result, CleanerError, ErrorCollector};

#[derive(Debug, Clone)]
pub struct CursorConfig {
    pub telemetry_machine_id: String,
    pub telemetry_mac_machine_id: String,
    pub telemetry_dev_device_id: String,
    pub telemetry_sqm_id: String,
}

#[derive(Debug, Clone)]
pub struct CursorCleaningResult {
    pub processes_terminated: Vec<String>,
    pub directories_removed: Vec<PathBuf>,
    pub config_updated: bool,
    pub backup_created: Option<PathBuf>,
    pub errors: ErrorCollector,
}

impl CursorCleaningResult {
    pub fn new() -> Self {
        Self {
            processes_terminated: Vec::new(),
            directories_removed: Vec::new(),
            config_updated: false,
            backup_created: None,
            errors: ErrorCollector::new(),
        }
    }
}

/// Find Cursor storage directories (similar to VSCode storage directories)
pub fn find_cursor_storage_directories() -> Vec<PathBuf> {
    let mut cursor_dirs = Vec::new();

    // Windows: AppData\Roaming\Cursor\User\globalStorage and workspaceStorage
    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = dirs::config_dir() {
            let cursor_global = appdata.join("Cursor/User/globalStorage");
            if cursor_global.exists() {
                cursor_dirs.push(cursor_global);
            }

            let cursor_workspace = appdata.join("Cursor/User/workspaceStorage");
            if cursor_workspace.exists() {
                // Add all workspace directories
                if let Ok(entries) = std::fs::read_dir(&cursor_workspace) {
                    for entry in entries.filter_map(|e| e.ok()) {
                        if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                            cursor_dirs.push(entry.path());
                        }
                    }
                }
            }
        }
    }

    // macOS: ~/Library/Application Support/Cursor/User/globalStorage
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = dirs::home_dir() {
            let cursor_global = home.join("Library/Application Support/Cursor/User/globalStorage");
            if cursor_global.exists() {
                cursor_dirs.push(cursor_global);
            }

            let cursor_workspace = home.join("Library/Application Support/Cursor/User/workspaceStorage");
            if cursor_workspace.exists() {
                if let Ok(entries) = std::fs::read_dir(&cursor_workspace) {
                    for entry in entries.filter_map(|e| e.ok()) {
                        if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                            cursor_dirs.push(entry.path());
                        }
                    }
                }
            }
        }
    }

    // Linux: ~/.config/Cursor/User/globalStorage
    #[cfg(target_os = "linux")]
    {
        if let Some(home) = dirs::home_dir() {
            let cursor_global = home.join(".config/Cursor/User/globalStorage");
            if cursor_global.exists() {
                cursor_dirs.push(cursor_global);
            }

            let cursor_workspace = home.join(".config/Cursor/User/workspaceStorage");
            if cursor_workspace.exists() {
                if let Ok(entries) = std::fs::read_dir(&cursor_workspace) {
                    for entry in entries.filter_map(|e| e.ok()) {
                        if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                            cursor_dirs.push(entry.path());
                        }
                    }
                }
            }
        }
    }

    cursor_dirs
}

pub fn find_cursor_directories() -> Vec<PathBuf> {
    let mut cursor_dirs = Vec::new();
    
    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = dirs::config_dir() {
            let cursor_path = appdata.join("Cursor");
            if cursor_path.exists() {
                cursor_dirs.push(cursor_path);
            }
        }
        
        if let Some(home) = dirs::home_dir() {
            let cursor_home = home.join(".cursor");
            if cursor_home.exists() {
                cursor_dirs.push(cursor_home);
            }
        }
        
        if let Some(local_appdata) = std::env::var("LOCALAPPDATA").ok() {
            let cursor_local = PathBuf::from(local_appdata).join("cursor");
            if cursor_local.exists() {
                cursor_dirs.push(cursor_local);
            }
        }
    }
    
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = dirs::home_dir() {
            let cursor_support = home.join("Library/Application Support/Cursor");
            if cursor_support.exists() {
                cursor_dirs.push(cursor_support);
            }
            
            let cursor_home = home.join(".cursor");
            if cursor_home.exists() {
                cursor_dirs.push(cursor_home);
            }
        }
    }
    
    #[cfg(target_os = "linux")]
    {
        if let Some(home) = dirs::home_dir() {
            let cursor_config = home.join(".config/Cursor");
            if cursor_config.exists() {
                cursor_dirs.push(cursor_config);
            }
            
            let cursor_home = home.join(".cursor");
            if cursor_home.exists() {
                cursor_dirs.push(cursor_home);
            }
        }
    }
    
    cursor_dirs
}

pub fn get_cursor_storage_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = dirs::config_dir() {
            let storage_path = appdata.join("Cursor/User/globalStorage/storage.json");
            return Some(storage_path);
        }
    }
    
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = dirs::home_dir() {
            let storage_path = home.join("Library/Application Support/Cursor/User/globalStorage/storage.json");
            return Some(storage_path);
        }
    }
    
    #[cfg(target_os = "linux")]
    {
        if let Some(home) = dirs::home_dir() {
            let storage_path = home.join(".config/Cursor/User/globalStorage/storage.json");
            return Some(storage_path);
        }
    }
    
    None
}

pub fn generate_cursor_ids() -> CursorConfig {
    let mac_machine_id = Uuid::new_v4().to_string();
    let dev_device_id = Uuid::new_v4().to_string();
    let sqm_id = format!("{{{}}}", Uuid::new_v4().to_string().to_uppercase());

    let prefix = "auth0|user_";
    let prefix_bytes = prefix.as_bytes();
    let prefix_hex = prefix_bytes.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();

    // use uuid for random bytes instead of rand crate
    let random_uuid = Uuid::new_v4();
    let random_bytes = random_uuid.as_bytes();
    let random_hex = random_bytes.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();

    let machine_id = format!("{}{}", prefix_hex, random_hex);

    CursorConfig {
        telemetry_machine_id: machine_id,
        telemetry_mac_machine_id: mac_machine_id,
        telemetry_dev_device_id: dev_device_id,
        telemetry_sqm_id: sqm_id,
    }
}

pub fn clean_cursor_config(config: &CursorConfig) -> Result<bool> {
    let storage_path = match get_cursor_storage_path() {
        Some(path) => path,
        None => return Ok(false),
    };
    
    if !storage_path.exists() {
        return Ok(false);
    }
    
    let backup_dir = storage_path.parent()
        .unwrap_or(&storage_path)
        .join("backups");
    
    if !backup_dir.exists() {
        fs::create_dir_all(&backup_dir)?;
    }
    
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let backup_path = backup_dir.join(format!("storage.json.backup_{}", timestamp));
    
    fs::copy(&storage_path, &backup_path)?;
    
    let content = fs::read_to_string(&storage_path)?;
    let mut storage_json: Value = serde_json::from_str(&content)
        .unwrap_or_else(|_| json!({}));
    
    if let Some(obj) = storage_json.as_object_mut() {
        obj.insert("telemetry.machineId".to_string(), json!(config.telemetry_machine_id));
        obj.insert("telemetry.macMachineId".to_string(), json!(config.telemetry_mac_machine_id));
        obj.insert("telemetry.devDeviceId".to_string(), json!(config.telemetry_dev_device_id));
        obj.insert("telemetry.sqmId".to_string(), json!(config.telemetry_sqm_id));
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        obj.insert("lastModified".to_string(), json!(timestamp.to_string()));
    }
    
    let updated_content = serde_json::to_string_pretty(&storage_json)?;
    fs::write(&storage_path, updated_content)?;
    
    Ok(true)
}

pub fn terminate_cursor_processes() -> Result<bool> {
    let process_names = vec!["Cursor", "cursor", "Cursor.exe", "cursor.exe"];
    let mut terminated = false;
    
    for process_name in process_names {
        #[cfg(target_os = "windows")]
        {
            let output = std::process::Command::new("tasklist")
                .args(&["/FI", &format!("IMAGENAME eq {}", process_name)])
                .output();
                
            if let Ok(output) = output {
                let output_str = String::from_utf8_lossy(&output.stdout);
                if output_str.contains(process_name) {
                    let _ = std::process::Command::new("taskkill")
                        .args(&["/F", "/IM", process_name])
                        .output();
                    terminated = true;
                }
            }
        }
        
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            let output = std::process::Command::new("pgrep")
                .arg(process_name)
                .output();
                
            if let Ok(output) = output {
                if !output.stdout.is_empty() {
                    let _ = std::process::Command::new("pkill")
                        .args(&["-f", process_name])
                        .output();
                    terminated = true;
                }
            }
        }
    }
    
    if terminated {
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
    
    Ok(terminated)
}

pub fn remove_cursor_directories() -> Result<Vec<PathBuf>> {
    let cursor_dirs = find_cursor_directories();
    let mut removed_dirs = Vec::new();

    for dir in cursor_dirs {
        if dir.exists() {
            match fs::remove_dir_all(&dir) {
                Ok(_) => removed_dirs.push(dir),
                Err(_) => continue,
            }
        }
    }

    Ok(removed_dirs)
}

/// Perform complete Cursor IDE cleaning
pub async fn clean_cursor_ide(args: &crate::cli::CliArgs) -> Result<CursorCleaningResult> {
    let mut result = CursorCleaningResult::new();

    // Step 1: Terminate Cursor processes (only if not disabled by no_terminate)
    if !args.no_terminate {
        match terminate_cursor_processes() {
            Ok(terminated) => {
                if terminated {
                    result.processes_terminated.push("Cursor".to_string());
                }
            },
            Err(e) => result.errors.add_error(CleanerError::Process {
                operation: "terminate".to_string(),
                process: "Cursor".to_string(),
                source: e.to_string(),
            }),
        }
    }

    // Step 2: Generate new IDs
    let new_config = generate_cursor_ids();

    // Step 3: Find Cursor storage directories (like VSCode)
    let cursor_storage_dirs = find_cursor_storage_directories();
    result.directories_removed = cursor_storage_dirs.clone(); // Track found directories

    // Step 4: Update storage files (like VSCode storage.json updates)
    for directory in &cursor_storage_dirs {
        // Create a dummy channel since we're not using the UI here
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();

        match crate::storage::update_vscode_storage(directory, &tx) {
            Ok(_) => result.config_updated = true,
            Err(e) => result.errors.add_error(CleanerError::Json {
                operation: "update_storage".to_string(),
                path: directory.display().to_string(),
                source: e.to_string(),
            }),
        }
    }

    // Step 5: Clean databases (only if not disabled by no_signout)
    if !args.no_signout {
        for directory in &cursor_storage_dirs {
            // Create a dummy channel since we're not using the UI here
            let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();

            match crate::database::clean_vscode_databases(directory, &tx) {
                Ok(_) => {}, // Success - databases cleaned
                Err(e) => result.errors.add_error(CleanerError::Database {
                    operation: "clean_databases".to_string(),
                    path: directory.display().to_string(),
                    source: e.to_string(),
                }),
            }
        }
    }

    // Step 6: Update Cursor-specific configuration (legacy approach)
    match clean_cursor_config(&new_config) {
        Ok(updated) => {
            if updated {
                result.config_updated = true;
            }
        },
        Err(e) => result.errors.add_error(CleanerError::Json {
            operation: "update_cursor_config".to_string(),
            path: "cursor_storage.json".to_string(),
            source: e.to_string(),
        }),
    }

    Ok(result)
}
