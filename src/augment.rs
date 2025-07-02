use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use crate::utils::{Result, CleanerError, ErrorCollector};

#[derive(Debug, Clone)]
pub struct AugmentCleaningResult {
    pub processes_terminated: Vec<String>,
    pub directories_found: Vec<PathBuf>,
    pub databases_cleaned: Vec<String>,
    pub storage_updated: Vec<String>,
    pub errors: ErrorCollector,
}

impl AugmentCleaningResult {
    pub fn new() -> Self {
        Self {
            processes_terminated: Vec::new(),
            directories_found: Vec::new(),
            databases_cleaned: Vec::new(),
            storage_updated: Vec::new(),
            errors: ErrorCollector::new(),
        }
    }
}

/// Find VSCode/Augment storage directories across different platforms and installations
pub fn find_augment_storage_directories() -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut base_dirs = vec![dirs::config_dir(), dirs::home_dir(), dirs::data_dir()];

    if let Some(home) = dirs::home_dir() {
        base_dirs.push(Some(home.join(".vscode")));

        #[cfg(target_os = "linux")]
        {
            base_dirs.push(Some(home.join("snap/code/common/.config")));
            base_dirs.push(Some(home.join(".var/app/com.visualstudio.code/config")));
            base_dirs.push(Some(home.join(".var/app/com.visualstudio.code-insiders/config")));
        }

        #[cfg(target_os = "macos")]
        {
            if let Some(app_support) = dirs::config_dir() {
                base_dirs.push(Some(app_support.join("Code - Insiders")));
                base_dirs.push(Some(app_support.join("Cursor")));
                base_dirs.push(Some(app_support.join("VSCodium")));
            }
        }
    }

    let global_patterns = [
        &["User", "globalStorage"] as &[&str],
        &["data", "User", "globalStorage"],
        &[crate::utils::MACHINE_ID],
        &["data", crate::utils::MACHINE_ID],
    ];

    let workspace_patterns = [
        &["User", "workspaceStorage"] as &[&str],
        &["data", "User", "workspaceStorage"],
    ];

    base_dirs
        .into_iter()
        .filter_map(|dir| dir)
        .flat_map(|base| scan_storage(&base, &global_patterns, &workspace_patterns))
        .filter(|path| path.exists() && seen.insert(path.clone()))
        .collect()
}

/// Scan a directory for VSCode/Augment storage using the provided patterns
fn scan_storage(
    base_dir: &PathBuf,
    global_patterns: &[&[&str]],
    workspace_patterns: &[&[&str]],
) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(base_dir) else { return Vec::new(); };

    entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
        .flat_map(|entry| {
            let path = entry.path();

            let global_paths: Vec<PathBuf> = global_patterns.iter()
                .map(|pattern| pattern.iter().fold(path.clone(), |p, seg| p.join(seg)))
                .collect();

            let workspace_paths: Vec<PathBuf> = workspace_patterns.iter()
                .flat_map(|pattern| {
                    let workspace_base = pattern.iter().fold(path.clone(), |p, seg| p.join(seg));
                    if !workspace_base.exists() { return Vec::new(); }

                    let Ok(entries) = fs::read_dir(&workspace_base) else { return Vec::new(); };
                    entries
                        .filter_map(|entry| entry.ok())
                        .filter(|entry| entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
                        .map(|entry| entry.path())
                        .collect()
                })
                .collect();

            global_paths.into_iter().chain(workspace_paths)
        })
        .collect()
}

/// Terminate VSCode processes that might be using Augment extension
pub fn terminate_augment_processes() -> Result<Vec<String>> {
    let process_names = vec![
        "code", "Code", "code.exe", "Code.exe",
        "code-insiders", "code-insiders.exe",
        "vscodium", "VSCodium", "vscodium.exe",
        "cursor", "Cursor", "cursor.exe", "Cursor.exe"
    ];
    
    let mut terminated = Vec::new();
    
    for process_name in process_names {
        #[cfg(target_os = "windows")]
        {
            let output = std::process::Command::new("tasklist")
                .args(&["/FI", &format!("IMAGENAME eq {}", process_name)])
                .output();
                
            if let Ok(output) = output {
                let output_str = String::from_utf8_lossy(&output.stdout);
                if output_str.contains(process_name) {
                    let result = std::process::Command::new("taskkill")
                        .args(&["/F", "/IM", process_name])
                        .output();
                    
                    if result.is_ok() {
                        terminated.push(process_name.to_string());
                    }
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
                    let result = std::process::Command::new("pkill")
                        .args(&["-f", process_name])
                        .output();
                    
                    if result.is_ok() {
                        terminated.push(process_name.to_string());
                    }
                }
            }
        }
    }
    
    if !terminated.is_empty() {
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
    
    Ok(terminated)
}

/// Clean Augment extension data from VSCode databases
pub fn clean_augment_databases(directories: &[PathBuf]) -> Result<Vec<String>> {
    let mut cleaned = Vec::new();

    for directory in directories {
        // Create a dummy channel since we're not using the UI here
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();

        match crate::database::clean_vscode_databases(directory, &tx) {
            Ok(_) => cleaned.push(directory.to_string_lossy().to_string()),
            Err(_) => continue, // Skip failed directories
        }
    }

    Ok(cleaned)
}

/// Update VSCode storage to remove Augment extension traces
pub fn update_augment_storage(directories: &[PathBuf]) -> Result<Vec<String>> {
    let mut updated = Vec::new();

    for directory in directories {
        // Create a dummy channel since we're not using the UI here
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();

        match crate::storage::update_vscode_storage(directory, &tx) {
            Ok(_) => updated.push(directory.to_string_lossy().to_string()),
            Err(_) => continue, // Skip failed directories
        }
    }

    Ok(updated)
}

/// Perform complete Augment extension cleaning
pub async fn clean_augment_extension(args: &crate::cli::CliArgs) -> Result<AugmentCleaningResult> {
    let mut result = AugmentCleaningResult::new();
    
    // Step 1: Terminate processes (only if not disabled by no_terminate)
    if !args.no_terminate {
        match terminate_augment_processes() {
            Ok(terminated) => result.processes_terminated = terminated,
            Err(e) => result.errors.add_error(CleanerError::Process {
                operation: "terminate".to_string(),
                process: "augment_processes".to_string(),
                source: e.to_string(),
            }),
        }
    }
    
    // Step 2: Find storage directories
    result.directories_found = find_augment_storage_directories();
    
    // Step 3: Clean databases (only if not disabled by no_signout)
    if !args.no_signout {
        match clean_augment_databases(&result.directories_found) {
            Ok(cleaned) => result.databases_cleaned = cleaned,
            Err(e) => result.errors.add_error(CleanerError::Database {
                operation: "clean".to_string(),
                path: "augment_databases".to_string(),
                source: e.to_string(),
            }),
        }
    }
    
    // Step 4: Update storage
    match update_augment_storage(&result.directories_found) {
        Ok(updated) => result.storage_updated = updated,
        Err(e) => result.errors.add_error(CleanerError::Json {
            operation: "update_storage".to_string(),
            path: "augment_storage".to_string(),
            source: e.to_string(),
        }),
    }
    
    Ok(result)
}
