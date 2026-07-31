/// project scopeの状態値。翻訳しない安定したenum。
///
/// `unknown`は使用しない。観測していない項目は、観測できなかった理由を持つ値で示す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    Ready,
    Missing,
    Mismatch,
    Changed,
    Running,
    Stopped,
    NotCreated,
    Clean,
    Dirty,
    Attached,
    Detached,
    NotExposed,
    Exposed,
    NotApplicable,
    NotObservedStopped,
}

impl Value {
    pub fn as_str(self) -> &'static str {
        match self {
            Value::Ready => "ready",
            Value::Missing => "missing",
            Value::Mismatch => "mismatch",
            Value::Changed => "changed",
            Value::Running => "running",
            Value::Stopped => "stopped",
            Value::NotCreated => "not-created",
            Value::Clean => "clean",
            Value::Dirty => "dirty",
            Value::Attached => "attached",
            Value::Detached => "detached",
            Value::NotExposed => "not-exposed",
            Value::Exposed => "exposed",
            Value::NotApplicable => "not-applicable",
            Value::NotObservedStopped => "not-observed-stopped",
        }
    }

    pub fn legend_id(self) -> &'static str {
        match self {
            Value::Ready => "legend-ready",
            Value::Missing => "legend-missing",
            Value::Mismatch => "legend-mismatch",
            Value::Changed => "legend-changed",
            Value::Running => "legend-sandbox-running",
            Value::Stopped => "legend-sandbox-stopped",
            Value::NotCreated => "legend-not-created",
            Value::Clean => "legend-clean",
            Value::Dirty => "legend-dirty",
            Value::Attached => "legend-attached",
            Value::Detached => "legend-detached",
            Value::NotExposed => "legend-not-exposed",
            Value::Exposed => "legend-exposed",
            Value::NotApplicable => "legend-not-applicable",
            Value::NotObservedStopped => "legend-not-observed-stopped",
        }
    }
}
