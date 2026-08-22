use super::{CharacterSet, ColorMode, Environment, StreamPolicy, Terminals};

/// 1実行ぶんの描画条件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderingPolicy {
    pub stdout: StreamPolicy,
    pub stderr: StreamPolicy,
}

impl RenderingPolicy {
    /// 現在のprocessの環境と端末から描画条件を決める。
    pub fn detect(mode: ColorMode) -> RenderingPolicy {
        Self::resolve(mode, &Environment::detect(), &Terminals::detect())
    }

    /// 明示option、環境変数、TTYからstreamごとの条件を決める。
    ///
    /// 優先順位は次のとおりとし、CIかどうかを独自に推測しない。
    ///
    /// 1. 明示的な`--color=always|never|auto`
    /// 2. `NO_COLOR`が存在すれば`Never`
    /// 3. `CLICOLOR_FORCE`が`0`以外なら`Always`
    /// 4. `TERM=dumb`なら`Never`
    /// 5. `Auto``は対象streamがTTYのときだけ有効`
    pub(super) fn resolve(
        mode: ColorMode,
        environment: &Environment,
        terminals: &Terminals,
    ) -> RenderingPolicy {
        let characters = if is_dumb(environment) {
            CharacterSet::Ascii
        } else {
            CharacterSet::Unicode
        };
        RenderingPolicy {
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
#[path = "rendering_policy_test.rs"]
mod rendering_policy_test;
