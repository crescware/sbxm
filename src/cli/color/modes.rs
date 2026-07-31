use crate::design::ColorMode;

pub(super) fn modes() -> Vec<&'static str> {
    ColorMode::ALL.iter().map(|mode| mode.as_str()).collect()
}
