use crate::i18n::Locale;
use crate::metadata::GitIdentity;

use super::FileDeclaration;

/// validation済みのglobal config。
///
/// 欠落したoptional fieldは未保存として扱う。表示言語が未保存であることと、
/// 特定の言語が保存されていることを同じ値にしない。Git identityも同じで、未保存で
/// あることは、利用者がまだ案件の名義を選んでいないことを意味する。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GlobalConfig {
    pub language: Option<Locale>,
    /// 利用者が選んだ、新規登録の既定となる名義。
    pub git_identity: Option<GitIdentity>,
    pub files: Vec<FileDeclaration>,
}
