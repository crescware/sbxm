use crate::app::Interactivity;

pub fn non_tty() -> Interactivity {
    Interactivity::from_available(false)
}
