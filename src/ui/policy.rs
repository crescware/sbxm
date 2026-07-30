//! 色と文字集合を出すかどうかの判定。
//!
//! 判定は純粋関数へ閉じ、`std::env`と`IsTerminal`を読むのは本moduleのadapterだけとする。
//! 判定そのものを環境から切り離さないと、testが全分岐を確かめるためにprocess全体の
//! 環境変数を書き換えることになり、並行実行で結果が混ざる。
//!
//! streamごとに独立して判定する。stdoutをpipeしてstderrを端末に残した場合、正常結果は
//! plain text、進捗と診断は色付きになる。

/// `--color`が受け取る三値。
///
/// 値の一覧と表記をここだけが持つ。parserもhelpもこの宣言から導出する。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorMode {
    /// streamがTTYのときだけ色を出す。
    #[default]
    Auto,
    /// redirect先にも色を出す。利用者が明示した場合だけ選べる。
    Always,
    /// 色を出さない。
    Never,
}

impl ColorMode {
    /// 受け付ける値の全体。helpの一覧もparserの候補もここから作る。
    pub const ALL: [ColorMode; 3] = [ColorMode::Auto, ColorMode::Always, ColorMode::Never];

    /// 翻訳しない安定した表記。
    pub fn as_str(self) -> &'static str {
        match self {
            ColorMode::Auto => "auto",
            ColorMode::Always => "always",
            ColorMode::Never => "never",
        }
    }

    /// `--color`が受け付ける厳密な値。
    pub fn parse_exact(value: &str) -> Option<ColorMode> {
        ColorMode::ALL
            .into_iter()
            .find(|mode| mode.as_str() == value)
    }
}

impl std::fmt::Display for ColorMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// markerと罫線に使える文字の範囲。
///
/// localeから推測しない。Unicodeを既定とし、`TERM=dumb`のように端末側が表示を
/// 保証できない場合だけASCIIへ落とす。罫線を意味の必須要素にしないため、どちらでも
/// 情報は同じである。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterSet {
    Unicode,
    Ascii,
}

/// 色判定が読む環境変数の観測値。
///
/// 値そのものではなく観測結果を持つことで、判定を純粋関数にする。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Environment {
    /// `NO_COLOR`が存在するか。値は問わず、空文字もopt-outとして扱う。
    pub no_color: bool,
    /// `CLICOLOR_FORCE`の値。
    pub clicolor_force: Option<String>,
    /// `TERM`の値。
    pub term: Option<String>,
}

impl Environment {
    /// 実行中のprocessの環境変数を読む。
    pub fn detect() -> Environment {
        Environment {
            no_color: std::env::var_os("NO_COLOR").is_some(),
            clicolor_force: std::env::var("CLICOLOR_FORCE").ok(),
            term: std::env::var("TERM").ok(),
        }
    }
}

/// streamごとのTTY判定と端末幅。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Terminals {
    pub stdout_is_tty: bool,
    pub stderr_is_tty: bool,
    /// 端末の桁数。読めない場合は`None`。
    pub width: Option<usize>,
}

impl Terminals {
    /// 実行中のprocessのstreamを観測する。
    pub fn detect() -> Terminals {
        use std::io::IsTerminal;
        let terminal = console::Term::stderr();
        Terminals {
            stdout_is_tty: std::io::stdout().is_terminal(),
            stderr_is_tty: std::io::stderr().is_terminal(),
            width: terminal.is_term().then(|| terminal.size().1 as usize),
        }
    }
}

/// 1 streamの描画条件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamPolicy {
    pub color: bool,
    pub characters: CharacterSet,
    /// 端末の桁数。折り返しではなく、prompt labelの省略にだけ使う。
    pub width: Option<usize>,
}

impl StreamPolicy {
    /// ANSIを一切出さないstream。testと非TTYの既定。
    pub fn plain() -> StreamPolicy {
        StreamPolicy {
            color: false,
            characters: CharacterSet::Unicode,
            width: None,
        }
    }

    /// 色を出すstream。
    #[cfg(test)]
    pub fn colored() -> StreamPolicy {
        StreamPolicy {
            color: true,
            ..StreamPolicy::plain()
        }
    }

    /// ASCII glyphだけを使うstream。
    #[cfg(test)]
    pub fn ascii() -> StreamPolicy {
        StreamPolicy {
            characters: CharacterSet::Ascii,
            ..StreamPolicy::plain()
        }
    }
}

/// 1実行ぶんの描画条件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputPolicy {
    pub stdout: StreamPolicy,
    pub stderr: StreamPolicy,
}

impl OutputPolicy {
    /// 明示option、環境変数、TTYからstreamごとの条件を決める。
    ///
    /// 優先順位は次のとおりとし、CIかどうかを独自に推測しない。
    ///
    /// 1. 明示的な`--color=always|never|auto`
    /// 2. `NO_COLOR`が存在すれば`Never`
    /// 3. `CLICOLOR_FORCE`が`0`以外なら`Always`
    /// 4. `TERM=dumb`なら`Never`
    /// 5. `Auto`は対象streamがTTYのときだけ有効
    pub fn resolve(
        mode: ColorMode,
        environment: &Environment,
        terminals: &Terminals,
    ) -> OutputPolicy {
        let characters = if is_dumb(environment) {
            CharacterSet::Ascii
        } else {
            CharacterSet::Unicode
        };
        OutputPolicy {
            stdout: StreamPolicy {
                color: color_for(mode, environment, terminals.stdout_is_tty),
                characters,
                width: terminals.width,
            },
            stderr: StreamPolicy {
                color: color_for(mode, environment, terminals.stderr_is_tty),
                characters,
                width: terminals.width,
            },
        }
    }

    /// ANSIを一切出さない条件。localeを決める前のerror報告とtestに使う。
    pub fn plain() -> OutputPolicy {
        OutputPolicy {
            stdout: StreamPolicy::plain(),
            stderr: StreamPolicy::plain(),
        }
    }
}

/// 1 streamの色可否。
fn color_for(mode: ColorMode, environment: &Environment, is_tty: bool) -> bool {
    match mode {
        // 明示指定はredirect先にも従う。利用者が選んだ以上、環境変数で覆さない。
        ColorMode::Always => return true,
        ColorMode::Never => return false,
        ColorMode::Auto => {}
    }
    if environment.no_color {
        return false;
    }
    if environment
        .clicolor_force
        .as_deref()
        .is_some_and(|value| value != "0")
    {
        return true;
    }
    if is_dumb(environment) {
        return false;
    }
    is_tty
}

fn is_dumb(environment: &Environment) -> bool {
    environment.term.as_deref() == Some("dumb")
}

#[cfg(test)]
#[path = "policy_test.rs"]
mod policy_test;
