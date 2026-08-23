//! Global config `~/.sbxm/config.yaml`。
//!
//! configは利用者設定だけを持つ。登録案件の索引は`registry.yaml`が持ち、責務を混ぜない。
//! configはtoken、secret、runtime状態を保存しない。
//!
//! fileが存在しないこと、および既知のoptional fieldが無いことは正常であり、default設定
//! として扱う。ただし、存在するconfigが構文不正、未知version、permission不正、symlink、
//! read失敗である場合はdefaultへfallbackせず拒否する。不正なconfigを自動修復しない。

mod config_location;
mod config_observation;
mod config_state;
mod config_version;
mod declaration;
mod document_version;
mod ensure_config_dir;
mod file_declaration;
mod global_config;
mod host_file_source;
mod invalid_value;
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
mod render;
mod replace_line;
mod sandbox_home_relative_path;
mod save_git_identity;
mod save_language;
mod serialized;
mod unknown_key_warnings;
mod write_config;

pub use config_location::ConfigLocation;
pub(crate) use config_observation::ConfigObservation;
pub use config_state::ConfigState;
pub use config_version::CONFIG_VERSION;
use declaration::declaration;
use document_version::DOCUMENT_VERSION;
pub use ensure_config_dir::ensure_config_dir;
pub use file_declaration::FileDeclaration;
pub use global_config::GlobalConfig;
pub use host_file_source::HostFileSource;
use invalid_value::invalid_value;
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
pub use render::render;
use replace_line::replace_line;
pub use sandbox_home_relative_path::SandboxHomeRelativePath;
pub use save_git_identity::save_git_identity;
pub use save_language::save_language;
pub(crate) use serialized::serialized;
use unknown_key_warnings::unknown_key_warnings;
use write_config::write_config;

#[cfg(test)]
#[path = "config_test.rs"]
mod config_test;
