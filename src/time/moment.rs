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
}
