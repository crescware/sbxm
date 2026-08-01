use super::*;

impl ExternalFailure {
    /// 表示用のstderr。原文のbyte列をlossyに変換するが、変換の有無は別途診断する。
    pub fn stderr_text(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }
}
