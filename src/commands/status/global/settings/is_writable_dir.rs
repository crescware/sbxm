use std::path::Path;

pub fn is_writable_dir(path: &Path) -> bool {
    let probe = path.join(".sbxm-write-probe");
    match std::fs::create_dir(&probe) {
        Ok(()) => {
            let _ = std::fs::remove_dir(&probe);
            true
        }
        Err(error) => error.kind() == std::io::ErrorKind::AlreadyExists,
    }
}
