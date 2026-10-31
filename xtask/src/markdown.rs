//! Markdownの段落が途中で改行されていないことを確認する。
//!
//! 改行は段落の終わりにだけ置く。表示幅に合わせた折り返しは表示側が行うものであり、文の途中に
//! 文字数で改行を入れない。英語も日本語も同じである。
//!
//! 引数なしで実行すると、repositoryが追跡する`.md`と未追跡の`.md`をすべて確認する。fileを
//! 引数に渡せばそのfileだけを確認する。`--fix`を付けると、段落の続きの行を前の行へ結合して
//! 書き戻す。code block、front matter、表、HTML、見出し、引用、水平線は触らない。
//!
//! 結合するときの区切りは、境目のどちらかがCJKの文字なら空白なし、どちらもASCIIなら空白1つと
//! する。日本語の文中に英単語が2語並ぶ箇所(例: `unit test`)は後者に当たる。

use std::ffi::OsStr;
use std::io::{self, Write};
use std::ops::RangeInclusive;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::process::Command;

#[cfg(test)]
#[path = "markdown_test.rs"]
mod markdown_test;

const CJK_RANGES: [RangeInclusive<char>; 9] = [
    '\u{3000}'..='\u{303F}', // CJKの記号と句読点
    '\u{3040}'..='\u{309F}', // ひらがな
    '\u{30A0}'..='\u{30FF}', // カタカナ
    '\u{31F0}'..='\u{31FF}', // カタカナ拡張
    '\u{3300}'..='\u{33FF}', // CJK互換
    '\u{3400}'..='\u{4DBF}', // CJK統合漢字拡張A
    '\u{4E00}'..='\u{9FFF}', // CJK統合漢字
    '\u{F900}'..='\u{FAFF}', // CJK互換漢字
    '\u{FF00}'..='\u{FFEF}', // 全角形
];

/// 検査または結合を行い、すべてのfileが規約を満たしていれば`true`を返す。
///
/// `--fix`のときは結合したfileを`out`へ、検査のときは違反を`err`へ書く。
pub(crate) fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> io::Result<bool> {
    let fix = args.iter().any(|arg| arg == "--fix");
    let mut paths: Vec<PathBuf> = args
        .iter()
        .filter(|arg| *arg != "--fix")
        .map(PathBuf::from)
        .collect();
    if paths.is_empty() {
        paths = list_markdown_files()?;
    }

    let mut failures = Vec::new();
    for path in &paths {
        let text = std::fs::read_to_string(path).map_err(|error| {
            io::Error::new(error.kind(), format!("{}: {error}", path.display()))
        })?;
        let unwrapped = unwrap(&text);
        if unwrapped.joined == text {
            continue;
        }
        if fix {
            std::fs::write(path, &unwrapped.joined)?;
            writeln!(
                out,
                "{}: {}行を前の行へ結合した",
                path.display(),
                unwrapped.joined_lines.len()
            )?;
        } else {
            failures.extend(
                unwrapped
                    .joined_lines
                    .iter()
                    .map(|line| format!("{}:{line}: 段落の途中で改行されている", path.display())),
            );
        }
    }

    if failures.is_empty() {
        return Ok(true);
    }
    for failure in &failures {
        writeln!(err, "{failure}")?;
    }
    writeln!(
        err,
        "\n{}箇所。改行は段落の終わりにだけ置き、文の途中では改行しない。\
         `mise run fmt`か`cargo xtask markdown --fix`で結合できる。",
        failures.len()
    )?;
    Ok(false)
}

/// repositoryが追跡する`.md`と、未追跡でignoreされていない`.md`を、current directoryから
/// の相対pathで返す。
fn list_markdown_files() -> io::Result<Vec<PathBuf>> {
    let output = Command::new("git")
        .args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
            "--",
            "*.md",
        ])
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "`git ls-files`が失敗した: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let mut paths: Vec<PathBuf> = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| PathBuf::from(OsStr::from_bytes(part)))
        .collect();
    paths.sort();
    paths.dedup();
    Ok(paths)
}

