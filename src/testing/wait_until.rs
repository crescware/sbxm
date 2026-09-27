use std::time::Duration;

use crate::testing::outcome::{Checked, Unmet};
use crate::time::Clock;

/// 待ち合わせの上限。hangを止めるためにあり、平常の実行が近づく値ではない。
///
/// `tests/poll_eintr.rs`が同じ定義を持ち、`tests/architecture.rs`が一致を確かめる。
const WAIT_LIMIT: Duration = Duration::from_secs(60);

/// 状態を見直す間隔。
const RECHECK: Duration = Duration::from_millis(10);

/// `ready`が値を返すまで、`clock`で待つ。
///
/// 契約testが、別のprocessやOSの状態が整うのを待つ唯一の手段である。確かめる対象が`poll`の
/// 待ちそのものである`tests/poll_eintr.rs`だけは、その`poll`の期限で待つ。
///
/// `ready`は状態を1度だけ見て、整っていれば`Some`、まだなら`None`を返す。待っても整わないと
/// 分かれば`Err`を返し、待つのをやめる。上限までに整わなければ、何を待っていたかを添えて
/// 失敗する。どれだけ早く整ったかは確かめない。
///
/// 時計は受け取る。OS層の時計をここで名指しすれば、OS層の外のtest支援codeが実時間を持つ。
#[track_caller]
pub fn wait_until<T>(
    clock: &dyn Clock,
    what: &str,
    mut ready: impl FnMut() -> Checked<Option<T>>,
) -> Checked<T> {
    let deadline = clock.now().after(WAIT_LIMIT);
    loop {
        if let Some(value) = ready()? {
            return Ok(value);
        }
        if clock.now() >= deadline {
            return Err(Unmet::new(format!(
                "{what} did not happen within {} seconds",
                WAIT_LIMIT.as_secs()
            )));
        }
        clock.sleep(RECHECK);
    }
}

#[cfg(test)]
#[path = "wait_until_test.rs"]
mod wait_until_test;
