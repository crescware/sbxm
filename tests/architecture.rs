//! デザインシステムの境界。
//!
//! 規約は文書だけでは守れない。ここが検出するのは「どこで何を組み立ててよいか」であり、
//! 見た目そのものは`src/design`のinvariant testが持つ。
//!
//! 検出は禁止APIと明確なprefixに限る。文字列検索だけでは`docker`のような語と実行
//! commandを完全には区別できないため、曖昧な判定を足して誤検出を増やさない。

mod disable_directive;
mod flaky_elements;
mod os_layer;
mod outcome;

use disable_directive::DISABLE_NEXT_LINE;
use flaky_elements::Element;
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
            "init", "add", "apply", "guide", "prepare", "rebuild", "open", "stop", "ls", "status",
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

/// 実行を求めるcommandをresourceへ書かせない検査の名前。検査の関数名と一致させる。
const EMBEDDED_COMMAND: &str = "no_resource_embeds_a_command_the_user_is_meant_to_run";

/// 宣言で次の1行だけ無効にできる検査。
///
/// 文字列検索では、commandの実行を求める文と、commandに言及する文を区別できない。
/// 区別できない検査だけが宣言を受け付ける。綴りだけで判定が決まる検査は受け付けない。
const DISABLEABLE: [&str; 1] = [EMBEDDED_COMMAND];

#[test]
fn no_resource_embeds_a_command_the_user_is_meant_to_run() -> Checked {
    // 宣言はこの関数名で検査を名指しする。関数名だけを変えると、古い名前の宣言が効き続ける。
    fn here() {}
    assert!(
        std::any::type_name_of_val(&here).ends_with(&format!("::{EMBEDDED_COMMAND}::here")),
        "EMBEDDED_COMMAND must name this check: {}",
        std::any::type_name_of_val(&here)
    );

    let mut offenders = Vec::new();
    for (name, text) in resources()? {
        offenders.extend(embedded_commands(&name, &text));
    }
    assert!(
        offenders.is_empty(),
        "the resource explains and the model supplies the command. A line that only mentions \
         a command may disable this check with a directive and a reason; see locales/README.md:\n{}",
        offenders.join("\n")
    );
    Ok(())
}

#[test]
fn every_disable_directive_names_a_disableable_rule_and_a_reason() -> Checked {
    let mut problems = Vec::new();
    for (name, text) in resources()? {
        problems.extend(directive_problems(&name, &text));
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    Ok(())
}

#[test]
fn a_directive_disables_the_command_check_for_the_next_line_only() {
    let text = format!(
        "{DISABLE_NEXT_LINE} {EMBEDDED_COMMAND} -- git cloneの規則を説明する\n\
         a = named as git clone names it\n\
         b = run git clone first\n"
    );
    assert_eq!(
        embedded_commands("t.ftl", &text),
        vec!["t.ftl:3: b = run git clone first".to_string()]
    );
}

#[test]
fn a_directive_that_disables_nothing_is_reported() {
    // 文言を直してcommandが無くなれば、宣言も消す。
    for text in [
        format!("{DISABLE_NEXT_LINE} {EMBEDDED_COMMAND} -- 説明する\na = no command here\n"),
        format!("a = no command here\n{DISABLE_NEXT_LINE} {EMBEDDED_COMMAND} -- 説明する\n"),
    ] {
        let offenders = embedded_commands("t.ftl", &text);
        assert_eq!(offenders.len(), 1, "{text}");
        assert!(offenders[0].contains("disables nothing"), "{offenders:?}");
    }
}

#[test]
fn a_directive_names_one_disableable_rule_and_gives_a_reason() {
    let next = "\na = run git clone first\n";
    assert!(
        directive_problems(
            "t.ftl",
            &format!("{DISABLE_NEXT_LINE} {EMBEDDED_COMMAND} -- 説明する{next}")
        )
        .is_empty()
    );
    for directive in [
        DISABLE_NEXT_LINE.to_string(),
        format!("{DISABLE_NEXT_LINE} {EMBEDDED_COMMAND}"),
        format!("{DISABLE_NEXT_LINE} {EMBEDDED_COMMAND} -- "),
        format!("{DISABLE_NEXT_LINE} two rules -- 説明する"),
        format!("{DISABLE_NEXT_LINE} no_resource_carries_an_emoji -- 説明する"),
    ] {
        assert_eq!(
            directive_problems("t.ftl", &format!("{directive}{next}")).len(),
            1,
            "{directive}"
        );
    }
}

/// `text`のうち、実行を求めるcommandを書いた行。
///
/// 宣言で無効にした行は数えない。無効にした行がcommandを書いていなければ、宣言が何も
/// 無効にしていないことを示す。宣言の行はcommentであり表示されないため、読まない。
fn embedded_commands(name: &str, text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut offenders = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let at = format!("{name}:{}", index + 1);
        if directive(line).is_some() {
            let disabled = lines
                .get(index + 1)
                .is_some_and(|next| directive(next).is_none() && embeds_command(next));
            if disables(line, EMBEDDED_COMMAND) && !disabled {
                offenders.push(format!("{at}: the directive disables nothing: {line}"));
            }
            continue;
        }
        let disabled = index
            .checked_sub(1)
            .is_some_and(|previous| disables(lines[previous], EMBEDDED_COMMAND));
        if !disabled && embeds_command(line) {
            offenders.push(format!("{at}: {}", line.trim()));
        }
    }
    offenders
}

