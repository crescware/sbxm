use super::ColorMode;

/// color modeが明示されたかどうかを含む、1回の起動へのcolor要求。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorSetting {
    /// 環境変数、端末種別、streamごとのTTYからpolicyを決める。
    #[default]
    Default,
    /// 利用者が明示したmode。環境変数より優先する。
    Explicit(ColorMode),
}
