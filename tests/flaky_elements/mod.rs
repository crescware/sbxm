//! flakyになりうる要素の検出。
//!
//! 要素とは、実行のたびに順序や時機が変わりうるものを指す。実時間、threadの順序、並行して
//! 動く子process、signal、書いた直後の実行可能file、file lockの取り合いである。完了まで
//! 同期的に待つだけの外部command実行（`output()`）は、順序の競合がないため数えない。
//!
//! 綴りの一致では、threadとCommandの`spawn`も、codeと文字列の`kill`も区別できず、別名でも
//! 逃げられる。sourceをtoken列として読み、`use`が導入した名前を完全修飾pathへ戻してから
//! 判定する。macroの引数もtoken列のまま同じように読む。`use`は使った場所で数え、他のfileへ
//! 渡す`pub use`だけはそれ自体を数える。
//!
//! methodは型を見ずに名前で判定する。`spawn`だけは、closureを受け取ればthreadの`spawn`、
//! そうでなければ`Command::spawn`とそれを包んだものとして分ける。
//!
//! 文字列literalは、testや外部processが実行するscriptとして行ごとに読む。`sleep`は実時間、
//! 背景の`&`は並行して動く子process、`kill`はsignal、`#!`で始まる中身は書いた直後に実行する
//! fileである。doc commentは読まない。
//!
//! `include_str!`・`include_bytes!`が読み込むfileの中身も、読み込んだ行の文字列literalとして
//! 数える。ここは読み込むpathを返し、fileを開いて`script_elements`で読むのは呼び出し側
//! （`tests/architecture.rs`）である。pathが文字列literalでなければ、どのfileかを決めずに
//! `None`として返す。
//!
//! OS層（`crate::boundary::os`）を名指しすることは`OsLayer`として数える。`crate`から始まる
//! pathは、このためだけに読む。
//!
//! 検出しないものもある。環境変数の値、一時directoryの共有、fakeの中での順序は、fixtureが毎回
//! 同じ値を与えることで守る。marker fileの出現を待つloopは、待つための実時間で検出する。
//! `Mutex::lock`と綴りが同じ`File::lock`のmethod呼び出しは見ない。OS層の外で要素を包んで
//! 公開すると、数えられるのは包んだfileだけである。OS層を包んだ本番codeを通して実OSを
//! 動かすtestも、名指ししない限り数えられない。実行時にpathで開くfile
//! （`tests/fixtures/fake_tool.sh`など）は読まない。

use std::collections::{BTreeMap, BTreeSet};

use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};
use syn::visit::Visit;

/// 要素の種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Element {
    /// `Instant::now`、`SystemTime::now`、`thread::sleep`、`elapsed()`、`recv_timeout`、
    /// scriptの`sleep`。
    RealTime,
    /// `thread::spawn`・`Builder`・`scope`、`mpsc`、`Condvar`、`Barrier`。
    Thread,
    /// `spawn()`、`Child`、`try_wait`、`wait`、PTY、I/O待ち（`rustix::event`）、scriptの`&`。
    ChildProcess,
    /// `signal_hook`、`kill`、`Signal`、scriptの`kill`。
    Signal,
    /// `#!`で始まる中身。書いた直後にexecすると、別threadがforkした子が書き込み端を持つ間は
    /// `ETXTBSY`で起動できない。
    WrittenExecutable,
    /// `File::try_lock`・`unlock`などのfile lock。
    FileLock,
    /// OS層（`crate::boundary::os`）の実物。OS層の外のtestが使えば、実OSを動かす。
    OsLayer,
}

/// 見つかった要素1つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub element: Element,
    pub line: usize,
    pub spelling: String,
}

/// `include_str!`・`include_bytes!`が読み込むfile1つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Included {
    pub line: usize,
    /// 書かれたままの相対path。文字列literalでなければ`None`。
    pub path: Option<String>,
}

