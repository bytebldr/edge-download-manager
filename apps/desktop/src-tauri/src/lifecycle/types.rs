use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeMode {
    Normal,
    Setup,
    Repair,
    Update,
    Uninstall,
}

impl RuntimeMode {
    pub fn from_args(args: &[String]) -> Self {
        for arg in args {
            let lower = arg.to_lowercase();
            if lower == "--setup" {
                return RuntimeMode::Setup;
            } else if lower == "--repair" || lower == "-repair" {
                return RuntimeMode::Repair;
            } else if lower == "--update" || lower == "-update" || lower == "/update" {
                return RuntimeMode::Update;
            } else if lower == "--uninstall" || lower == "-uninstall" {
                return RuntimeMode::Uninstall;
            }
        }
        RuntimeMode::Normal
    }

    pub fn is_lifecycle_wizard(&self) -> bool {
        !matches!(self, RuntimeMode::Normal)
    }

    pub fn hash_route(&self) -> Option<&'static str> {
        match self {
            RuntimeMode::Setup => Some("#setup"),
            RuntimeMode::Repair => Some("#repair"),
            RuntimeMode::Update => Some("#update"),
            RuntimeMode::Uninstall => Some("#uninstall"),
            RuntimeMode::Normal => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallOptions {
    pub target_dir: String,
    pub create_desktop_shortcut: bool,
    pub start_on_boot: bool,
    pub enable_browser_integration: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairPlan {
    pub repair_browser: bool,
    pub repair_shortcuts: bool,
    pub verify_binaries: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairReport {
    pub success: bool,
    pub repaired_browser: bool,
    pub repaired_shortcuts: bool,
    pub verified_binaries: bool,
    pub details: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExistingInstallationInfo {
    pub is_installed: bool,
    pub install_dir: Option<String>,
    pub version: Option<String>,
    pub publisher: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub release_name: String,
    pub release_notes: String,
    pub published_at: String,
    pub download_url: Option<String>,
    pub asset_name: Option<String>,
    pub asset_size: u64,
    pub html_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtaProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub percent: f64,
    pub status: String,
}
