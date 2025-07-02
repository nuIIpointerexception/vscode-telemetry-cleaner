use std::io::{self, Write};
use std::fmt;

// enhanced error types for better error handling and user feedback
#[derive(Debug, Clone)]
pub enum CleanerError {
    FileSystem {
        operation: String,
        path: String,
        source: String
    },
    Database {
        operation: String,
        path: String,
        source: String
    },
    Process {
        operation: String,
        process: String,
        source: String
    },
    Permission {
        operation: String,
        path: String,
        source: String
    },
    Json {
        operation: String,
        path: String,
        source: String
    },
    Terminal {
        operation: String,
        source: String
    },
    Unknown {
        operation: String,
        source: String
    },
}

impl fmt::Display for CleanerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CleanerError::FileSystem { operation, path, source } => {
                write!(f, "filesystem error during {}: {} ({})", operation, source, path)
            }
            CleanerError::Database { operation, path, source } => {
                write!(f, "database error during {}: {} ({})", operation, source, path)
            }
            CleanerError::Process { operation, process, source } => {
                write!(f, "process error during {}: {} ({})", operation, source, process)
            }
            CleanerError::Permission { operation, path, source } => {
                write!(f, "permission error during {}: {} ({})", operation, source, path)
            }
            CleanerError::Json { operation, path, source } => {
                write!(f, "json parsing error during {}: {} ({})", operation, source, path)
            }
            CleanerError::Terminal { operation, source } => {
                write!(f, "terminal error during {}: {}", operation, source)
            }
            CleanerError::Unknown { operation, source } => {
                write!(f, "unknown error during {}: {}", operation, source)
            }
        }
    }
}

impl std::error::Error for CleanerError {}

// error collection for continuing operations despite failures
#[derive(Debug, Clone, Default)]
pub struct ErrorCollector {
    pub errors: Vec<CleanerError>,
    pub warnings: Vec<String>,
}

impl ErrorCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_error(&mut self, error: CleanerError) {
        self.errors.push(error);
    }

    pub fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    pub fn warning_count(&self) -> usize {
        self.warnings.len()
    }

    pub fn get_summary(&self) -> String {
        match (self.error_count(), self.warning_count()) {
            (0, 0) => "all operations completed successfully".to_string(),
            (0, w) => format!("completed with {} warnings", w),
            (e, 0) => format!("completed with {} errors", e),
            (e, w) => format!("completed with {} errors and {} warnings", e, w),
        }
    }
}

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub type CleanerResult<T> = std::result::Result<T, CleanerError>;

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