/// 完全修飾pathの先頭が一致すれば要素とみなすもの。上から順に照らし、最初の一致を採る。
const PATHS: [(&[&str], Element); 28] = [
    (&["std", "time", "Instant", "now"], Element::RealTime),
    (&["std", "time", "SystemTime", "now"], Element::RealTime),
    (&["std", "thread", "sleep"], Element::RealTime),
    (&["std", "thread"], Element::Thread),
    (&["std", "sync", "mpsc"], Element::Thread),
    (&["std", "sync", "Condvar"], Element::Thread),
    (&["std", "sync", "Barrier"], Element::Thread),
    (&["std", "process", "Child"], Element::ChildProcess),
    (&["std", "process", "ChildStdin"], Element::ChildProcess),
    (&["std", "process", "ChildStdout"], Element::ChildProcess),
    (&["std", "process", "ChildStderr"], Element::ChildProcess),
    (
        &["std", "process", "Command", "spawn"],
        Element::ChildProcess,
    ),
    (&["rustix", "event"], Element::ChildProcess),
    (&["rustix", "pty"], Element::ChildProcess),
    (&["rustix", "termios"], Element::ChildProcess),
    (&["rustix", "process", "waitpid"], Element::ChildProcess),
    (&["rustix", "process", "waitid"], Element::ChildProcess),
    (&["rustix", "process", "kill_process"], Element::Signal),
    (
        &["rustix", "process", "kill_process_group"],
        Element::Signal,
    ),
    (
        &["rustix", "process", "kill_current_process_group"],
        Element::Signal,
    ),
    (&["rustix", "process", "Signal"], Element::Signal),
    (&["signal_hook"], Element::Signal),
    (&["std", "fs", "File", "try_lock"], Element::FileLock),
    (&["std", "fs", "File", "try_lock_shared"], Element::FileLock),
    (&["std", "fs", "File", "lock"], Element::FileLock),
    (&["std", "fs", "File", "lock_shared"], Element::FileLock),
    (&["std", "fs", "File", "unlock"], Element::FileLock),
    (&["crate", "boundary", "os"], Element::OsLayer),
];

/// 名前だけで要素と分かるmethod。受け手の型は見ない。どれも受け手が何であれ要素である。
///
/// `spawn`はclosureを受け取ればthread、そうでなければ子processである。
const METHODS: [(&str, Element); 13] = [
    ("elapsed", Element::RealTime),
    ("recv_timeout", Element::RealTime),
    ("spawn", Element::ChildProcess),
    ("try_wait", Element::ChildProcess),
    ("wait", Element::ChildProcess),
    ("kill", Element::Signal),
    ("wait_timeout", Element::Thread),
    ("wait_while", Element::Thread),
    ("wait_timeout_while", Element::Thread),
    ("try_lock", Element::FileLock),
    ("try_lock_shared", Element::FileLock),
    ("lock_shared", Element::FileLock),
    ("unlock", Element::FileLock),
];

/// `use`が導入した名前から完全修飾pathへの対応。
#[derive(Default)]
struct Imports {
    names: BTreeMap<String, Vec<String>>,
    globs: Vec<Vec<String>>,
    found: Vec<Found>,
    /// 読んでいる`use`が他のfileへ名前を渡すか。
    exported: bool,
}

impl Imports {
    fn collect(&mut self, prefix: &mut Vec<String>, tree: &syn::UseTree) {
        match tree {
            syn::UseTree::Path(path) => {
                prefix.push(path.ident.to_string());
                self.collect(prefix, &path.tree);
                prefix.pop();
            }
            syn::UseTree::Name(name) => {
                let ident = name.ident.to_string();
                let full = if ident == "self" {
                    prefix.clone()
                } else {
                    let mut full = prefix.clone();
                    full.push(ident.clone());
                    full
                };
                let alias = if ident == "self" {
                    prefix.last().cloned().unwrap_or_default()
                } else {
                    ident
                };
                self.import(alias, full, name.ident.span().start().line);
            }
            syn::UseTree::Rename(rename) => {
                let mut full = prefix.clone();
                if rename.ident != "self" {
                    full.push(rename.ident.to_string());
                }
                self.import(
                    rename.rename.to_string(),
                    full,
                    rename.ident.span().start().line,
                );
            }
            syn::UseTree::Glob(glob) => {
                if let Some(element) = element_of(prefix) {
                    self.found.push(Found {
                        element,
                        line: glob.star_token.spans[0].start().line,
                        spelling: format!("use {}::*", prefix.join("::")),
                    });
                }
                self.globs.push(prefix.clone());
            }
            syn::UseTree::Group(group) => {
                for tree in &group.items {
                    self.collect(prefix, tree);
                }
            }
        }
    }

    fn import(&mut self, alias: String, full: Vec<String>, line: usize) {
        if self.exported
            && let Some(element) = element_of(&full)
        {
            self.found.push(Found {
                element,
                line,
                spelling: format!("use {}", full.join("::")),
            });
        }
        self.names.insert(alias, full);
    }