/// `line`が、実行を求めるcommandだと確実に分かる綴りを持つか。
fn embeds_command(line: &str) -> bool {
    INVOCATIONS.iter().any(|(program, subcommands)| {
        line.find(program).is_some_and(|position| {
            let rest = &line[position + program.len()..];
            subcommands.is_empty()
                || subcommands
                    .iter()
                    .any(|subcommand| rest.starts_with(subcommand))
        })
    })
}

/// `text`の宣言のうち、形が崩れているもの、または無効にできない検査を名指しするもの。
fn directive_problems(name: &str, text: &str) -> Vec<String> {
    let mut problems = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let at = format!("{name}:{}", index + 1);
        match directive(line) {
            Some(Err(reason)) => problems.push(format!("{at}: {reason}: {line}")),
            Some(Ok(rule)) if !DISABLEABLE.contains(&rule) => {
                problems.push(format!("{at}: {rule} cannot be disabled: {line}"));
            }
            None | Some(Ok(_)) => {}
        }
    }
    problems
}

/// `line`が`rule`を無効にする正しい宣言か。
fn disables(line: &str, rule: &str) -> bool {
    matches!(directive(line), Some(Ok(named)) if named == rule)
}

/// `line`が宣言なら、無効にする検査の名前。宣言でなければ`None`、形が崩れていれば理由。
///
/// 宣言は`# architecture-test-disable-next-line <検査名> -- <理由>`の形だけとする。
fn directive(line: &str) -> Option<Result<&str, &'static str>> {
    let rest = line.strip_prefix(DISABLE_NEXT_LINE)?;
    let Some(rest) = rest.strip_prefix(' ') else {
        return Some(Err("the directive names no rule"));
    };
    let Some((rule, reason)) = rest.split_once(" --") else {
        return Some(Err("the directive gives no reason after --"));
    };
    if rule.is_empty() || rule.contains(char::is_whitespace) {
        return Some(Err("the directive does not name exactly one rule"));
    }
    if reason.trim().is_empty() {
        return Some(Err("the directive gives no reason after --"));
    }
    Some(Ok(rule))
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

/// OSの呼び出しそのものを置く層。分岐を持たず、coverageの母集団から外す。
const OS_LAYER: &str = "src/boundary/os/";

#[test]
fn the_os_layer_has_no_branches() -> Checked {
    let mut violations = Vec::new();
    for (path, text) in sources()? {
        if !path.starts_with(OS_LAYER) || is_test_code(&path) {
            continue;
        }
        let entry = path.ends_with("/mod.rs");
        for violation in os_layer::violations(&text, entry).required_because("the source parses")? {
            violations.push(format!("{path}: {violation}"));
        }
    }
    assert!(
        violations.is_empty(),
        "each function in {OS_LAYER} is one call, so that no decision escapes coverage:\n{}",
        violations.join("\n")
    );
    Ok(())
}

/// testを置く場所か。`tests/module_boundaries.rs`がcoverageから外す4か所と同じ綴り。
fn is_test_code(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    path.starts_with("tests/")
        || path.starts_with("src/testing/")
        || path.contains("/fake/")
        || name.contains("_test")
}

/// OS層の実物を、判断のcodeへ渡してよい本番のfile。
///
/// 判断のcodeは基本操作を差し込みで受け取り、実物を選ぶのはここに挙げた配線だけとする。
/// 一覧に無い本番codeがOS層を名指しすれば、差し込まずに実OSを使う判断が紛れる。
const OS_LAYER_WIRING: [&str; 6] = [
    "src/app/execute.rs",
    "src/paths/lock/acquire_exclusive_lock.rs",
    "src/paths/lock/acquire_shared_lock.rs",
    "src/paths/lock/exclusive_lock.rs",
    "src/paths/lock/shared_lock.rs",
    "src/support/daemon/list_with_timeout.rs",
];

