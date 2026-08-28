use super::Key;

/// 打鍵の供給元。
///
/// 待つことだけを持ち、画面のことを知らない。打鍵をどう解釈するかは[`super::action_for`]
/// と[`super::PromptUi`]が決めるため、実装は端末から1つ読むだけでよい。
pub trait Keys {
    fn read_key(&mut self) -> std::io::Result<Key>;
}