/// 段落の続きの行を前の行へ結合した結果。
pub(crate) struct Unwrapped {
    /// 結合後の本文。
    pub(crate) joined: String,
    /// 前の行へ結合した行の、元の本文での行番号(1始まり)。
    pub(crate) joined_lines: Vec<usize>,
}

/// 段落の続きの行を前の行へ結合する。
pub(crate) fn unwrap(text: &str) -> Unwrapped {
    let mut unwrapper = Unwrapper::default();
    let mut lines = text
        .split('\n')
        .enumerate()
        .map(|(index, line)| (index + 1, line))
        .peekable();
    if lines.peek().is_some_and(|(_, line)| line.trim() == "---") {
        // front matter。閉じる`---`まで触らない。
        for (_, line) in lines.by_ref() {
            unwrapper.out.push(line.to_owned());
            if unwrapper.out.len() > 1 && line.trim() == "---" {
                break;
            }
        }
    }
    for (number, line) in lines {
        unwrapper.feed(number, line);
    }
    unwrapper.finish()
}

/// code blockを囲む行。同じ記号を同じ数以上並べた行で閉じる。
#[derive(Clone, Copy, PartialEq, Eq)]
struct Fence {
    marker: char,
    length: usize,
}

/// 段落の外から見た行の種類。
enum Kind {
    /// code blockの始まり。
    FenceOpen(Fence),
    /// 空行。段落とHTML blockを閉じる。
    Blank,
    /// HTML blockの始まり。次の空行まで触らない。
    HtmlOpen,
    /// 1行で完結し、前後と結合しないもの。表の行、見出し、水平線、引用。
    Standalone,
    /// list item。続く行を取り込む段落を新しく始める。
    ListItem,
    /// 4つ以上の空白で始まる行。段落の外ではcode blockであり、段落の中では続きの行である。
    IndentedCode,
    /// 本文。段落を始めるか、前の段落へ続く。
    Text,
}

#[derive(Default)]
struct Unwrapper {
    out: Vec<String>,
    joined_lines: Vec<usize>,
    /// 組み立て中の段落。空なら段落の外にいる。
    paragraph: Vec<(usize, String)>,
    /// code blockの中にいる間だけ持つ。
    fence: Option<Fence>,
    in_html: bool,
}

impl Unwrapper {
    fn feed(&mut self, number: usize, line: &str) {
        if let Some(fence) = self.fence {
            if closes(fence, line) {
                self.fence = None;
            }
            self.out.push(line.to_owned());
            return;
        }
        match classify(line) {
            Kind::FenceOpen(fence) => {
                self.flush();
                self.fence = Some(fence);
                self.out.push(line.to_owned());
            }
            Kind::Blank => {
                self.flush();
                self.in_html = false;
                self.out.push(line.to_owned());
            }
            _ if self.in_html => self.out.push(line.to_owned()),
            Kind::HtmlOpen => {
                self.flush();
                self.in_html = true;
                self.out.push(line.to_owned());
            }
            Kind::Standalone => {
                self.flush();
                self.out.push(line.to_owned());
            }
            Kind::ListItem => {
                self.flush();
                self.paragraph.push((number, line.to_owned()));
            }
            Kind::IndentedCode if self.paragraph.is_empty() => self.out.push(line.to_owned()),
            Kind::IndentedCode | Kind::Text => self.paragraph.push((number, line.to_owned())),
        }
    }

    /// 組み立て中の段落を1行にして出力へ移す。
    fn flush(&mut self) {
        let mut paragraph = std::mem::take(&mut self.paragraph).into_iter();
        let Some((_, first)) = paragraph.next() else {
            return;
        };
        let mut joined = first.trim_end().to_owned();
        for (number, line) in paragraph {
            let rest = line.trim();
            self.joined_lines.push(number);
            joined.push_str(separator(&joined, rest));
            joined.push_str(rest);
        }
        self.out.push(joined);
    }

    fn finish(mut self) -> Unwrapped {
        self.flush();
        Unwrapped {
            joined: self.out.join("\n"),
            joined_lines: self.joined_lines,
        }
    }
}

