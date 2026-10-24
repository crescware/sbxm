use std::time::Duration;

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::diagnostics::ErrorId;

/// 3つのcommandの結果を並べた、手順の出力。
///
/// 2つ目は末尾の改行を持たず、3つ目はNULを含む。どちらもcommandが書いたとおりに届く。
/// 1つ目だけが所要時間を持つ。
const THREE: &[u8] = b"0 0 6 0 1234567\nhello\n1 2 3 10 -\nabcnot found\n2 0 4 0 -\na\0b\0";

#[test]
fn each_record_carries_its_status_stdout_stderr_and_elapsed_as_written() -> Checked {
    let records = parse_inside_check(THREE, 3).required()?;
    assert_eq!(
        records,
        [
            InsideRecord {
                status: 0,
                stdout: b"hello\n".to_vec(),
                stderr: Vec::new(),
                elapsed: Some(Duration::from_nanos(1_234_567)),
            },
            InsideRecord {
                status: 2,
                stdout: b"abc".to_vec(),
                stderr: b"not found\n".to_vec(),
                elapsed: None,
            },
            InsideRecord {
                status: 0,
                stdout: b"a\0b\0".to_vec(),
                stderr: Vec::new(),
                elapsed: None,
            },
        ]
    );
    Ok(())
}

#[test]
fn no_command_reads_as_an_empty_output() -> Checked {
    assert_eq!(parse_inside_check(b"", 0).required()?, Vec::new());
    Ok(())
}

#[test]
fn an_output_that_stops_short_is_not_read_as_fewer_commands() -> Checked {
    // 3つ目の見出しが無い。後ろのcommandが無かったのではなく、手順が途中で止まっている。
    let cut = &THREE[..THREE.len() - 12];
    let error = parse_inside_check(cut, 3).refused_because("a record is missing")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    Ok(())
}

#[test]
fn malformed_headers_and_leftover_bytes_are_all_refused() -> Checked {
    let cases: [(&[u8], usize); 10] = [
        // 見出しの列が足りない。
        (b"0 0 6 0\nhello\n", 1),
        // 見出しの番号が順どおりでない。
        (b"1 0 6 0 -\nhello\n", 1),
        // 終了statusの範囲を超える。
        (b"0 256 0 0 -\n", 1),
        (b"0 -1 0 0 -\n", 1),
        // byte数が数でない。
        (b"0 0 six 0 -\nhello\n", 1),
        (b"0 0 6 none -\nhello\n", 1),
        // 所要時間が、計らなかった印でもnano秒の整数でもない。
        (b"0 0 6 0 fast\nhello\n", 1),
        (b"0 0 6 0 -5\nhello\n", 1),
        // 告げた長さのbyte列が無い。
        (b"0 0 6 0 -\nhell", 1),
        // 最後の結果の後ろに余りがある。
        (b"0 0 6 0 -\nhello\nextra", 1),
    ];
    for (output, expected) in cases {
        let error = parse_inside_check(output, expected)
            .refused_because(&format!("malformed output is rejected: {output:?}"))?;
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    }
    Ok(())
}

fn two_commands() -> Vec<Vec<String>> {
    vec![
        vec!["printenv".to_string(), "SSH_AUTH_SOCK".to_string()],
        vec!["ssh-add".to_string(), "-L".to_string()],
    ]
}

#[test]
fn the_arguments_put_an_empty_separator_after_every_command() {
    assert_eq!(
        inside_check_arguments(&two_commands(), false),
        [
            "sh",
            "-c",
            INSIDE_CHECK,
            "sh",
            "printenv",
            "SSH_AUTH_SOCK",
            "",
            "ssh-add",
            "-L",
            "",
        ]
    );
}

#[test]
fn a_timed_check_asks_the_procedure_before_the_first_command() {
    assert_eq!(
        inside_check_arguments(&two_commands(), true)[..6],
        ["sh", "-c", INSIDE_CHECK, "sh", "-t", "printenv"]
    );
}

#[test]
fn the_exec_arguments_are_read_back_into_the_commands_they_carry() {
    for timed in [false, true] {
        let mut args = vec!["exec".to_string(), "org-repo".to_string(), "--".to_string()];
        args.extend(inside_check_arguments(&two_commands(), timed));
        assert_eq!(
            split_inside_check(&args),
            Some(("org-repo".to_string(), timed, two_commands()))
        );
    }
}

#[test]
fn an_exec_that_is_not_the_check_is_not_split() {
    let plain = ["exec", "org-repo", "--", "printenv", "SSH_AUTH_SOCK"].map(String::from);
    assert_eq!(split_inside_check(&plain), None);
    // 手順の形だが、別のscriptを渡している。
    let other = ["exec", "org-repo", "--", "sh", "-c", "true", "sh", "x", ""].map(String::from);
    assert_eq!(split_inside_check(&other), None);
    // Sandbox名が無い。
    let unnamed = ["--", "sh", "-c", INSIDE_CHECK, "sh"].map(String::from);
    assert_eq!(split_inside_check(&unnamed), None);
}

#[test]
fn the_check_announces_itself_on_its_first_line() {
    // testのhostの代役は、この行で手順を見分けて、commandごとの答えを並べる。
    assert!(INSIDE_CHECK.starts_with("# sbxm inside check\n"));
}
