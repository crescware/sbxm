//! デザインシステムの境界。
//!
//! 規約は文書だけでは守れない。ここが検出するのは「どこで何を組み立ててよいか」であり、
//! 見た目そのものは`src/design`のinvariant testが持つ。
//!
//! 検出は禁止APIと明確なprefixに限る。文字列検索だけでは`docker`のような語と実行
//! commandを完全には区別できないため、曖昧な判定を足して誤検出を増やさない。

mod outcome;

use outcome::{Checked, Required};

use std::path::{Path, PathBuf};

/// 描画を組み立ててよい唯一の場所。
const DESIGN: &str = "src/design";

/// color modeの受け入れ語彙を定義してよい唯一のproduction file。
const COLOR_MODE: &str = "src/design/policy/color_mode.rs";

/// command line adapter。
const COMMAND_LINE: &str = "src/boundary/command_line";

/// 具体的なterminal adapterを置いてよい唯一のmodule。
const TERMINAL_ADAPTER: &str = "src/boundary/terminal/";

/// clapへ接続する具体adapterを置いてよい唯一のmodule。
const CLAP_ADAPTER: &str = "src/boundary/command_line/clap/";

/// ANSI escape sequenceを生成してよい唯一のfile。
const RENDERER: &str = "src/design/painter.rs";

/// 子processへ端末のstreamを直接渡してよい唯一のfile。
const CONFIGURE: &str = "src/boundary/host/configure.rs";

/// `sbx`の出力を端末へ出す起動を組み立ててよい唯一のfile。
const SBX_RELAY: &str = "src/support/sandbox/relayed.rs";

/// 確認promptにだけ答える`sbx`起動を組み立ててよい唯一のfile。
const SBX_PTY_CONFIRM: &str = "src/support/sandbox/remove_confirmed.rs";

/// Dockerのprocessを組み立ててよい唯一のmodule。
const DOCKER_SUPPORT: &str = "src/support/docker/";

/// 外部toolのbyteを運ぶmodule。
const RELAY: &str = "src/boundary/host";

/// session leaseのpathへ触れてよい唯一の場所。
///
/// `Locked`のmethodだけがこのpathへlockを取ることで、project lockを保持した
/// `Locked`を経由しない限りsession leaseを取得できないという、lock順序を
/// project lock→session leaseに固定する制約を型で保証する。
const SESSION_LEASE_ACQUIRER: &str = "src/support/select/locked.rs";

/// sandbox名の完全一致入力を`ProtectionConfirmation`へ変えてよい唯一の場所。
///
/// `confirmation::confirm`はsnapshotをconsumeする低水準APIである。呼び出し箇所が
/// 増えると、rebuild / destroyがそれぞれ独自の確認判定を持ち、共通gateを経ない
/// `ProtectionConfirmation`が生まれ得る。
const PROTECTION_CONFIRMER: &str = "src/support/protection/confirmation/confirm_interactively.rs";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// `src`配下のRust source。
fn sources() -> Checked<Vec<(String, String)>> {
    let mut found = Vec::new();
    collect(&root().join("src"), &mut found)?;
    found.sort();
    let mut sources = Vec::with_capacity(found.len());
    for path in found {
        let text = std::fs::read_to_string(&path).required_because("the source is readable")?;
        let relative = path
            .strip_prefix(root())
            .required_because("inside the repository")?
            .to_string_lossy()
            .into_owned();
        sources.push((relative, text));
    }
    Ok(sources)
}

