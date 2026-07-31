/// 出力片が果たす役割。
///
/// `Role`は「これは何か」であり、「何色か」ではない。theme optionを将来足す場合も
/// この一覧は変えず、写像の中身だけを差し替える。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// section、guidance、diagnosticの見出し。
    Heading,
    /// tableの列名。
    TableHeader,
    /// 進行中を示すmarker。
    ProgressMarker,
    /// 成功を示すmarker。
    SuccessMarker,
    /// 注意を示すmarkerとlabel。
    WarningMarker,
    /// 失敗を示すmarkerとlabel。
    ErrorMarker,
    /// 利用者がそのままshellへ入力する一行。
    Command,
    /// 照合の基準になる短い値。
    Important,
    /// 操作説明、凡例、metadataのような補助情報。
    Muted,
    /// promptのkeyboard focusがある行。
    PromptCurrent,
    /// promptで選択済みであることを示すcheckbox。
    PromptChecked,
}
