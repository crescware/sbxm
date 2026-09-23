//! Global config `~/.sbxm/config.yaml`。
//!
//! configは利用者設定だけを持つ。登録案件の索引は`registry.yaml`が持ち、責務を混ぜない。
//! configはtoken、secret、runtime状態を保存しない。
//!
//! fileが存在しないこと、および既知のoptional fieldが無いことは正常であり、default設定
//! として扱う。ただし、存在するconfigが構文不正、未知version、permission不正、symlink、
//! read失敗である場合はdefaultへfallbackせず拒否する。不正なconfigを自動修復しない。

mod append_file_entry;
mod config_location;
mod config_observation;
mod config_state;
mod config_version;
mod declaration;
mod document_version;
mod edit_config;
mod ensure_config_dir;
mod file_declaration;
mod file_not_declared;
mod global_config;
mod host_file_source;
mod invalid_value;
mod known_file_keys;
mod known_top_level_keys;
mod load;
mod missing_field;
mod observe;
mod parse;
mod parse_files;
mod parse_git_identity;
mod raw_config;
mod raw_file;
mod read_existing;
mod remove_file_declaration;
mod remove_file_entry;
mod render;
mod sandbox_home_relative_path;
mod save_file_declaration;
mod save_git_identity;
mod save_language;
mod serialized;
mod set_top_level;
mod unknown_key_warnings;
mod write_config;

use append_file_entry::append_file_entry;
pub use config_location::ConfigLocation;
pub(crate) use config_observation::ConfigObservation;
pub use config_state::ConfigState;
pub use config_version::CONFIG_VERSION;
use declaration::declaration;
use document_version::DOCUMENT_VERSION;
use edit_config::edit_config;
pub use ensure_config_dir::ensure_config_dir;
pub use file_declaration::FileDeclaration;
pub use file_not_declared::file_not_declared;
pub use global_config::GlobalConfig;
pub use host_file_source::HostFileSource;
use invalid_value::invalid_value;
use known_file_keys::KNOWN_FILE_KEYS;
use known_top_level_keys::KNOWN_TOP_LEVEL_KEYS;
pub use load::load;
use missing_field::missing_field;
pub(crate) use observe::observe;
use parse::parse;
use parse_files::parse_files;
use parse_git_identity::parse_git_identity;
use raw_config::RawConfig;
use raw_file::RawFile;
use read_existing::read_existing;
pub use remove_file_declaration::remove_file_declaration;
use remove_file_entry::remove_file_entry;
pub use render::render;
pub use sandbox_home_relative_path::SandboxHomeRelativePath;
pub use save_file_declaration::save_file_declaration;
pub use save_git_identity::save_git_identity;
pub use save_language::save_language;
pub(crate) use serialized::serialized;
use set_top_level::set_top_level;
use unknown_key_warnings::unknown_key_warnings;
use write_config::write_config;

#[cfg(test)]
#[path = "config_test.rs"]
mod config_test;

#[cfg(test)]
#[path = "file_declarations_test.rs"]
mod file_declarations_test;
