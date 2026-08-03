//! symlinkを解決した絶対pathを持つ、一時的な`$HOME`。
//!
//! macOSの`TMPDIR`は`/var/...`から`/private/var/...`へのsymlinkの下にある。sbxmが記録
//! するpathは、子processの`current_dir`(`getcwd`)がsymlinkを解決した実pathを返すために
//! 実pathとなる。fixtureがsymlinkを含むpathからそのまま期待値を組み立てると、宣言した
//! 文字列とsbxmが記録した実pathが食い違う。ここで一度解決しておけば、以降のすべての
//! 参照が同じ実pathを指す。
//!
//! 6本のtest binary(`cli`、`status`、`prompt_pty`、`prompt_terminal`、`host`、
//! `lifecycle`)がこれを取り込む。temp directoryを持たないbinaryへは足さない。

use std::path::{Path, PathBuf};

use crate::outcome::{Checked, Required};

/// 一時的な`$HOME`。`path()`はsymlinkを解決した実pathを返す。
pub struct TempHome {
    _dir: tempfile::TempDir,
    path: PathBuf,
}

impl TempHome {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// 一時的な`$HOME`を作る。
pub fn temp_home() -> Checked<TempHome> {
    let dir = tempfile::tempdir().required_because("temporary home")?;
    let path = std::fs::canonicalize(dir.path())
        .required_because("the temporary home resolves to a real path")?;
    Ok(TempHome { _dir: dir, path })
}
