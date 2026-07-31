use super::{Checked, Required, Unmet};

impl<T> Required<T> for Option<T> {
    #[track_caller]
    fn required(self) -> Checked<T> {
        match self {
            Some(value) => Ok(value),
            None => Err(Unmet::new("a value was required, but none was present")),
        }
    }

    #[track_caller]
    fn required_because(self, reason: &str) -> Checked<T> {
        match self {
            Some(value) => Ok(value),
            None => Err(Unmet::new(reason)),
        }
    }
}
