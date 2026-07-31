use crate::cli::Interactivity;

pub fn tty() -> Interactivity {
    Interactivity {
        stdin_is_tty: true,
        stderr_is_tty: true,
    }
}
