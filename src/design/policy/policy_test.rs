use super::*;

fn environment() -> Environment {
    Environment::default()
}

fn terminals(stdout: bool, stderr: bool) -> Terminals {
    Terminals {
        stdout_is_tty: stdout,
        stderr_is_tty: stderr,
        width: None,
    }
}

#[test]
fn auto_colors_only_the_streams_that_are_terminals() {
    let policy = OutputPolicy::resolve(ColorMode::Auto, &environment(), &terminals(false, true));
    assert!(!policy.stdout.color, "a piped stdout stays plain");
    assert!(policy.stderr.color, "a terminal stderr keeps its color");
}

#[test]
fn the_two_streams_are_judged_independently() {
    let both = OutputPolicy::resolve(ColorMode::Auto, &environment(), &terminals(true, true));
    assert!(both.stdout.color && both.stderr.color);

    let neither = OutputPolicy::resolve(ColorMode::Auto, &environment(), &terminals(false, false));
    assert!(!neither.stdout.color && !neither.stderr.color);
}

#[test]
fn an_explicit_mode_wins_over_every_environment_variable() {
    let hostile = Environment {
        no_color: true,
        clicolor_force: Some("1".to_string()),
        term: Some("dumb".to_string()),
    };

    let always = OutputPolicy::resolve(ColorMode::Always, &hostile, &terminals(false, false));
    assert!(always.stdout.color, "--color=always reaches a redirect");
    assert!(always.stderr.color);

    let never = OutputPolicy::resolve(ColorMode::Never, &hostile, &terminals(true, true));
    assert!(!never.stdout.color, "--color=never silences a terminal");
    assert!(!never.stderr.color);
}

#[test]
fn no_color_wins_over_clicolor_force_and_the_terminal() {
    let environment = Environment {
        no_color: true,
        clicolor_force: Some("1".to_string()),
        ..environment()
    };
    let policy = OutputPolicy::resolve(ColorMode::Auto, &environment, &terminals(true, true));
    assert!(!policy.stdout.color);
    assert!(!policy.stderr.color);
}

#[test]
fn an_empty_no_color_is_still_an_opt_out() {
    // 値ではなく存在を尊重する。`NO_COLOR=`も無効化として扱う。
    let environment = Environment {
        no_color: true,
        ..environment()
    };
    let policy = OutputPolicy::resolve(ColorMode::Auto, &environment, &terminals(true, true));
    assert!(!policy.stdout.color);
}

#[test]
fn clicolor_force_colors_a_redirect_unless_it_is_zero() {
    let forced = Environment {
        clicolor_force: Some("1".to_string()),
        ..environment()
    };
    let policy = OutputPolicy::resolve(ColorMode::Auto, &forced, &terminals(false, false));
    assert!(policy.stdout.color, "a forced run reaches a redirect");

    let disabled = Environment {
        clicolor_force: Some("0".to_string()),
        ..environment()
    };
    let policy = OutputPolicy::resolve(ColorMode::Auto, &disabled, &terminals(false, false));
    assert!(!policy.stdout.color, "zero does not force anything");
}

#[test]
fn clicolor_force_wins_over_a_dumb_terminal() {
    let environment = Environment {
        clicolor_force: Some("1".to_string()),
        term: Some("dumb".to_string()),
        ..environment()
    };
    let policy = OutputPolicy::resolve(ColorMode::Auto, &environment, &terminals(true, true));
    assert!(policy.stdout.color);
}

#[test]
fn a_dumb_terminal_loses_both_color_and_unicode_glyphs() {
    let environment = Environment {
        term: Some("dumb".to_string()),
        ..environment()
    };
    let policy = OutputPolicy::resolve(ColorMode::Auto, &environment, &terminals(true, true));
    assert!(!policy.stdout.color);
    assert_eq!(policy.stdout.characters, CharacterSet::Ascii);
    assert_eq!(policy.stderr.characters, CharacterSet::Ascii);
}

#[test]
fn a_normal_terminal_keeps_unicode_glyphs_even_without_color() {
    // 罫線とmarkerは意味の補助であり、色の有無と結び付けない。
    let policy = OutputPolicy::resolve(ColorMode::Never, &environment(), &terminals(true, true));
    assert_eq!(policy.stdout.characters, CharacterSet::Unicode);
}

#[test]
fn the_plain_policy_never_colors_anything() {
    let policy = OutputPolicy::plain();
    assert!(!policy.stdout.color);
    assert!(!policy.stderr.color);
}

#[test]
fn every_color_mode_round_trips_through_its_stable_spelling() {
    for mode in ColorMode::ALL {
        assert_eq!(ColorMode::parse_exact(mode.as_str()), Some(mode));
    }
    assert_eq!(ColorMode::parse_exact("sometimes"), None);
}

#[test]
fn the_default_color_mode_is_auto() {
    assert_eq!(ColorMode::default(), ColorMode::Auto);
}

#[test]
fn a_displayed_color_mode_is_a_value_the_option_accepts_back() {
    // helpの一覧も診断もDisplayを通る。`as_str`と離れると、画面に出た語をそのまま
    // `--color`へ渡しても拒否される。
    for mode in ColorMode::ALL {
        assert_eq!(mode.to_string(), mode.as_str());
        assert_eq!(ColorMode::parse_exact(&mode.to_string()), Some(mode));
    }
    assert_eq!(
        ColorMode::ALL.map(|mode| mode.to_string()).join(", "),
        "auto, always, never"
    );
    assert_eq!(ColorMode::default().to_string(), "auto");
}
