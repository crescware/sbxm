//! 認証probeだけに答え、その後のruntime観測は失敗するhost。
use std::path::Path;
use std::process::Command;

use crate::fake_tool::install_fake_tool;
use crate::outcome::{Checked, Required};

pub fn command(home: &Path) -> Checked<Command> {
    let bin = home.join("authenticated-bin");
    std::fs::create_dir_all(&bin).required_because("create the fake bin directory")?;
    install_fake_tool(
        &bin,
        "sbx",
        r#"read -r state < "$0.probe"
if [ "$*" = "ls --json" ] && [ "$state" = ready ]; then
    printf 'used\n' > "$0.probe"
    printf '[]\n'
    exit 0
fi
printf 'the sandbox runtime is unavailable\n' >&2
exit 1
"#,
    )?;
    std::fs::write(bin.join("sbx.probe"), b"ready\n")
        .required_because("the first probe succeeds")?;
    // hostのgitはversionだけを答える。ほかの起動へ届くtestは、このhostでは書かない。
    install_fake_tool(
        &bin,
        "git",
        "printf 'git is not available to this test\\n' >&2\nexit 1\n",
    )?;
    let mut command = Command::new(env!("CARGO_BIN_EXE_sbxm"));
    command
        .current_dir(home)
        .env("HOME", home)
        .env("LC_ALL", "C")
        .env_remove("LC_MESSAGES")
        .env_remove("LANG")
        .env("PATH", bin);
    Ok(command)
}