/// flakyになりうる要素を検出する定義そのもの。検出する綴りを例として持つため読まない。
const FLAKY_ELEMENT_DEFINITION: &str = "tests/flaky_elements/";

/// flakyになりうる要素を書いてよいfileと、そこに書いてよい要素。
///
/// 一覧に無いfileに要素が現れても、一覧のfileに許した種類以外の要素が現れても落ちる。
/// 一覧のfileから要素が消えたら、一覧から外すまで落ちる。一覧は減る方向にしか動かない。
const FLAKY_ELEMENT_PLACES: [(&str, &[Element]); 21] = [
    // OS層。分岐を持たず、coverageの母集団から外す。
    ("src/boundary/os/system_clock.rs", &[Element::RealTime]),
    ("src/boundary/os/system_file_lock.rs", &[Element::FileLock]),
    // 外部processを動かす実行。判断とOSの呼び出しが同じ関数にある。
    ("src/boundary/host/poll_pipes.rs", &[Element::ChildProcess]),
    (
        "src/boundary/host/pump_until_exit.rs",
        &[Element::RealTime, Element::ChildProcess],
    ),
    ("src/boundary/host/run_inner.rs", &[Element::ChildProcess]),
    (
        "src/boundary/host/run_pty_confirmed.rs",
        &[Element::RealTime, Element::ChildProcess, Element::Signal],
    ),
    ("src/boundary/host/run_relay.rs", &[Element::ChildProcess]),
    ("src/boundary/host/signal_guard.rs", &[Element::Signal]),
    ("src/boundary/host/spawn.rs", &[Element::ChildProcess]),
    (
        "src/boundary/host/terminate_child.rs",
        &[Element::ChildProcess, Element::Signal],
    ),
    ("src/boundary/host/unwaitable.rs", &[Element::ChildProcess]),
    (
        "src/boundary/host/wait_with_limit.rs",
        &[Element::RealTime, Element::ChildProcess],
    ),
    // test。
    (
        "src/boundary/host/command_test.rs",
        &[Element::RealTime, Element::ChildProcess],
    ),
    (
        "src/boundary/host/fake/pump_until_exit_test.rs",
        &[Element::RealTime, Element::ChildProcess],
    ),
    (
        "src/boundary/host/run_pty_confirmed_test.rs",
        &[Element::RealTime, Element::ChildProcess, Element::Signal],
    ),
    (
        "tests/command_lifecycle.rs",
        &[Element::RealTime, Element::ChildProcess, Element::Signal],
    ),
    ("tests/host.rs", &[Element::ChildProcess, Element::Signal]),
    ("tests/place_from_stdin.rs", &[Element::Signal]),
    (
        "tests/prompt_pty.rs",
        &[Element::ChildProcess, Element::Signal],
    ),
    (
        "tests/prompt_terminal.rs",
        &[Element::ChildProcess, Element::Signal],
    ),
    ("tests/wait_until/mod.rs", &[Element::RealTime]),
];

