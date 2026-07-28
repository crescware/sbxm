//! 長い工程の開始を知らせる。
//!
//! 進捗の実況はしない。工程を始める前に、何を始めるのかと、どのくらいかかり得るのかを
//! 1行で示す。外部toolが何も出さない工程では、待ち時間が「止まっているのか動いている
//! のか分からない時間」になり、利用者が中断を選ぶ理由になる。
//!
//! 表示先はstderrとする。stdoutは結果の表であり、進捗を混ぜない。

use std::io::Write;
use std::sync::OnceLock;

use crate::error::Msg;
use crate::i18n::{Catalog, Locale};

/// 表示言語。決まるまでは何も出さない。
static LOCALE: OnceLock<Locale> = OnceLock::new();

/// 表示言語を1度だけ設定する。
///
/// 設定しない実行では`step`が何もしない。testはこの経路を通らないため、出力を
/// 持たない。
pub fn install(locale: Locale) {
    let _ = LOCALE.set(locale);
}

/// これから始める工程を示す。
pub fn step(message: &Msg) {
    let Some(locale) = LOCALE.get() else {
        return;
    };
    let Ok(text) = Catalog::new(*locale).format(message) else {
        return;
    };
    let mut stderr = std::io::stderr();
    let _ = writeln!(stderr, "{text}");
    let _ = stderr.flush();
}