    /// sourceに綴られたpathを、完全修飾pathの候補へ戻す。
    ///
    /// 先頭が`use`で導入した名前なら置き換える。extern crateから始まるpathはそのまま使う。
    /// 導入されていない1語の名前は、局所変数などでありうるため戻さない。glob importが
    /// あれば、その下にある名前である可能性も候補に含める。
    fn resolve(&self, segments: &[String]) -> Vec<Vec<String>> {
        let Some(first) = segments.first() else {
            return Vec::new();
        };
        if let Some(full) = self.names.get(first) {
            let mut resolved = full.clone();
            resolved.extend(segments[1..].iter().cloned());
            return vec![resolved];
        }
        let mut candidates = Vec::new();
        if segments.len() > 1 && EXTERN_ROOTS.contains(&first.as_str()) {
            candidates.push(segments.to_vec());
        }
        for glob in &self.globs {
            let mut resolved = glob.clone();
            resolved.extend(segments.iter().cloned());
            candidates.push(resolved);
        }
        candidates
    }
}

/// そのまま完全修飾pathとして読める先頭。`crate`から始まるpathは、
/// OS層の実物を見分けるために読む。
const EXTERN_ROOTS: [&str; 6] = ["std", "core", "alloc", "rustix", "signal_hook", "crate"];

impl<'ast> Visit<'ast> for Imports {
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        self.exported = !matches!(item.vis, syn::Visibility::Inherited);
        let mut prefix = Vec::new();
        self.collect(&mut prefix, &item.tree);
    }
}

/// 完全修飾pathが要素か。
fn element_of(full: &[String]) -> Option<Element> {
    PATHS.iter().find_map(|(pattern, element)| {
        (full.len() >= pattern.len()
            && pattern
                .iter()
                .zip(full)
                .all(|(expected, segment)| expected == segment))
        .then_some(*element)
    })
}

/// Rust sourceに現れる要素。
pub fn elements(text: &str) -> Result<Vec<Found>, syn::Error> {
    let (mut found, _) = read(text)?;
    found.sort_by(|left, right| {
        (left.line, left.element, &left.spelling).cmp(&(right.line, right.element, &right.spelling))
    });
    found.dedup();
    Ok(found)
}

/// Rust sourceが`include_str!`・`include_bytes!`で読み込むfile。doc commentの中は読まない。
pub fn included(text: &str) -> Result<Vec<Included>, syn::Error> {
    let (_, included) = read(text)?;
    Ok(included)
}

/// Rust sourceを読み、要素と、読み込むfileを集める。
fn read(text: &str) -> Result<(Vec<Found>, Vec<Included>), syn::Error> {
    let file = syn::parse_file(text)?;
    let mut imports = Imports::default();
    imports.visit_file(&file);
    let tokens: TokenStream = text.parse().map_err(syn::Error::from)?;
    let mut found = std::mem::take(&mut imports.found);
    let mut included = Vec::new();
    scan(&imports, tokens, &mut found, &mut included);
    Ok((found, included))
}

