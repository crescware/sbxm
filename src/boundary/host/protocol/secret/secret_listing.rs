use super::{CustomSecret, ServiceSecret};

/// `sbx secret ls --json`の全体。
///
/// service secretとcustom secretは別の表として返る。sbxmが案内する登録はservice
/// secretだが、以前の版が案内したcustom secretの後片付けにも同じ一覧を使う。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SecretListing {
    pub services: Vec<ServiceSecret>,
    pub customs: Vec<CustomSecret>,
}
