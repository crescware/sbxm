//! e2e testが、別processの状態が整うのを待つ唯一の手段。
//!
//! 上限は1つだけ置く。上限はhangを止めるためにあり、平常の実行が近づく値ではない。
//! 速さは確かめない。状態がどれだけ早く整ったかは、testが述べる契約ではない。
//!
//! 4本のtest binary(`host`、`prompt_pty`、`prompt_terminal`、`command_lifecycle`)がこれを
//! 取り込む。どれも別processの途中の状態を待つ。終わりまで待つだけの実行は`output()`で待ち、
//! これを使わない。

use std::time::{Duration, Instant};

use crate::outcome::{Checked, Unmet};

/// 待ち合わせの上限。
const WAIT_LIMIT: Duration = Duration::from_secs(60);

/// 状態を見直す間隔。
const RECHECK: Duration = Duration::from_millis(10);

/// `ready`が値を返すまで待つ。
///
/// `ready`は状態を1度だけ見て、整っていれば`Some`、まだなら`None`を返す。待っても整わない
/// と分かった場合は`Err`を返し、待つのをやめる。上限までに整わなければ、何を待っていたかを
/// 添えて失敗する。上限を過ぎたと判断する前に、必ずもう1度状態を見る。
#[track_caller]
pub fn wait_until<T>(what: &str, mut ready: impl FnMut() -> Checked<Option<T>>) -> Checked<T> {
    let deadline = Instant::now() + WAIT_LIMIT;
    loop {
        if let Some(value) = ready()? {
            return Ok(value);
        }
        if Instant::now() >= deadline {
            return Err(Unmet::new(format!(
                "{what} did not happen within {} seconds",
                WAIT_LIMIT.as_secs()
            )));
        }
        std::thread::sleep(RECHECK);
    }
}