/// `src`と`tests`のRust source。
fn sources_and_tests() -> Checked<Vec<(String, String)>> {
    let mut found = Vec::new();
    collect(&root().join("src"), &mut found)?;
    collect(&root().join("tests"), &mut found)?;
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

#[test]
fn flaky_elements_stay_where_they_are_allowed() -> Checked {
    let sources: Vec<(String, String)> = sources_and_tests()?
        .into_iter()
        .filter(|(path, _)| !path.starts_with(FLAKY_ELEMENT_DEFINITION))
        .collect();
    let violations = flaky_element_violations(&sources, &FLAKY_ELEMENT_PLACES, &OS_LAYER_WIRING)?;
    assert!(
        violations.is_empty(),
        "flaky elements are written only where FLAKY_ELEMENT_PLACES allows them:\n{}",
        violations.join("\n")
    );
    Ok(())
}

/// 要素が一覧の許す場所と種類を外れていないか。一覧は減る方向にしか動かせない。
///
/// OS層の実物を名指しすることも要素として数える。OS層の中と、`wiring`に挙げた本番の配線
/// だけが名指ししてよい。
fn flaky_element_violations(
    sources: &[(String, String)],
    places: &[(&str, &[Element])],
    wiring: &[&str],
) -> Checked<Vec<String>> {
    let mut violations = Vec::new();
    for (path, text) in sources {
        let mut found = flaky_elements::elements(text).required_because("the source parses")?;
        let wires = found.iter().any(|item| item.element == Element::OsLayer);
        let listed = wiring.contains(&path.as_str());
        if listed && !wires {
            violations.push(format!(
                "{path}: no longer names the OS layer; remove it from OS_LAYER_WIRING"
            ));
        }
        if listed && is_test_code(path) {
            violations.push(format!(
                "{path}: a test is not wiring; remove it from OS_LAYER_WIRING"
            ));
        }
        if (listed && !is_test_code(path)) || path.starts_with(OS_LAYER) {
            found.retain(|item| item.element != Element::OsLayer);
        }
        let allowed: &[Element] = places
            .iter()
            .find(|(place, _)| place == path)
            .map_or(&[], |(_, elements)| elements);
        for (element, at) in flaky_elements::by_element(&found) {
            if !allowed.contains(&element) {
                violations.push(format!(
                    "{path}: {element:?} is not allowed here:\n    {}",
                    at.join("\n    ")
                ));
            }
        }
        let present = flaky_elements::kinds(&found);
        for element in allowed {
            if !present.contains(element) {
                violations.push(format!(
                    "{path}: {element:?} is gone; remove it from FLAKY_ELEMENT_PLACES"
                ));
            }
        }
    }
    for (place, _) in places {
        if !sources.iter().any(|(path, _)| path == place) {
            violations.push(format!(
                "{place}: listed in FLAKY_ELEMENT_PLACES, but there is no such file"
            ));
        }
    }
    for place in wiring {
        if !sources.iter().any(|(path, _)| path == place) {
            violations.push(format!(
                "{place}: listed in OS_LAYER_WIRING, but there is no such file"
            ));
        }
    }
    Ok(violations)
}

#[test]
fn a_flaky_element_outside_the_list_is_reported() -> Checked {
    let sources = [(
        "src/wait.rs".to_string(),
        "fn wait() { std::thread::sleep(D); }".to_string(),
    )];
    let violations = flaky_element_violations(&sources, &[], &[])?;
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(violations[0].contains("RealTime is not allowed here"));
    Ok(())
}

#[test]
fn a_listed_file_may_not_take_another_kind_of_element() -> Checked {
    let sources = [(
        "src/wait.rs".to_string(),
        "fn wait() { std::thread::sleep(D); std::thread::spawn(f); }".to_string(),
    )];
    let violations =
        flaky_element_violations(&sources, &[("src/wait.rs", &[Element::RealTime])], &[])?;
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(violations[0].contains("Thread is not allowed here"));
    Ok(())
}

#[test]
fn only_the_listed_wiring_names_the_os_layer_outside_it() -> Checked {
    // 配線は実物を選んで渡すだけである。ほかの本番codeやtestが名指しすれば実OSを動かす。
    let uses = "use crate::boundary::os::SystemClock;\nfn f() { g(&SystemClock); }".to_string();
    let sources = [
        ("src/paths/lock/acquire.rs".to_string(), uses.clone()),
        ("src/paths/lock/decide.rs".to_string(), uses.clone()),
        ("src/paths/lock/lock_test.rs".to_string(), uses.clone()),
        ("src/boundary/os/system_clock_test.rs".to_string(), uses),
    ];
    let violations = flaky_element_violations(&sources, &[], &["src/paths/lock/acquire.rs"])?;
    assert_eq!(violations.len(), 2, "{violations:?}");
    assert!(violations[0].starts_with("src/paths/lock/decide.rs: OsLayer"));
    assert!(violations[1].starts_with("src/paths/lock/lock_test.rs: OsLayer"));

    // 名指ししなくなった配線は一覧から外す。
    let sources = [(
        "src/paths/lock/acquire.rs".to_string(),
        "fn f() {}".to_string(),
    )];
    let violations = flaky_element_violations(&sources, &[], &["src/paths/lock/acquire.rs"])?;
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(violations[0].contains("remove it from OS_LAYER_WIRING"));
    Ok(())
}

#[test]
fn an_element_that_is_gone_must_leave_the_list() -> Checked {
    // 一覧は減る方向にしか動かない。消えた要素を残せば、次に足す要素がそこへ紛れる。
    let sources = [("src/wait.rs".to_string(), "fn wait() {}".to_string())];
    let violations =
        flaky_element_violations(&sources, &[("src/wait.rs", &[Element::RealTime])], &[])?;
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(violations[0].contains("RealTime is gone"));

    let violations = flaky_element_violations(&[], &[("src/wait.rs", &[Element::RealTime])], &[])?;
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(violations[0].contains("no such file"));
    Ok(())
}
