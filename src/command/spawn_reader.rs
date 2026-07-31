use std::io::Read;

/// 子processの1 streamを読み切る。
pub(super) fn spawn_reader<R: Read + Send + 'static>(
    mut pipe: R,
) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut collected = Vec::new();
        let mut buffer = [0_u8; 8 * 1024];
        loop {
            match pipe.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => collected.extend_from_slice(&buffer[..read]),
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
        collected
    })
}
