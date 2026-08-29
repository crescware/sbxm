//! stdio、TTY/environment観測、consoleの実terminalをdesignのportへ接続するadapter。

mod create_prompt_ui;
mod create_ui;
mod detect_rendering_policy;
mod prompt_capability;
mod real_terminal;

pub(crate) use create_prompt_ui::create_prompt_ui;
pub(crate) use create_ui::create_ui;
pub(crate) use detect_rendering_policy::detect_rendering_policy;
pub(crate) use prompt_capability::PromptCapability;

#[cfg(test)]
#[path = "prompt_capability_test.rs"]
mod prompt_capability_test;
