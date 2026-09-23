use crate::design::Document;
use crate::msg;

/// 足した宣言をあとで全案件へ配置する手順。
pub fn apply_hint() -> Document {
    Document::new()
        .note(msg!("files-apply-hint"))
        .try_command("sbxm apply --files --all")
}
