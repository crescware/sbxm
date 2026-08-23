use std::io::{Read, Result as IoResult};

/// 読めるだけ読み、届いたbyteをそのまま渡す。EOFに達したpipeは閉じる。
pub(super) fn drain_pipe<P: Read>(
    pipe: &mut Option<P>,
    receive: &mut dyn FnMut(&[u8]),
) -> IoResult<()> {
    let Some(reader) = pipe.as_mut() else {
        return Ok(());
    };
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => {
                *pipe = None;
                return Ok(());
            }
            Ok(read) => receive(&buffer[..read]),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) => return Err(error),
        }
    }
}
