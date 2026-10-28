//! hostの道具（`sbx`、`git`）のversion確認が通った実行ファイルの記録。
//!
//! version確認は実行ファイルごとに一度でよい。最低versionを満たすと分かった実行ファイルの
//! 識別情報を`~/.sbxm/host-tools.yaml`へ記録し、次の実行で同じ実行ファイルが同じ識別情報の
//! まま見つかれば、確認を省略する。実行ファイルが変われば識別情報が一致せず、確認が走る。
//! 記録に期限は無い。
//!
//! 記録はsbxmの内部状態であり、どのcommandも一律に「記録があれば使い、確認が通れば書く」。
//! 記録を使えなかった理由と書けなかった理由は`--verbose`のtraceだけに残し、確認そのものの
//! 結果は変えない。

mod apple_git_shim;
mod digest;
mod host_tools_version;
mod load;
mod loaded;
mod not_reusable;
mod parse;
mod raw_host_tools;
mod raw_tool;
mod record;
mod recorded_tool;
mod recorded_version;
mod render;
mod reusable;
mod unsaved;
mod unused;

use apple_git_shim::APPLE_GIT_SHIM;
use digest::digest;
use host_tools_version::HOST_TOOLS_VERSION;
use load::load;
use loaded::Loaded;
use not_reusable::NotReusable;
use parse::parse;
use raw_host_tools::RawHostTools;
use raw_tool::RawTool;
pub use record::record;
use recorded_tool::RecordedTool;
pub use recorded_version::recorded_version;
use render::render;
use reusable::reusable;
use unsaved::Unsaved;
use unused::Unused;

#[cfg(test)]
#[path = "host_tools_test.rs"]
mod host_tools_test;
