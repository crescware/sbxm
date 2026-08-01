use std::io::Read;

/// 子processの1 streamを読み切る。
///
/// 読めなくなった場合は、そこまでの内容ではなく原因を返す。途中までの出力を最後まで
/// 読めた出力と同じ形で返すと、呼び出し側は短い出力と欠けた出力を見分けられない。
pub(super) fn spawn_reader<R: Read + Send + 'static>(
    mut pipe: R,
) -> std::thread::JoinHandle<std::io::Result<Vec<u8>>> {
    std::thread::spawn(move || {
        let mut collected = Vec::new();
        let mut buffer = [0_u8; 8 * 1024];
        loop {
            match pipe.read(&mut buffer) {
                Ok(0) => return Ok(collected),
                Ok(read) => collected.extend_from_slice(&buffer[..read]),
                // signalで中断された読み出しは失敗ではなく、やり直しである。
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
    })
}
