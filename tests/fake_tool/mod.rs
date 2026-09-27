//! sbxmが`PATH`から起動するhost toolの代役を置く。
//!
//! 実行可能fileは実行時に書かない。書いた直後のfileは、別threadのtestがforkした子が
//! 書き込み端を持っている間、`ETXTBSY`でexecできない。execされるのはrepositoryにある
//! `tests/fixtures/fake_tool.sh`だけであり、testは道具の名前でそこへのsymlinkを置き、
//! 振る舞いを`<名前>.sh`として実行bitを持たないfileへ書く。
//!
//! 7本のtest binary(`cli`、`status`、`command_lifecycle`、`prompt_pty`、`prompt_terminal`、
//! `host`、`lifecycle`)がこれを取り込む。`cli`と`status`は`authenticated_host`を通して使う。

use std::path::Path;

use crate::outcome::{Checked, Required};

/// `bin`へ、`program`として起動されると`script`を走らせる代役を置く。
///
/// 同じ名前へもう一度置くと、symlinkはそのままに振る舞いだけを差し替える。
pub fn install_fake_tool(bin: &Path, program: &str, script: &str) -> Checked {
    std::fs::write(bin.join(format!("{program}.sh")), script)
        .required_because("the fake tool's script is written")?;
    let link = bin.join(program);
    if std::fs::symlink_metadata(&link).is_err() {
        let wrapper = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake_tool.sh");
        std::os::unix::fs::symlink(wrapper, &link)
            .required_because("the fake tool is linked under its name")?;
    }
    Ok(())
}
