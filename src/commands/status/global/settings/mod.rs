//! global stateの診断。
//!
//! configとregistryはread-onlyで観測する。不在は正常であり、自動作成も自動修復も
//! 行わない。登録案件を巡回しないため、登録数によって実行時間と出力行数が増えない。

mod check_config;
mod check_git_identity;
mod check_registry;
mod check_state_directory;
mod is_writable_dir;

pub(super) use check_config::check_config;
pub(super) use check_git_identity::check_git_identity;
pub(super) use check_registry::check_registry;
pub(super) use check_state_directory::check_state_directory;
pub(super) use is_writable_dir::is_writable_dir;
