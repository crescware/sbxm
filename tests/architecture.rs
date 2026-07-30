//! デザインシステムの境界。
//!
//! 規約は文書だけでは守れない。ここが検出するのは「どこで何を組み立ててよいか」であり、
//! 見た目そのものは`src/ui`のinvariant testが持つ。
//!
//! 検出は禁止APIと明確なprefixに限る。文字列検索だけでは`docker`のような語と実行
//! commandを完全には区別できないため、曖昧な判定を足して誤検出を増やさない。

use std::path::{Path, PathBuf};

/// 描画を組み立ててよい唯一の場所。
const UI: &str = "src/ui";

/// ANSI escape sequenceを生成してよい唯一のfile。
const RENDERER: &str = "src/ui/renderer.rs";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// `src`配下のRust source。
fn sources() -> Vec<(String, String)> {
    let mut found = Vec::new();
    collect(&root().join("src"), &mut found);
    found.sort();
    found
        .into_iter()
        .map(|path| {
            let text = std::fs::read_to_string(&path).expect("the source is readable");
            let relative = path
                .strip_prefix(root())
                .expect("inside the repository")
                .to_string_lossy()
                .into_owned();
            (relative, text)
        })
        .collect()
}

fn collect(directory: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).expect("the directory is readable") {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            collect(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
}

/// `src/ui`の外か。
fn outside_ui(path: &str) -> bool {
    !path.starts_with(UI)
}

#[test]
fn user_facing_output_is_not_written_with_a_print_macro() {
    // 直接書くと、block間隔とstreamの責務がcommandごとに散る。
    let mut offenders = Vec::new();
    for (path, text) in sources() {
        if !outside_ui(&path) {
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
}

#[test]
fn ansi_escape_sequences_are_generated_in_one_file() {
    let mut offenders = Vec::new();
    for (path, text) in sources() {
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
}

#[test]
fn the_terminal_crate_is_imported_only_by_the_design_system() {
    // 具体的なterminal crateの型を`ui`の外へ公開しない。
    let mut offenders = Vec::new();
    for (path, text) in sources() {
        if !outside_ui(&path) {
            continue;
        }
        for (index, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("use console::")
                || trimmed.starts_with("use dialoguer::")
                || line.contains("console::Term")
                || line.contains("console::Style")
            {
                offenders.push(format!("{path}:{}: {}", index + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "the terminal crate stays inside {UI}:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn selection_prompts_are_built_in_one_place() {
    let mut offenders = Vec::new();
    for (path, text) in sources() {
        if path == "src/ui/prompt.rs" {
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
}

#[test]
fn no_command_writes_its_own_block_spacing() {
    // 先頭の改行で余白を作ると、rendererの間隔管理を迂回する。
    let mut offenders = Vec::new();
    for (path, text) in sources() {
        if !outside_ui(&path) || path.ends_with("_test.rs") {
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
}

#[test]
fn no_resource_pads_itself_with_blank_lines() {
    // block間隔はrendererが決める。resourceが前後の余白を持つと二重になる。
    let mut offenders = Vec::new();
    for (name, text) in resources() {
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
}

/// FTL resourceの原文。
fn resources() -> Vec<(String, String)> {
    let directory = root().join("locales");
    let mut found: Vec<(String, String)> = std::fs::read_dir(&directory)
        .expect("the locales directory is readable")
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "ftl"))
        .map(|path| {
            let text = std::fs::read_to_string(&path).expect("the resource is readable");
            (
                path.file_name()
                    .expect("a file name")
                    .to_string_lossy()
                    .into_owned(),
                text,
            )
        })
        .collect();
    found.sort();
    assert!(!found.is_empty(), "no FTL resource was found");
    found
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
fn no_resource_embeds_a_command_the_user_is_meant_to_run() {
    let mut offenders = Vec::new();
    for (name, text) in resources() {
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
}

#[test]
fn no_resource_carries_a_command_placeholder() {
    // commandはtypedな一行として渡す。placeholderが残っていれば経路が古い。
    let mut offenders = Vec::new();
    for (name, text) in resources() {
        for (index, line) in text.lines().enumerate() {
            if line.contains("$command") {
                offenders.push(format!("{name}:{}: {}", index + 1, line.trim()));
            }
        }
    }
    assert!(offenders.is_empty(), "{}", offenders.join("\n"));
}

#[test]
fn no_resource_carries_a_severity_marker_or_an_escape() {
    // prefixとstyleはrendererが付ける。翻訳者が記号の一貫性を預からない。
    let mut offenders = Vec::new();
    for (name, text) in resources() {
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
}

#[test]
fn no_resource_carries_an_emoji() {
    let mut offenders = Vec::new();
    for (name, text) in resources() {
        for (index, line) in text.lines().enumerate() {
            for character in line.chars() {
                let point = character as u32;
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
}