fn classify(line: &str) -> Kind {
    if let Some((fence, _)) = fence_of(line) {
        return Kind::FenceOpen(fence);
    }
    if line.trim().is_empty() {
        return Kind::Blank;
    }
    if is_html_block(line) {
        return Kind::HtmlOpen;
    }
    if is_table_row(line) || is_heading(line) || is_horizontal_rule(line) || is_blockquote(line) {
        return Kind::Standalone;
    }
    if is_list_item(line) {
        return Kind::ListItem;
    }
    if is_indented_code(line) {
        return Kind::IndentedCode;
    }
    Kind::Text
}

/// 境目のどちらかがCJKの文字なら空白を入れず、どちらもそうでなければ空白1つを入れる。
fn separator(before: &str, after: &str) -> &'static str {
    let touches_cjk =
        before.chars().last().is_some_and(is_cjk) || after.chars().next().is_some_and(is_cjk);
    if touches_cjk { "" } else { " " }
}

fn is_cjk(ch: char) -> bool {
    CJK_RANGES.iter().any(|range| range.contains(&ch))
}

/// 3つ以上の`` ` ``か`~`で始まる行なら、その囲みと、記号より後ろを返す。
fn fence_of(line: &str) -> Option<(Fence, &str)> {
    let rest = line.trim_start();
    let marker = rest.chars().next().filter(|ch| matches!(ch, '`' | '~'))?;
    let after = rest.trim_start_matches(marker);
    let length = rest.len() - after.len();
    (length >= 3).then_some((Fence { marker, length }, after))
}

fn closes(fence: Fence, line: &str) -> bool {
    fence_of(line).is_some_and(|(closing, after)| {
        closing.marker == fence.marker && closing.length >= fence.length && after.trim().is_empty()
    })
}

fn is_html_block(line: &str) -> bool {
    line.trim_start()
        .strip_prefix('<')
        .and_then(|rest| rest.chars().next())
        .is_some_and(|ch| ch.is_ascii_alphabetic() || matches!(ch, '/' | '!'))
}

fn is_table_row(line: &str) -> bool {
    line.trim_start().starts_with('|')
}

fn is_blockquote(line: &str) -> bool {
    line.trim_start().starts_with('>')
}

/// 先頭の空白を3つまで除く。見出しと水平線は3つまでの字下げを許す。
fn after_indent(mut line: &str) -> &str {
    for _ in 0..3 {
        match line.strip_prefix(char::is_whitespace) {
            Some(rest) => line = rest,
            None => break,
        }
    }
    line
}

fn is_heading(line: &str) -> bool {
    let rest = after_indent(line);
    let hashes = rest.chars().take_while(|ch| *ch == '#').count();
    (1..=6).contains(&hashes) && rest.chars().nth(hashes).is_none_or(char::is_whitespace)
}

fn is_horizontal_rule(line: &str) -> bool {
    let rest = after_indent(line);
    let Some(marker) = rest
        .chars()
        .next()
        .filter(|ch| matches!(ch, '-' | '*' | '_'))
    else {
        return false;
    };
    rest.chars().filter(|ch| *ch == marker).count() >= 3
        && rest.chars().all(|ch| ch == marker || ch.is_whitespace())
}

/// `-`、`*`、`+`か、数字の列に`.`か`)`を添えた印に、空白が続く行。
fn is_list_item(line: &str) -> bool {
    let rest = line.trim_start();
    let after_marker = if let Some(after) = rest.strip_prefix(['-', '*', '+']) {
        Some(after)
    } else {
        let after_digits = rest.trim_start_matches(|ch: char| ch.is_ascii_digit());
        let has_digits = after_digits.len() < rest.len();
        has_digits
            .then(|| after_digits.strip_prefix(['.', ')']))
            .flatten()
    };
    after_marker.is_some_and(|after| after.starts_with(char::is_whitespace))
}

fn is_indented_code(line: &str) -> bool {
    let after_spaces = line.trim_start_matches(' ');
    line.len() - after_spaces.len() >= 4 && after_spaces.starts_with(|ch: char| !ch.is_whitespace())
}
