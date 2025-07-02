use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use crate::utils::MACHINE_ID;

pub fn find_vscode_storage_directories() -> Vec<PathBuf> {
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
        &[MACHINE_ID],
        &["data", MACHINE_ID],
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

/// Scan a directory for VSCode storage using the provided patterns
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