/// token列を読み、pathとmethod呼び出しと文字列literalから要素を、`include_str!`・
/// `include_bytes!`から読み込むfileを集める。
fn scan(
    imports: &Imports,
    tokens: TokenStream,
    found: &mut Vec<Found>,
    included: &mut Vec<Included>,
) {
    let tokens: Vec<TokenTree> = tokens.into_iter().collect();
    let mut index = 0;
    while index < tokens.len() {
        match &tokens[index] {
            // doc commentは`#[doc = "..."]`として現れる。説明の文はscriptではない。
            TokenTree::Punct(punct) if punct.as_char() == '#' => {
                let next = if matches!(tokens.get(index + 1), Some(TokenTree::Punct(bang)) if bang.as_char() == '!')
                {
                    index + 2
                } else {
                    index + 1
                };
                if let Some(TokenTree::Group(group)) = tokens.get(next)
                    && group.delimiter() == Delimiter::Bracket
                    && is_doc(&group.stream())
                {
                    index = next + 1;
                    continue;
                }
                index += 1;
            }
            TokenTree::Punct(punct) if punct.as_char() == '.' => {
                let previous_is_dot = index > 0
                    && matches!(&tokens[index - 1], TokenTree::Punct(dot) if dot.as_char() == '.');
                if !previous_is_dot
                    && let Some(TokenTree::Ident(method)) = tokens.get(index + 1)
                    && let Some(TokenTree::Group(arguments)) = tokens.get(index + 2)
                    && arguments.delimiter() == Delimiter::Parenthesis
                    && let Some((_, element)) = METHODS.iter().find(|(name, _)| method == name)
                {
                    let closure = starts_a_closure(&arguments.stream());
                    let element = if method == "spawn" && closure {
                        Element::Thread
                    } else {
                        *element
                    };
                    found.push(Found {
                        element,
                        line: method.span().start().line,
                        spelling: format!(".{method}({})", if closure { "|| ..." } else { "" }),
                    });
                }
                index += 1;
            }
            // `use`は導入した名前を使った場所で数える。
            TokenTree::Ident(ident) if ident == "use" => {
                while index < tokens.len()
                    && !matches!(&tokens[index], TokenTree::Punct(end) if end.as_char() == ';')
                {
                    index += 1;
                }
                index += 1;
            }
            TokenTree::Ident(ident) => {
                if (ident == "include_str" || ident == "include_bytes")
                    && matches!(tokens.get(index + 1), Some(TokenTree::Punct(bang)) if bang.as_char() == '!')
                    && let Some(TokenTree::Group(arguments)) = tokens.get(index + 2)
                {
                    included.push(Included {
                        line: ident.span().start().line,
                        path: literal_path(&arguments.stream()),
                    });
                }
                let (segments, line, end) = path_at(&tokens, index);
                for candidate in imports.resolve(&segments) {
                    if let Some(element) = element_of(&candidate) {
                        found.push(Found {
                            element,
                            line,
                            spelling: segments.join("::"),
                        });
                        break;
                    }
                }
                index = end;
            }
            TokenTree::Literal(literal) => {
                if let Some(text) = string_value(&literal.to_string()) {
                    for (element, spelling) in script_elements(&text) {
                        found.push(Found {
                            element,
                            line: literal.span().start().line,
                            spelling,
                        });
                    }
                }
                index += 1;
            }
            TokenTree::Group(group) => {
                scan(imports, group.stream(), found, included);
                index += 1;
            }
            TokenTree::Punct(_) => index += 1,
        }
    }
}

/// macroの引数が文字列literal1つだけなら、その値。
fn literal_path(stream: &TokenStream) -> Option<String> {
    let arguments: Vec<TokenTree> = stream.clone().into_iter().collect();
    match arguments.as_slice() {
        [TokenTree::Literal(literal)] => string_value(&literal.to_string()),
        _ => None,
    }
}

/// 引数がclosureから始まるか。
fn starts_a_closure(stream: &TokenStream) -> bool {
    match stream.clone().into_iter().next() {
        Some(TokenTree::Punct(bar)) => bar.as_char() == '|',
        Some(TokenTree::Ident(keyword)) => keyword == "move",
        _ => false,
    }
}

/// 属性の中身が`doc = ...`か。
fn is_doc(stream: &TokenStream) -> bool {
    matches!(stream.clone().into_iter().next(), Some(TokenTree::Ident(ident)) if ident == "doc")
}

/// `index`から始まる`a::b::c`を読み、語の列と行と、続きの位置を返す。
fn path_at(tokens: &[TokenTree], index: usize) -> (Vec<String>, usize, usize) {
    let mut segments = Vec::new();
    let mut line = 0;
    let mut at = index;
    while let Some(TokenTree::Ident(ident)) = tokens.get(at) {
        if segments.is_empty() {
            line = ident.span().start().line;
        }
        segments.push(ident.to_string());
        at += 1;
        let joined = matches!(tokens.get(at), Some(TokenTree::Punct(first)) if first.as_char() == ':' && first.spacing() == Spacing::Joint)
            && matches!(tokens.get(at + 1), Some(TokenTree::Punct(second)) if second.as_char() == ':');
        if !joined {
            break;
        }
        at += 2;
    }
    (segments, line, at)
}

/// 文字列literalの値。文字列でなければ`None`。
fn string_value(token: &str) -> Option<String> {
    if let Ok(literal) = syn::parse_str::<syn::LitStr>(token) {
        return Some(literal.value());
    }
    syn::parse_str::<syn::LitByteStr>(token)
        .ok()
        .map(|literal| String::from_utf8_lossy(&literal.value()).into_owned())
}

