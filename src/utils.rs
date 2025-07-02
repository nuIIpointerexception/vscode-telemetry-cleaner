use std::io::{self, Write};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub const MACHINE_ID: &str = "machineId";

pub const TELEMETRY_KEYS: [&str; 4] = [
    "telemetry.machineId",
    "telemetry.devDeviceId",
    "telemetry.macMachineId",
    "storage.serviceMachineId"
];

pub const COUNT_QUERY: &str = "SELECT COUNT(*) FROM ItemTable WHERE key LIKE '%augment%';";
pub const DELETE_QUERY: &str = "DELETE FROM ItemTable WHERE key LIKE '%augment%';";

pub const VSCODE_PROCESSES: [&str; 14] = [
    "code", "code.exe", "Code", "Code.exe",
    "code-insiders", "code-insiders.exe",
    "cursor", "cursor.exe", "Cursor", "Cursor.exe",
    "windsurf", "windsurf.exe", "trae", "trae.exe"
];

pub fn pause_for_user_input(no_pause: bool) {
    if no_pause { return; }
    print!("\nPress Enter to exit...");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut String::new()).unwrap();
}
