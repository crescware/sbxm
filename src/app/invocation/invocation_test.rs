use crate::config::{ConfigLocation, ConfigObservation};
use crate::design::{ColorMode, RenderingPolicy};
use crate::diagnostics::ErrorId;
use crate::i18n::Locale;
use crate::testing::cli::{argv, non_tty};

use super::{CommandLine, Interactivity, Invocation};

impl Invocation {
    /// process environmentを読まずに、この起動の観測値を明示して組み立てる。
    ///
    /// productionの`new`が一度だけ観測する値をそのまま受け取る。localeの解決規則は
    /// `new`と同じものを通す。
    pub(crate) fn for_test(
        command_line: CommandLine,
        config: ConfigObservation,
        shell: Option<Locale>,
        policy: RenderingPolicy,
        interactivity: Interactivity,
    ) -> Self {
        let locale =
            crate::i18n::resolve_locale(command_line.locale_override(), config.language(), shell);
        Self {
            command_line,
            config,
            locale,
            policy,
            interactivity,
        }
    }
}

fn observation(configured: Option<Locale>) -> ConfigObservation {
    ConfigObservation::new(
        ConfigLocation::from_home(std::path::PathBuf::from("/test-home")),
        configured,
    )
}

#[test]
fn an_invocation_resolves_one_locale_from_its_inputs() {
    let invocation = Invocation::for_test(
        CommandLine::new(argv(&["--lang", "ja", "ls"])),
        observation(Some(Locale::En)),
        Some(Locale::En),
        RenderingPolicy::detect(ColorMode::Never),
        non_tty(),
    );

    assert_eq!(invocation.locale(), Locale::Ja);
}

#[test]
fn an_invalid_language_is_reported_before_full_parsing() {
    let invocation = Invocation::for_test(
        CommandLine::new(argv(&["--lang=zz", "ls"])),
        observation(None),
        Some(Locale::En),
        RenderingPolicy::detect(ColorMode::Never),
        non_tty(),
    );

    let result = invocation.parse();
    assert!(result.is_err(), "an invalid language must be refused");
    let error = result.err().unwrap_or(crate::diagnostics::Error::Canceled);

    assert_eq!(error.first_id(), Some(ErrorId::InvalidLang));
}
