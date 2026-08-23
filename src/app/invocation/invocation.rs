use crate::commands::Command;
use crate::config::{ConfigLocation, ConfigObservation};
use crate::design::RenderingPolicy;
use crate::diagnostics::Result;
use crate::i18n::{self, Catalog, Locale};

use super::{CommandLine, Interactivity};

/// 1回のapplication実行。
pub(crate) struct Invocation {
    pub(super) command_line: CommandLine,
    pub(super) config: ConfigObservation,
    pub(super) locale: Locale,
    pub(super) policy: RenderingPolicy,
    pub(super) interactivity: Interactivity,
}

impl Invocation {
    /// productionの起動材料を1回だけ観測し、この実行の値を確定する。
    pub(crate) fn new(command_line: CommandLine, config: ConfigObservation) -> Self {
        let locale = i18n::resolve_locale(
            command_line.locale_override(),
            config.language(),
            i18n::shell_locale(),
        );
        let policy = RenderingPolicy::detect(command_line.color_mode());
        let interactivity = Interactivity::detect();
        Self {
            command_line,
            config,
            locale,
            policy,
            interactivity,
        }
    }

    pub(crate) fn parse(&self) -> Result<Command> {
        if let Some(value) = self.command_line.invalid_locale_override() {
            return Err(CommandLine::invalid_locale_error(value));
        }
        let catalog = Catalog::new(self.locale);
        super::parse::parse(self.command_line.argv(), &catalog, self.interactivity)
    }

    pub(crate) fn location(&self) -> &ConfigLocation {
        self.config.location()
    }

    pub(crate) fn locale(&self) -> Locale {
        self.locale
    }

    pub(crate) fn rendering_policy(&self) -> RenderingPolicy {
        self.policy
    }

    pub(crate) fn can_prompt(&self) -> bool {
        self.interactivity.can_prompt()
    }
}

#[cfg(test)]
#[path = "invocation_test.rs"]
mod invocation_test;
