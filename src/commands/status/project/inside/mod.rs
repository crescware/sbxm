//! Sandboxと、その中で見える状態の診断。

mod check_inside;
mod check_sandbox;
mod check_secret;
mod check_ssh_agent;

pub(super) use check_inside::check_inside;
pub(super) use check_sandbox::check_sandbox;
pub(super) use check_secret::check_secret;
pub(super) use check_ssh_agent::check_ssh_agent;

#[cfg(test)]
#[path = "inside_test.rs"]
mod inside_test;
