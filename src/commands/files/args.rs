use std::path::PathBuf;

/// `files`の引数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Args {
    /// 宣言を1件足す。
    Add {
        /// 利用者が指定したままのpath。相対pathはcurrent directoryから解決する。
        source: PathBuf,
        /// Sandboxの`agent` homeからの配置先。省略時はhomeからの相対pathを使う。
        destination: Option<String>,
    },
    /// 宣言を並べる。
    Ls,
    /// 配置先で指定した宣言を外す。
    Rm { destination: String },
}
