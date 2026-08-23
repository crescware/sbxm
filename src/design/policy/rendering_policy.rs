use super::{CharacterSet, ColorMode, ColorSetting, Environment, StreamPolicy, Terminals};

/// 1実行ぶんの描画条件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderingPolicy {
    pub stdout: StreamPolicy,
    pub stderr: StreamPolicy,
}

impl RenderingPolicy {
    /// 現在のprocessの環境と端末から描画条件を決める。
    pub fn detect(setting: ColorSetting) -> RenderingPolicy {
        Self::resolve(setting, &Environment::detect(), &Terminals::detect())
    }

    /// 明示option、環境変数、TTYからstreamごとの条件を決める。
    ///
    /// 優先順位は次のとおりとし、CIかどうかを独自に推測しない。
    ///
    /// 明示指定は環境変数より優先し、指定がない場合だけ環境変数とTTYを見る。
    pub(super) fn resolve(
        setting: ColorSetting,
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
                color: color_for(setting, environment, terminals.stdout_is_tty),
                characters,
                width: terminals.width,
            },
            stderr: StreamPolicy {
                color: color_for(setting, environment, terminals.stderr_is_tty),
                characters,
                width: terminals.width,
            },
        }
    }
}

/// 1 streamの色可否。
fn color_for(setting: ColorSetting, environment: &Environment, is_tty: bool) -> bool {
    match setting {
        // 明示指定は利用者が選んだ以上、環境変数で覆さない。
        ColorSetting::Explicit(ColorMode::Always) => return true,
        ColorSetting::Explicit(ColorMode::Never) => return false,
        ColorSetting::Explicit(ColorMode::Auto) => return is_tty,
        ColorSetting::Default => {}
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
