use std::fmt::Debug;

use super::{Checked, Refused, Required, Unmet};

impl<T, E: Debug> Required<T> for std::result::Result<T, E> {
    #[track_caller]
    fn required(self) -> Checked<T> {
        match self {
            Ok(value) => Ok(value),
            Err(error) => Err(Unmet::new(format!("{error:?}"))),
        }
    }

    #[track_caller]
    fn required_because(self, reason: &str) -> Checked<T> {
        match self {
            Ok(value) => Ok(value),
            Err(error) => Err(Unmet::new(format!("{reason}: {error:?}"))),
        }
    }
}

impl<T: Debug, E> Refused<T, E> for std::result::Result<T, E> {
    #[track_caller]
    fn refused(self) -> Checked<E> {
        match self {
            Err(error) => Ok(error),
            Ok(value) => Err(Unmet::new(format!(
                "a refusal was required, but {value:?} was produced"
            ))),
        }
    }

    #[track_caller]
    fn refused_because(self, reason: &str) -> Checked<E> {
        match self {
            Err(error) => Ok(error),
            Ok(value) => Err(Unmet::new(format!("{reason}, but {value:?} was produced"))),
        }
    }
}
