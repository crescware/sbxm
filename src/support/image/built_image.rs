use crate::design::Warning;

/// 使用できる状態のimage。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltImage {
    pub name: String,
    /// `docker image inspect`が示した`Id`。
    ///
    /// image storeとattestationの有無で、config、manifest、image indexの
    /// どれを指すかが変わる。archiveとの対応の判定には使わない。
    pub id: String,
    /// このimageが宣言しているlabel。archiveとの対応はこれで判定する。
    pub labels: Vec<(String, String)>,
    /// この実行でbuildしたか。
    pub built: bool,
    /// 成果物としては成立したが、利用者へ伝える必要がある事実。
    pub warnings: Vec<Warning>,
}
