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

/// `sbx version`への答え。sbxmが求める最低versionのCLIの出力であり、最低versionを
/// 上げたらこのfileだけを合わせる。一致しなければsbxmのunit testが落ちる。
const SBX_VERSION: &str = include_str!("../fixtures/sbx-version");

/// `git --version`への答え。sbxmが求める最低versionのgitの出力であり、最低versionを
/// 上げたらこのfileだけを合わせる。一致しなければsbxmのunit testが落ちる。
const GIT_VERSION: &str = include_str!("../fixtures/git-version");

/// `bin`へ、`program`として起動されると`script`を走らせる代役を置く。
///
/// 同じ名前へもう一度置くと、symlinkはそのままに振る舞いだけを差し替える。
///
/// `sbx`と`git`の代役は、`script`が何を書いても`sbx version`と`git --version`へ
/// `SBX_VERSION`と`GIT_VERSION`を答える。sbxmはそれぞれを使うcommandの最初にversionを
/// 確かめるため、台本ごとには書かせない。
pub fn install_fake_tool(bin: &Path, program: &str, script: &str) -> Checked {
    let script = match program {
        "sbx" => version_answer("version", SBX_VERSION, script),
        "git" => version_answer("--version", GIT_VERSION, script),
        _ => script.to_string(),
    };
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

/// 引数が`asked`だけの起動へ`answer`を返し、それ以外は`script`へ渡す台本。
fn version_answer(asked: &str, answer: &str, script: &str) -> String {
    format!("if [ \"$*\" = {asked} ]; then\n    printf '%s' '{answer}'\n    exit 0\nfi\n{script}")
}
