use std::io::Result as IoResult;
use std::os::fd::AsFd;

/// 読み取り端を、待たずに読めるようにする。
pub(super) fn set_nonblocking<Fd: AsFd>(fd: &Fd) -> IoResult<()> {
    let flags = rustix::fs::fcntl_getfl(fd)?;
    let nonblocking = flags | rustix::fs::OFlags::NONBLOCK;
    Ok(rustix::fs::fcntl_setfl(fd, nonblocking)?)
}