/// scriptとして読んだ文字列literalの要素。
pub fn script_elements(text: &str) -> Vec<(Element, String)> {
    let mut found = Vec::new();
    if text.starts_with("#!") {
        found.push((
            Element::WrittenExecutable,
            text.lines().next().unwrap_or_default().to_string(),
        ));
    }
    for line in text.lines() {
        if shell_word_with_argument(line, "sleep", |next| {
            next.is_ascii_digit() || next == '$' || next == '.'
        }) {
            found.push((Element::RealTime, line.trim().to_string()));
        }
        if shell_word_with_argument(line, "kill", |next| {
            next.is_ascii_digit() || matches!(next, '-' | '$' | '%')
        }) {
            found.push((Element::Signal, line.trim().to_string()));
        }
        if runs_in_background(line) {
            found.push((Element::ChildProcess, line.trim().to_string()));
        }
    }
    found
}

/// `word`がshellのcommandとして現れ、空白のあとに`argument`を満たす文字が続くか。
fn shell_word_with_argument(line: &str, word: &str, argument: impl Fn(char) -> bool) -> bool {
    line.match_indices(word).any(|(at, _)| {
        let starts = line[..at].chars().next_back().is_none_or(|before| {
            before.is_whitespace() || matches!(before, ';' | '&' | '|' | '(' | '{')
        });
        let rest = &line[at + word.len()..];
        let spaced = rest.starts_with(char::is_whitespace);
        starts && spaced && rest.trim_start().chars().next().is_some_and(&argument)
    })
}

/// 行に、背景で走らせる`&`があるか。`&&`、`>&`、`&>`、`2>&1`は数えない。
fn runs_in_background(line: &str) -> bool {
    let characters: Vec<char> = line.chars().collect();
    characters.iter().enumerate().any(|(at, &character)| {
        if character != '&' {
            return false;
        }
        let before = at.checked_sub(1).map(|previous| characters[previous]);
        let after = characters.get(at + 1).copied();
        let separated_before = before.is_none_or(|c| c.is_whitespace() || c == ')');
        let separated_after = after.is_none_or(|c| c.is_whitespace() || matches!(c, ')' | ';'));
        separated_before && separated_after
    })
}

/// 要素の種類ごとに、見つかった場所を束ねる。
pub fn by_element(found: &[Found]) -> BTreeMap<Element, Vec<String>> {
    let mut grouped: BTreeMap<Element, Vec<String>> = BTreeMap::new();
    for item in found {
        grouped
            .entry(item.element)
            .or_default()
            .push(format!("{}: {}", item.line, item.spelling));
    }
    grouped
}

