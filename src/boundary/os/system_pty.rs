use std::ffi::CString;
use std::fs::File;
use std::io;
use std::process::Stdio;

use rustix::fs::{Mode, OFlags};
use rustix::io::FdFlags;
use rustix::pty::OpenptFlags;
use rustix::termios::{OptionalActions, Termios, Winsize};

use crate::boundary::host::Pty;

use super::RealHost;

impl Pty for RealHost {
    type Controller = File;
    type Terminal = File;
    type Name = CString;
    type Settings = Termios;

    fn open_controller(&self) -> io::Result<File> {
        Ok(File::from(rustix::pty::openpt(
            OpenptFlags::RDWR | OpenptFlags::NOCTTY,
        )?))
    }

    fn close_on_exec(&self, controller: &File) -> io::Result<()> {
        Ok(rustix::io::fcntl_setfd(controller, FdFlags::CLOEXEC)?)
    }

    fn grant(&self, controller: &File) -> io::Result<()> {
        Ok(rustix::pty::grantpt(controller)?)
    }

    fn unlock_terminal(&self, controller: &File) -> io::Result<()> {
        Ok(rustix::pty::unlockpt(controller)?)
    }

    fn terminal_name(&self, controller: &File) -> io::Result<CString> {
        Ok(rustix::pty::ptsname(controller, Vec::new())?)
    }

    fn open_terminal(&self, name: &CString) -> io::Result<File> {
        Ok(File::from(rustix::fs::open(
            name,
            OFlags::RDWR | OFlags::NOCTTY | OFlags::CLOEXEC,
            Mode::empty(),
        )?))
    }

    fn settings(&self, terminal: &File) -> io::Result<Termios> {
        Ok(rustix::termios::tcgetattr(terminal)?)
    }

    fn make_raw(&self, settings: &mut Termios) {
        settings.make_raw();
    }

    fn apply_settings(&self, terminal: &File, settings: &Termios) -> io::Result<()> {
        Ok(rustix::termios::tcsetattr(
            terminal,
            OptionalActions::Now,
            settings,
        )?)
    }

    fn set_size(&self, controller: &File, rows: u16, columns: u16) -> io::Result<()> {
        Ok(rustix::termios::tcsetwinsize(
            controller,
            Winsize {
                ws_row: rows,
                ws_col: columns,
                ws_xpixel: 0,
                ws_ypixel: 0,
            },
        )?)
    }

    fn controller_nonblocking(&self, controller: &File) -> io::Result<()> {
        Ok(rustix::fs::fcntl_setfl(
            controller,
            rustix::fs::fcntl_getfl(controller)? | OFlags::NONBLOCK,
        )?)
    }

    fn terminal_stdio(&self, terminal: &File) -> io::Result<Stdio> {
        Ok(Stdio::from(terminal.try_clone()?))
    }
}

#[cfg(test)]
#[path = "system_pty_test.rs"]
mod system_pty_test;
