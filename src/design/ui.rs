use crate::diagnostics::{Error, Msg};
use crate::i18n::{Catalog, Locale};

use crate::design::renderer::Renderer;

use super::{Document, ExternalOutput, GuidanceItem, OutputPolicy, ProgressSink, Warning};

/// 1実行ぶんの利用者interface。
///
/// locale、streamごとの描画条件、blockの間隔を1箇所へ集める。command printerがcatalogや
/// terminalを持たないのは、表示構造だけをunit testできるようにするためである。
pub struct Ui<'a> {
    catalog: Catalog,
    stdout: Renderer<'a>,
    stderr: Renderer<'a>,
    /// 外部toolの出力が続いている最中か。
    external: bool,
}

impl Ui<'static> {
    /// 実際の端末へ書くUI。
    pub fn terminal(locale: Locale, policy: OutputPolicy) -> Ui<'static> {
        Ui {
            catalog: Catalog::new(locale),
            stdout: Renderer::new(std::io::stdout(), policy.stdout),
            stderr: Renderer::new(std::io::stderr(), policy.stderr),
            external: false,
        }
    }
}

impl Ui<'_> {
    pub fn locale(&self) -> Locale {
        self.catalog.locale()
    }

    /// 表示言語を差し替える。
    ///
    /// 表示localeはconfigを読む前にbest-effortで決める。configのvalidationを通ったあと、
    /// または`init`がconfigを作ったあとに、宣言された言語で残りを描く。
    pub fn set_locale(&mut self, locale: Locale) {
        if self.catalog.locale() != locale {
            self.catalog = Catalog::new(locale);
        }
    }

    /// 既に組み立てられたhelpを書く。
    pub fn help(&mut self, text: &str) {
        self.stdout(&Document::new().verbatim(text));
    }

    /// promptがrendererを経由せずstderrへ書いたことを知らせる。
    ///
    /// 次のblockが空行から始まるようにするためだけに使う。
    pub fn note_prompt_output(&mut self) {
        self.stderr.note_external_write();
    }

    /// 正常結果を書く。
    pub fn stdout(&mut self, document: &Document) {
        self.stdout.write(&self.catalog, document);
    }

    /// 実行中のfeedbackと異常を書く。
    pub fn stderr(&mut self, document: &Document) {
        self.stderr.write(&self.catalog, document);
    }

    /// これから始める工程を示し、直ちにflushする。
    ///
    /// 外部toolが何も出さない工程では、待ち時間が「止まっているのか動いているのか
    /// 分からない時間」になる。工程を始める前に1行だけ知らせる。
    pub fn progress(&mut self, message: Msg) {
        self.stderr(&Document::new().progress(message));
    }

    /// 結果を隠さずに注意を伝える。
    pub fn warning(&mut self, warning: &Warning) {
        self.stderr(&warning_document(warning));
    }

    /// 失敗を報告する。中断は何も表示しない。
    pub fn error(&mut self, error: &Error) {
        let mut document = Document::new();
        for diagnostic in error.diagnostics() {
            document = document.diagnostic(diagnostic.clone());
        }
        if !document.is_empty() {
            self.stderr(&document);
        }
    }
}

impl ProgressSink for Ui<'_> {
    fn step(&mut self, message: Msg) {
        self.progress(message);
    }
}

impl ExternalOutput for Ui<'_> {
    fn relay(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        self.open_boundary();
        self.stderr.write_external(bytes);
    }

    fn hand_over(&mut self) {
        self.open_boundary();
    }

    fn finished(&mut self) {
        // 何も出さなかった外部toolのために境界を作らない。
        if !self.external {
            return;
        }
        self.external = false;
        self.stderr.end_external();
        self.stdout.note_external_write();
    }
}

impl Ui<'_> {
    /// sbxmの出力と外部toolの出力の境界に、空行を1つだけ置く。
    ///
    /// 端末では2つのstreamが同じ場所へ出る。両方が空行を負っていても、利用者が見る
    /// 境界は1つである。
    fn open_boundary(&mut self) {
        if self.external {
            return;
        }
        self.external = true;
        let stdout_owed = self.stdout.take_owed_blank();
        let stderr_owed = self.stderr.take_owed_blank();
        if stdout_owed || stderr_owed {
            self.stderr.write_blank();
        }
    }
}

/// warningを、説明・補足・commandのblockへ分ける。
fn warning_document(warning: &Warning) -> Document {
    let mut document = Document::new().warning(warning.description.clone(), warning.facts.clone());
    if !warning.guidance.is_empty() {
        document = document.guidance(
            None,
            warning
                .guidance
                .iter()
                .cloned()
                .map(GuidanceItem::Plain)
                .collect(),
        );
    }
    for command in &warning.commands {
        document = document.command(command.clone());
    }
    document
}

#[cfg(test)]
#[path = "ui_test.rs"]
mod ui_test;
