use std::io;
use std::os::unix::process::CommandExt;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus};

use crate::boundary::host::Processes;

use super::RealHost;

impl Processes for RealHost {
    type Child = Child;
    type Stdin = ChildStdin;
    type Stdout = ChildStdout;
    type Stderr = ChildStderr;

    fn own_group(&self, command: &mut Command) {
        command.process_group(0);
    }

    fn start(&self, command: &mut Command) -> io::Result<Child> {
        command.spawn()
    }

    fn check_exit(&self, child: &mut Child) -> io::Result<Option<ExitStatus>> {
        child.try_wait()
    }

    fn wait_exit(&self, child: &mut Child) -> io::Result<ExitStatus> {
        child.wait()
    }

    fn end_child(&self, child: &mut Child) -> io::Result<()> {
        child.kill()
    }

    fn take_stdin(&self, child: &mut Child) -> Option<ChildStdin> {
        child.stdin.take()
    }

    fn take_stdout(&self, child: &mut Child) -> Option<ChildStdout> {
        child.stdout.take()
    }

    fn take_stderr(&self, child: &mut Child) -> Option<ChildStderr> {
        child.stderr.take()
    }
}

#[cfg(test)]
#[path = "system_process_test.rs"]
mod system_process_test;
