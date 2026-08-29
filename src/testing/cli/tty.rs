use crate::app::Interactivity;

pub fn tty() -> Interactivity {
    Interactivity::from_available(true)
}
