//! Docker Engineへのアクセス。
//!
//! `"docker"`というprogram名で`CommandSpec`/`TerminalCommand`を組み立てるのはこの
//! moduleだけとする。他のmoduleはここが公開する操作を通してのみdockerを使う。

mod build;
mod exists;
mod inspect;
mod read_server_version;
mod require_reachable;
mod save;
mod version_probe;

pub use build::build;
pub use exists::exists;
pub use inspect::inspect;
pub use read_server_version::read_server_version;
pub use require_reachable::require_reachable;
pub use save::save;
use version_probe::version_probe;
