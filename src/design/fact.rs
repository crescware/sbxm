use crate::diagnostics::Msg;

use crate::design::text::Inline;

/// 診断が示す事実1件。項目名と、翻訳しない値。
///
/// 説明文から変数を追い出すための単位である。`sbx ls`のような値を地の文へ混ぜると、
/// 読み手はまず文のどこが変数かを探すことになる。項目名を左へ置いて行を分ければ、
/// 色を足さずに境界が決まる。翻訳済み文字列を検索して部分装飾する必要もなくなる。
///
/// 値が1行に収まるかどうかで形が変わる。収まる値は項目名と同じ行へ置き、収まらない
/// 値は次の行から字下げして原文のまま並べる。改行を1行へ潰すと外部が示した位置が
/// 失われるため、この分岐はrendererではなく構築時に決める。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fact {
    /// 1行に収まる事実。項目名と同じ行に値を置く。
    OneLine { label: Msg, value: Inline },
    /// 複数行にわたる事実。項目名の次の行から字下げして並べる。
    ///
    /// 複数行の値は外部が書いた原文であり、外部outputを着色しないという規則に従う。
    /// `Inline`が持っていた装飾はここでは引き継がない。
    ManyLines { label: Msg, lines: Vec<String> },
    /// 翻訳する値。
    ///
    /// sbxm自身が観測したことを述べる値は、原文が英語ではなくmessageである。外部が
    /// 書いた原文と同じ経路へ流すと、翻訳された文のなかに英語が残る。
    Translated { label: Msg, value: Msg },
}

impl Fact {
    /// `Command:`。失敗した外部commandの起動。
    pub fn command(invocation: &str) -> Fact {
        Fact::new(
            Msg::new("diagnostic-command-label"),
            Inline::important(invocation),
        )
    }

    /// `Working directory:`。外部commandを起動したdirectory。
    pub fn directory(path: &str) -> Fact {
        Fact::new(Msg::new("diagnostic-directory-label"), Inline::path(path))
    }

    /// `Path:`。診断の対象になったhost上のpath。
    pub fn path(path: &str) -> Fact {
        Fact::new(Msg::new("diagnostic-path-label"), Inline::path(path))
    }

    /// `Field:`。診断の対象になった宣言fileの項目名。
    pub fn field(name: &str) -> Fact {
        Fact::new(Msg::new("diagnostic-field-label"), Inline::important(name))
    }

    /// `Entry:`。一覧のなかで問題になった宣言の位置。
    pub fn entry(index: usize) -> Fact {
        Fact::new(
            Msg::new("diagnostic-entry-label"),
            Inline::important(index.to_string()),
        )
    }

    /// `Source:`。宣言fileの取得元。
    pub fn source(path: &str) -> Fact {
        Fact::new(Msg::new("diagnostic-source-label"), Inline::path(path))
    }

    /// `Destination:`。宣言fileのSandbox内での配置先。
    pub fn destination(path: &str) -> Fact {
        Fact::new(Msg::new("diagnostic-destination-label"), Inline::path(path))
    }

    /// `Value:`。受け付けられなかった値そのもの。
    pub fn value(value: &str) -> Fact {
        Fact::new(Msg::new("diagnostic-value-label"), Inline::important(value))
    }

    /// `Image:`。診断の対象になったcontainer image。
    pub fn image(name: &str) -> Fact {
        Fact::new(Msg::new("diagnostic-image-label"), Inline::important(name))
    }

    /// `Template:`。診断の対象になったTemplate。
    pub fn template(name: &str) -> Fact {
        Fact::new(
            Msg::new("diagnostic-template-label"),
            Inline::important(name),
        )
    }

    /// `Sandbox:`。診断の対象になったSandbox。
    pub fn sandbox(name: &str) -> Fact {
        Fact::new(
            Msg::new("diagnostic-sandbox-label"),
            Inline::important(name),
        )
    }

    /// `Document:`。組み立てに失敗した記録の種別。
    pub fn document(name: &str) -> Fact {
        Fact::new(
            Msg::new("diagnostic-document-label"),
            Inline::important(name),
        )
    }

    /// `Cause:`。外部が書いた原文をそのまま示す。
    pub fn cause(detail: &str) -> Fact {
        Fact::new(Msg::new("diagnostic-cause-label"), Inline::text(detail))
    }

    /// `Cause:`。sbxm自身が観測したことを、翻訳する文で示す。
    pub fn reason(detail: Msg) -> Fact {
        Fact::Translated {
            label: Msg::new("diagnostic-cause-label"),
            value: detail,
        }
    }

    /// 事実を1件作る。値の形が表示を決める。
    pub fn new(label: Msg, value: Inline) -> Fact {
        // 末尾の改行は値の一部ではない。残すと1行の値まで複数行として扱ってしまう。
        let text = value.as_str().trim_end();
        let mut lines = text.lines();
        let first = lines.next().unwrap_or_default().to_string();
        if lines.next().is_some() {
            let lines = text.lines().map(str::to_string).collect();
            return Fact::ManyLines { label, lines };
        }
        Fact::OneLine {
            label,
            value: value.with_text(&first),
        }
    }

    /// 項目名。
    pub fn label(&self) -> &Msg {
        match self {
            Fact::OneLine { label, .. }
            | Fact::ManyLines { label, .. }
            | Fact::Translated { label, .. } => label,
        }
    }
}

#[cfg(test)]
#[path = "fact_test.rs"]
mod fact_test;