fn collect(directory: &Path, found: &mut Vec<PathBuf>) -> Checked {
    for entry in std::fs::read_dir(directory).required_because("the directory is readable")? {
        let path = entry.required_because("directory entry")?.path();
        if path.is_dir() {
            collect(&path, found)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
    Ok(())
}

/// `src/design`の外か。
fn outside_design(path: &str) -> bool {
    !path.starts_with(DESIGN)
}

#[test]
fn color_mode_vocabulary_stays_in_the_design_policy() -> Checked {
    let mut offenders = Vec::new();
    for (path, text) in sources()? {
        if !path.starts_with(COMMAND_LINE) || path.ends_with("_test.rs") {
            continue;
        }
        for (index, line) in text.lines().enumerate() {
            if ["\"auto\"", "\"always\"", "\"never\""]
                .iter()
                .any(|value| line.contains(value))
            {
                offenders.push(format!("{path}:{}: {}", index + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "accepted color values belong to {COLOR_MODE}:\n{}",
        offenders.join("\n")
    );
    Ok(())
}

#[test]
fn clap_is_used_only_by_the_command_line_boundary_adapter() -> Checked {
    let mut offenders = Vec::new();
    for (path, text) in sources()? {
        if path.ends_with("_test.rs")
            || path.starts_with(CLAP_ADAPTER)
            || path == "src/boundary/command_line/mod.rs"
        {
            continue;
        }
        for (index, line) in text.lines().enumerate() {
            if line.contains("clap::") || line.contains("use clap") {
                offenders.push(format!("{path}:{}: {}", index + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "clap belongs only in {CLAP_ADAPTER}:\n{}",
        offenders.join("\n")
    );
    Ok(())
}

#[test]
fn the_design_system_does_not_depend_on_the_cli_parser() -> Checked {
    let mut offenders = Vec::new();
    for (path, text) in sources()? {
        if !path.starts_with(DESIGN) || path.ends_with("_test.rs") {
            continue;
        }
        for (index, line) in text.lines().enumerate() {
            if line.contains("clap::") || line.contains("use clap") {
                offenders.push(format!("{path}:{}: {}", index + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "the design system must not depend on clap:\n{}",
        offenders.join("\n")
    );
    Ok(())
}

#[test]
fn user_facing_output_is_not_written_with_a_print_macro() -> Checked {
    // 直接書くと、block間隔とstreamの責務がcommandごとに散る。
    let mut offenders = Vec::new();
    for (path, text) in sources()? {
        if !outside_design(&path) {
            continue;
        }
        for (index, line) in text.lines().enumerate() {
            for macro_name in ["println!", "print!", "eprintln!", "eprint!"] {
                if line.contains(macro_name) {
                    offenders.push(format!("{path}:{}: {}", index + 1, line.trim()));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "user-facing output belongs to the design system:\n{}",
        offenders.join("\n")
    );
    Ok(())
}

#[test]
fn ansi_escape_sequences_are_generated_in_one_file() -> Checked {
    let mut offenders = Vec::new();
    for (path, text) in sources()? {
        if path == RENDERER {
            continue;
        }
        for (index, line) in text.lines().enumerate() {
            // testは期待値としてescapeを書く。生成しているのはrendererだけである。
            if path.ends_with("_test.rs") {
                continue;
            }
            if line.contains("\\u{1b}") || line.contains("\\x1b") || line.contains("\\033") {
                offenders.push(format!("{path}:{}: {}", index + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "only {RENDERER} generates ANSI:\n{}",
        offenders.join("\n")
    );
    Ok(())
}

#[test]
fn concrete_terminal_adapters_are_confined_to_the_terminal_boundary() -> Checked {
    // promptのportと描画styleはdesignに残し、実端末の型だけをboundaryへ閉じ込める。
    let mut offenders = Vec::new();
    for (path, text) in sources()? {
        if path.starts_with(TERMINAL_ADAPTER) || path.ends_with("_test.rs") {
            continue;
        }
        for (index, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("use console")
                || trimmed.starts_with("use dialoguer")
                || line.contains("console::Term")
                || line.contains("console::Key")
            {
                offenders.push(format!("{path}:{}: {}", index + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "concrete terminal adapters belong only in {TERMINAL_ADAPTER}:\n{}",
        offenders.join("\n")
    );
    Ok(())
}

#[test]
fn terminal_and_environment_observation_stays_in_the_terminal_boundary() -> Checked {
    let observed = [
        "std::io::stdin()",
        "std::io::stdout()",
        "std::io::stderr()",
        "IsTerminal",
        "\"NO_COLOR\"",
        "\"CLICOLOR_FORCE\"",
        "\"TERM\"",
    ];
    let mut offenders = Vec::new();
    for (path, text) in sources()? {
        if path.starts_with(TERMINAL_ADAPTER) || path.ends_with("_test.rs") {
            continue;
        }
        for (index, line) in text.lines().enumerate() {
            if observed.iter().any(|value| line.contains(value)) {
                offenders.push(format!("{path}:{}: {}", index + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "terminal and environment observation belongs in {TERMINAL_ADAPTER}:\n{}",
        offenders.join("\n")
    );
    Ok(())
}

#[test]
fn selection_prompts_are_built_in_one_place() -> Checked {
    let mut offenders = Vec::new();
    for (path, text) in sources()? {
        if path == "src/design/prompt.rs" {
            continue;
        }
        for (index, line) in text.lines().enumerate() {
            for construction in ["Select::new()", "MultiSelect::new()", "Confirm::new()"] {
                if line.contains(construction) {
                    offenders.push(format!("{path}:{}: {}", index + 1, line.trim()));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a command must not grow its own prompt:\n{}",
        offenders.join("\n")
    );
    Ok(())
}

#[test]
fn no_command_writes_its_own_block_spacing() -> Checked {
    // 先頭の改行で余白を作ると、rendererの間隔管理を迂回する。
    let mut offenders = Vec::new();
    for (path, text) in sources()? {
        if !outside_design(&path) || path.ends_with("_test.rs") {
            continue;
        }
        for (index, line) in text.lines().enumerate() {
            for macro_name in ["write!", "writeln!", "format!"] {
                let Some(position) = line.find(macro_name) else {
                    continue;
                };
                if line[position..].contains("\"\\n") {
                    offenders.push(format!("{path}:{}: {}", index + 1, line.trim()));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "block spacing belongs to the renderer:\n{}",
        offenders.join("\n")
    );
    Ok(())
}

#[test]
fn a_child_process_is_pointed_at_the_terminal_in_one_file_only() -> Checked {
    // 子processへ端末をそのまま渡せる場所が増えると、sbxmの行と外部toolの行のあいだに
    // 空行を置かない経路ができる。
    let mut offenders = Vec::new();
    for (path, text) in sources()? {
        if path == CONFIGURE || path.ends_with("_test.rs") {
            continue;
        }
        for (index, line) in text.lines().enumerate() {
            if line.contains("Stdio::inherit") {
                offenders.push(format!("{path}:{}: {}", index + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "external output reaches the terminal through ExternalOutput, not through {CONFIGURE}:\n{}",
        offenders.join("\n")
    );
    Ok(())
}

#[test]
fn what_sbx_says_reaches_the_terminal_through_one_place() -> Checked {
    // `sbx`は自分への入り方を案内する。sbxmの案内と食い違う行を落とす判断を、subcommand
    // ごとに書き分けると、書き忘れた1つが利用者を別の入り方へ連れて行く。
    let mut offenders = Vec::new();
    for (path, text) in sources()? {
        if path == SBX_RELAY || path.ends_with("_test.rs") {
            continue;
        }
        for (index, line) in text.lines().enumerate() {
            if line.contains("TerminalCommand::relayed(\"sbx\"") {
                offenders.push(format!("{path}:{}: {}", index + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a relayed sbx command is built in {SBX_RELAY}:\n{}",
        offenders.join("\n")
    );
    Ok(())
}

#[test]
fn sbx_is_run_with_a_confirmation_prompt_in_one_place_only() -> Checked {
    // 期待するpromptと答えの組はここでしか決めない。呼び出し箇所が増えると、
    // どのpromptに何を答えるかがcommandごとに分かれ、固定protocolでなくなる。
    let mut offenders = Vec::new();
    for (path, text) in sources()? {
        if path == SBX_PTY_CONFIRM || path.ends_with("_test.rs") {
            continue;
        }
        for (index, line) in text.lines().enumerate() {
            if line.contains("PtyConfirmedCommand::new(") {
                offenders.push(format!("{path}:{}: {}", index + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a PTY-confirmed sbx command is built only in {SBX_PTY_CONFIRM}:\n{}",
        offenders.join("\n")
    );
    Ok(())
}

#[test]
fn docker_commands_are_constructed_in_one_module() -> Checked {
    // CommandSpecのconstructorは任意のprogram名を受け取るため、Rustのmodule privacy
    // だけではdockerの境界を強制できない。実行を表す明確なconstructor呼び出しをここで
    // 検査し、新しいdocker経路の追加時に集約を忘れたままmergeされないようにする。
    let constructors = [
        "CommandSpec::capture(\"docker\"",
        "CommandSpec::probe(\"docker\"",
        "TerminalCommand::relayed(\"docker\"",
        "TerminalCommand::handed_over(\"docker\"",
    ];
    let mut offenders = Vec::new();
    for (path, text) in sources()? {
        if path.starts_with(DOCKER_SUPPORT) {
            continue;
        }
        for (index, line) in text.lines().enumerate() {
            if constructors
                .iter()
                .any(|constructor| line.contains(constructor))
            {
                offenders.push(format!("{path}:{}: {}", index + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "docker commands must be built through {DOCKER_SUPPORT}:\n{}",
        offenders.join("\n")
    );
    Ok(())
}

#[test]
fn the_session_lease_is_acquired_only_while_holding_the_project_lock() -> Checked {
    // `session_lease_file`は`Locked`のmethod以外から呼べない。session lease自体を
    // 直接構築する経路が増えると、project lockを取らずにsession leaseだけを取得する
    // 逆順が生まれてしまう。
    let mut offenders = Vec::new();
    for (path, text) in sources()? {
        if path == SESSION_LEASE_ACQUIRER || path.ends_with("_test.rs") {
            continue;
        }
        for (index, line) in text.lines().enumerate() {
            // `.`付きの呼び出し形にすることで、`ProjectPaths`自身の定義行を誤検出しない。
            if line.contains(".session_lease_file(") {
                offenders.push(format!("{path}:{}: {}", index + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "the session lease path is touched only from {SESSION_LEASE_ACQUIRER}, always behind the project lock:\n{}",
        offenders.join("\n")
    );
    Ok(())
}

#[test]
fn a_protection_confirmation_is_built_in_one_place_only() -> Checked {
    // `confirmation::confirm`はsandbox名の完全一致だけを合図に`ProtectionConfirmation`
    // を作る低水準APIである。呼び出し箇所が増えると、rebuild / destroyがそれぞれ別の
    // 確認判定を持ち、共通gateを経ない`ProtectionConfirmation`を作れてしまう。
    let mut offenders = Vec::new();
    for (path, text) in sources()? {
        if path == PROTECTION_CONFIRMER || path.ends_with("_test.rs") {
            continue;
        }
        for (index, line) in text.lines().enumerate() {
            if line.contains("confirmation::confirm(") {
                offenders.push(format!("{path}:{}: {}", index + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a ProtectionConfirmation is built only from {PROTECTION_CONFIRMER}:\n{}",
        offenders.join("\n")
    );
    Ok(())
}

#[test]
fn no_command_grows_its_own_receiver_for_external_output() -> Checked {
    // 境界の空行を置くのは`src/design`、外部toolのbyteを運ぶのは`src/boundary/host`である。
    // commandや工程が自前の受け口を持てば、そのcommandだけ見え方が分かれる。
    let mut offenders = Vec::new();
    for (path, text) in sources()? {
        let allowed = !outside_design(&path)
            || path.starts_with(RELAY)
            || path.starts_with("src/testing")
            || path.ends_with("_test.rs");
        if allowed {
            continue;
        }
        for (index, line) in text.lines().enumerate() {
            if line.contains("impl ExternalOutput for") {
                offenders.push(format!("{path}:{}: {}", index + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "external output is received by {DESIGN} and carried by {RELAY}:\n{}",
        offenders.join("\n")
    );
    Ok(())
}

#[test]
fn no_resource_pads_itself_with_blank_lines() -> Checked {
    // block間隔はrendererが決める。resourceが前後の余白を持つと二重になる。
    let mut offenders = Vec::new();
    for (name, text) in resources()? {
        for (index, line) in text.lines().enumerate() {
            let Some((_, value)) = line.split_once(" = ") else {
                continue;
            };
            if value != value.trim() {
                offenders.push(format!("{name}:{}: {}", index + 1, line.trim()));
            }
        }
    }
    assert!(offenders.is_empty(), "{}", offenders.join("\n"));
    Ok(())
}

/// FTL resourceの原文。
fn resources() -> Checked<Vec<(String, String)>> {
    let directory = root().join("locales");
    let mut found: Vec<(String, String)> = Vec::new();
    for entry in
        std::fs::read_dir(&directory).required_because("the locales directory is readable")?
    {
        let path = entry.required_because("directory entry")?.path();
        if path.extension().is_some_and(|extension| extension == "ftl") {
            let text =
                std::fs::read_to_string(&path).required_because("the resource is readable")?;
            let name = path
                .file_name()
                .required_because("a file name")?
                .to_string_lossy()
                .into_owned();
            found.push((name, text));
        }
    }
    found.sort();
    assert!(!found.is_empty(), "no FTL resource was found");
    Ok(found)
}

/// 実行を求めるcommandだと確実に分かる綴り。
///
/// `sbxm builds only from`のような文中の語を拾わないよう、program名のあとに続く
/// subcommandまで見る。
const INVOCATIONS: [(&str, &[&str]); 5] = [
    (
        "sbxm ",
        &[
            "init", "add", "apply", "prepare", "rebuild", "open", "stop", "ls", "status",
            "destroy", "--",
        ],
    ),
    (
        "sbx ",
        &[
            "secret", "rm", "ls", "create", "exec", "login", "template", "version",
        ],
    ),
    (
        "git ",
        &[
            "clone",
            "worktree",
            "config",
            "fetch",
            "status",
            "commit",
            "push",
            "init",
            "remote",
            "rev-parse",
        ],
    ),
    ("chmod ", &[]),
    ("mise ", &["trust", "install", "use"]),
];

#[test]
fn no_resource_embeds_a_command_the_user_is_meant_to_run() -> Checked {
    let mut offenders = Vec::new();
    for (name, text) in resources()? {
        for (index, line) in text.lines().enumerate() {
            for (program, subcommands) in INVOCATIONS {
                let Some(position) = line.find(program) else {
                    continue;
                };
                let rest = &line[position + program.len()..];
                let named = subcommands.is_empty()
                    || subcommands
                        .iter()
                        .any(|subcommand| rest.starts_with(subcommand));
                if named {
                    offenders.push(format!("{name}:{}: {}", index + 1, line.trim()));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "the resource explains and the model supplies the command:\n{}",
        offenders.join("\n")
    );
    Ok(())
}

#[test]
fn no_resource_carries_a_command_placeholder() -> Checked {
    // commandはtypedな一行として渡す。placeholderが残っていれば経路が古い。
    let mut offenders = Vec::new();
    for (name, text) in resources()? {
        for (index, line) in text.lines().enumerate() {
            if line.contains("$command") {
                offenders.push(format!("{name}:{}: {}", index + 1, line.trim()));
            }
        }
    }
    assert!(offenders.is_empty(), "{}", offenders.join("\n"));
    Ok(())
}

#[test]
fn no_resource_carries_a_severity_marker_or_an_escape() -> Checked {
    // prefixとstyleはrendererが付ける。翻訳者が記号の一貫性を預からない。
    let mut offenders = Vec::new();
    for (name, text) in resources()? {
        for (index, line) in text.lines().enumerate() {
            let Some((_, value)) = line.split_once(" = ") else {
                continue;
            };
            for marker in ["\u{1b}", "\u{2192} ", "\u{2713} ", "\u{d7} ", "\u{203a} "] {
                if value.contains(marker) {
                    offenders.push(format!("{name}:{}: {}", index + 1, line.trim()));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "markers belong to the renderer:\n{}",
        offenders.join("\n")
    );
    Ok(())
}

#[test]
fn no_resource_carries_an_emoji() -> Checked {
    let mut offenders = Vec::new();
    for (name, text) in resources()? {
        for (index, line) in text.lines().enumerate() {
            for character in line.chars() {
                let point = u32::from(character);
                let pictograph = (0x1F000..=0x1FAFF).contains(&point)
                    || (0x2600..=0x27BF).contains(&point)
                    || point == 0xFE0F
                    || point == 0x200D;
                if pictograph {
                    offenders.push(format!("{name}:{}: {}", index + 1, line.trim()));
                    break;
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a pictograph can be drawn in more than one color:\n{}",
        offenders.join("\n")
    );
    Ok(())
}
