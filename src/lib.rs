pub mod cli;
pub mod database;
pub mod filesystem;
pub mod process;
pub mod storage;
pub mod utils;
pub mod zen_garden;

pub use cli::CliArgs;
pub use database::clean_vscode_databases;
pub use filesystem::find_vscode_storage_directories;
pub use process::terminate_vscode_processes;
pub use storage::{update_vscode_storage, lock_file_permissions};
pub use utils::{Result, pause_for_user_input};
pub use zen_garden::ZenGarden;

// Legacy function removed - zen garden is now the default interface