/// 見つかった要素の種類。
pub fn kinds(found: &[Found]) -> BTreeSet<Element> {
    found.iter().map(|item| item.element).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds_in(source: &str) -> Result<Vec<Element>, syn::Error> {
        Ok(kinds(&elements(source)?).into_iter().collect())
    }

    #[test]
    fn a_path_is_resolved_through_its_import_and_alias() -> Result<(), syn::Error> {
        assert_eq!(
            kinds_in("use std::thread::sleep as pause;\nfn f() { pause(D); }")?,
            [Element::RealTime]
        );
        assert_eq!(
            kinds_in("use std::time::{Duration, Instant};\nfn f() { Instant::now(); }")?,
            [Element::RealTime]
        );
        assert_eq!(
            kinds_in("fn f() { ::std::thread::sleep(D); }")?,
            [Element::RealTime]
        );
        assert_eq!(
            kinds_in("use std::sync::mpsc::*;\nfn f() { channel::<u8>(); }")?,
            [Element::Thread]
        );
        Ok(())
    }

    #[test]
    fn an_import_counts_where_it_is_used_unless_it_is_passed_on() -> Result<(), syn::Error> {
        // `thread`を導入しても、使ったのが`sleep`だけなら実時間である。
        assert_eq!(
            kinds_in("use std::thread;\nfn f() { thread::sleep(D); }")?,
            [Element::RealTime]
        );
        assert_eq!(
            kinds_in("pub use std::thread::sleep;")?,
            [Element::RealTime]
        );
        assert_eq!(
            kinds_in("pub(crate) use std::thread::spawn as go;")?,
            [Element::Thread]
        );
        Ok(())
    }

    #[test]
    fn the_os_layer_is_found_through_its_crate_path() -> Result<(), syn::Error> {
        assert_eq!(
            kinds_in("use crate::boundary::os::SystemClock;\nfn f() { SystemClock.now(); }")?,
            [Element::OsLayer]
        );
        assert!(kinds_in("use crate::time::Clock;\nfn f(clock: &dyn Clock) {}")?.is_empty());
        Ok(())
    }

    #[test]
    fn a_name_that_was_not_imported_is_not_an_element() -> Result<(), syn::Error> {
        // 同じ綴りでも、導入していなければ局所の名前である。
        assert!(kinds_in("fn f(thread: T, sleep: S) { sleep(thread); spawn(x); }")?.is_empty());
        assert!(kinds_in("use crate::boundary::host::spawn;\nfn f() { spawn(x); }")?.is_empty());
        assert!(
            kinds_in(
                "use std::time::{Duration, SystemTime};\nfn f(t: SystemTime) -> Duration { D }"
            )?
            .is_empty()
        );
        Ok(())
    }

    #[test]
    fn a_method_is_an_element_whatever_it_is_called_on() -> Result<(), syn::Error> {
        assert_eq!(kinds_in("fn f() { c.spawn(); }")?, [Element::ChildProcess]);
        assert_eq!(kinds_in("fn f() { s.spawn(|| g()); }")?, [Element::Thread]);
        assert_eq!(
            kinds_in("fn f() { b.spawn(move || g()); }")?,
            [Element::Thread]
        );
        // closureでなければ、子processを起動する手続きを包んだものである。
        assert_eq!(
            kinds_in("fn f() { host.spawn(&[a]); }")?,
            [Element::ChildProcess]
        );
        assert_eq!(kinds_in("fn f() { s.elapsed(); }")?, [Element::RealTime]);
        assert_eq!(kinds_in("fn f() { c.kill(); }")?, [Element::Signal]);
        assert_eq!(kinds_in("fn f() { f.try_lock(); }")?, [Element::FileLock]);
        assert_eq!(
            kinds_in("use std::fs::File;\nfn f() { File::unlock(&f); }")?,
            [Element::FileLock]
        );
        // 呼び出しでなければfieldである。
        assert!(kinds_in("fn f() { c.kill; }")?.is_empty());
        Ok(())
    }

    #[test]
    fn macro_arguments_are_read_like_code() -> Result<(), syn::Error> {
        assert_eq!(
            kinds_in("fn f() { assert!(s.elapsed() < D); }")?,
            [Element::RealTime]
        );
        assert_eq!(
            kinds_in("use std::thread;\nfn f() { vec![thread::spawn(g)]; }")?,
            [Element::Thread]
        );
        Ok(())
    }

    #[test]
    fn a_script_is_read_line_by_line() {
        assert_eq!(
            script_elements("printf ready\nsleep 30\n"),
            [(Element::RealTime, "sleep 30".to_string())]
        );
        assert_eq!(
            script_elements("(printf x > f) &"),
            [(Element::ChildProcess, "(printf x > f) &".to_string())]
        );
        assert_eq!(
            script_elements("kill -TERM $$"),
            [(Element::Signal, "kill -TERM $$".to_string())]
        );
        assert_eq!(
            script_elements("#!/bin/sh\nexit 0"),
            [(Element::WrittenExecutable, "#!/bin/sh".to_string())]
        );
    }

    #[test]
    fn redirections_and_prose_are_not_a_script() {
        for text in [
            "printf x >&2",
            "cat 2>&1",
            "true && false",
            "a=1&b=2",
            "the child must be killed promptly",
            "sleeps until the lock is free",
            "`sleep`は書き込み直後のfileではない",
        ] {
            assert!(script_elements(text).is_empty(), "{text}");
        }
    }

    #[test]
    fn a_file_read_by_include_is_returned_with_its_line() -> Result<(), syn::Error> {
        assert_eq!(
            included(
                "const S: &str = include_str!(\"a.sh\");\nconst B: &[u8] = include_bytes!(\"b.sh\");"
            )?,
            [
                Included {
                    line: 1,
                    path: Some("a.sh".to_string()),
                },
                Included {
                    line: 2,
                    path: Some("b.sh".to_string()),
                },
            ]
        );
        // literalでなければ、どのfileかを決めない。
        assert_eq!(
            included("const S: &str = include_str!(concat!(\"a\", \".sh\"));")?,
            [Included {
                line: 1,
                path: None,
            }]
        );
        // doc commentとして読み込む文書はscriptではない。
        assert!(included("#![doc = include_str!(\"README.md\")]\nfn f() {}")?.is_empty());
        Ok(())
    }

    #[test]
    fn a_doc_comment_is_not_a_script() -> Result<(), syn::Error> {
        assert!(kinds_in("/// sleep 30 &\nfn f() {}")?.is_empty());
        assert_eq!(
            kinds_in("fn f() { g(\"sleep 30\"); }")?,
            [Element::RealTime]
        );
        Ok(())
    }
}
