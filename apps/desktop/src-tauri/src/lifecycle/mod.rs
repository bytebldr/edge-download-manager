pub mod install;
pub mod manager;
pub mod platform;
pub mod repair;
pub mod types;
pub mod uninstall;
pub mod update;

pub use install::{
    check_existing_installation, get_default_install_dir, perform_native_installation,
};
pub use manager::{LifecycleManager, LifecycleState, LifecycleStatus};
pub use repair::perform_native_repair;
pub use types::{
    ExistingInstallationInfo, InstallOptions, OtaProgress, RepairPlan, RepairReport, RuntimeMode,
    UpdateCheckResult,
};
pub use uninstall::{perform_native_uninstallation, perform_silent_uninstallation};
pub use update::{
    check_for_updates, get_app_version, install_update, is_newer_version, open_installer_folder,
    parse_version,
};
