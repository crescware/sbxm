use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// 起点branchの記録の在り方。
///
/// keyが現れたことと、その値が`null`であることは別の事実である。`Option`を重ねると
/// どちらも`None`になり、「未確定として記録された」のか「記録が欠けている」のかを
/// 言い分けられない。3つの状態に名前を与えて、型がそのまま区別を担う。
#[derive(Debug, Default)]
pub enum RawStartRef {
    /// keyそのものが無い。記録が欠けている。
    #[default]
    Missing,
    /// keyはあり、値は`null`。起点branchが未確定であると記録されている。
    Unset,
    /// keyがあり、branch名が記録されている。
    Named(String),
}

impl<'de> Deserialize<'de> for RawStartRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // ここへ来た時点でkeyは現れている。`Missing`はfieldの`default`だけが作る。
        Ok(Option::<String>::deserialize(deserializer)?.map_or(Self::Unset, Self::Named))
    }
}

impl Serialize for RawStartRef {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Missing | Self::Unset => serializer.serialize_none(),
            Self::Named(value) => serializer.serialize_some(value),
        }
    }
}
