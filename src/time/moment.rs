use std::time::Duration;

/// 単調な時計の読み。その時計の起点からの経過で表す。
///
/// `Instant`はOSの時計からしか作れず、testの時計は読みを作れない。起点からの経過なら、
/// OSの時計もtestの時計も同じ形で返せる。別の時計の読みどうしは比べない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Moment(Duration);

impl Moment {
    /// 起点から`elapsed`経った読み。
    pub fn since_origin(elapsed: Duration) -> Moment {
        Moment(elapsed)
    }

    /// この読みの`span`後。表せる最後の読みを越えない。
    pub fn after(self, span: Duration) -> Moment {
        Moment(self.0.saturating_add(span))
    }

    /// `earlier`からこの読みまでの経過。`earlier`が後なら0とする。
    ///
    /// 同じ時計の2つの読みの差だけを取る。別の時計の読みを渡しても型では防げないため、
    /// 呼び出し側が同じ時計から読むことを守る。
    pub fn since(self, earlier: Moment) -> Duration {
        self.0.saturating_sub(earlier.0)
    }
}

#[cfg(test)]
#[path = "moment_test.rs"]
mod moment_test;
