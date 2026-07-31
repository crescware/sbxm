use crate::cli::Interactivity;

pub fn non_tty() -> Interactivity {
    Interactivity {
        stdin_is_tty: false,
        stderr_is_tty: false,
    }
}
