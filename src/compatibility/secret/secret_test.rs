use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::design::Fact;
use crate::diagnostics::ErrorId;

/// 拒否が`Cause:`として示した原文。
///
/// 同じerror IDで拒む道が何本もあるため、どれを通ったかはこの行でしか区別できない。
fn refusal_cause(error: &crate::diagnostics::Error) -> Checked<String> {
    error
        .diagnostics()
        .first()
        .required_because("one diagnostic")?
        .facts
        .iter()
        .find_map(|fact| match fact {
            Fact::OneLine { label, value } if label.id == "diagnostic-cause-label" => {
                Some(value.as_str().to_string())
            }
            _ => None,
        })
        .required_because("the refusal states what could not be read")
}

#[test]
fn the_secret_listing_of_the_target_version_is_read_as_it_is() -> Checked {
    // 対象versionが実際に出力する形。service secretの表のあとに、見出しを挟んで
    // custom secretの表が続く。
    let observed = "SCOPE           TYPE      NAME     SECRET\n\
                        sbxm-example    service   github   (stored)\n\
                        \n\
                        CUSTOM SECRETS\n\
                        SCOPE          TARGETS      ENV        PLACEHOLDER      SECRET\n\
                        sbxm-example   github.com   GH_TOKEN   sbx-cs-example   ghp_example\n";
    assert_eq!(
        parse_custom_secrets(observed).required()?,
        vec![CustomSecret {
            scope: "sbxm-example".to_string(),
            placeholder: "sbx-cs-example".to_string(),
            targets: vec!["github.com".to_string()],
            env: "GH_TOKEN".to_string(),
        }]
    );

    // custom secretの見出しがない出力は、1件も登録がないことを示す。service secretの
    // 表だけを読み違えて登録ありとしない。
    let services_only = "SCOPE           TYPE      NAME     SECRET\n\
                             sbxm-example    service   github   (stored)\n";
    assert!(parse_custom_secrets(services_only).required()?.is_empty());

    // 1件もない場合は表ではなく文で示す。
    let absent = "No secrets found for scope \"sbxm-example\".\n";
    assert!(parse_custom_secrets(absent).required()?.is_empty());

    // 1つの列が複数のhostを並べることがある。空白1つで切ると列がずれる。
    let several = "CUSTOM SECRETS\n\
                       SCOPE          TARGETS                  ENV        PLACEHOLDER      SECRET\n\
                       sbxm-example   github.com gitlab.com    GH_TOKEN   sbx-cs-example   ghp_example\n";
    assert_eq!(
        parse_custom_secrets(several).required()?[0].targets,
        vec!["github.com".to_string(), "gitlab.com".to_string()]
    );

    // 実機がwildcardを登録したscopeで出す形。`TARGETS`はcommaと空白1つで区切り、
    // wildcardは展開せず書いたまま並べる。scope名とsecretは記録から伏せてある。
    let wildcards = "CUSTOM SECRETS\n\
                         SCOPE          TARGETS                                                        ENV        PLACEHOLDER               SECRET\n\
                         sbxm-example   github.com, **.github.com, **.githubusercontent.com, ghcr.io   GH_TOKEN   sbx-cs-Y1k0SfTWbkN6HzCO   ghp_redacted\n";
    let parsed = parse_custom_secrets(wildcards).required()?;
    assert_eq!(
        parsed,
        vec![CustomSecret {
            scope: "sbxm-example".to_string(),
            targets: vec![
                "github.com".to_string(),
                "**.github.com".to_string(),
                "**.githubusercontent.com".to_string(),
                "ghcr.io".to_string(),
            ],
            env: "GH_TOKEN".to_string(),
            placeholder: "sbx-cs-Y1k0SfTWbkN6HzCO".to_string(),
        }],
        "the pattern is compared as written, so it has to survive the listing unexpanded"
    );

    // scopeを読めない一覧からは、消してよい登録とほかのSandboxが使う登録を区別できない。
    let scopeless = "CUSTOM SECRETS\n\
                         TARGETS      ENV        PLACEHOLDER      SECRET\n\
                         github.com   GH_TOKEN   sbx-cs-example   ghp_example\n";

    for output in [
        "",
        "CUSTOM SECRETS\nSCOPE          ENV\nsbxm-example   GH_TOKEN\n",
        "CUSTOM SECRETS\nSCOPE          TARGETS      ENV\nsbxm-example   github.com\n",
        scopeless,
    ] {
        let error = parse_custom_secrets(output).refused_because("{output} must be refused")?;
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    }
    Ok(())
}

#[test]
fn the_custom_secret_table_ends_at_the_first_blank_line() -> Checked {
    // 表の下に文が続く出力がある。空行で切らずに読み進めると、列数の合わない行として
    // 一覧全体を拒み、実在する登録を見落とす。
    let observed = "CUSTOM SECRETS\n\
                        SCOPE          TARGETS      ENV        PLACEHOLDER      SECRET\n\
                        sbxm-example   github.com   GH_TOKEN   sbx-cs-example   ghp_example\n\
                        \n\
                        Run `sbx secret set` to add another.\n";

    let parsed =
        parse_custom_secrets(observed).required_because("the closing sentence is not a row")?;
    assert_eq!(
        parsed,
        vec![CustomSecret {
            scope: "sbxm-example".to_string(),
            targets: vec!["github.com".to_string()],
            env: "GH_TOKEN".to_string(),
            placeholder: "sbx-cs-example".to_string(),
        }]
    );
    Ok(())
}

#[test]
fn a_row_that_does_not_fill_the_columns_is_refused_with_both_counts() -> Checked {
    // 列がずれた行では、どの値がENVでどれがPLACEHOLDERかを決められない。数の食い違いを
    // そのまま示さないと、読み手は出力のどこがずれたかを探せない。
    let short_row = "CUSTOM SECRETS\n\
                         SCOPE          TARGETS      ENV        PLACEHOLDER      SECRET\n\
                         sbxm-example   github.com   GH_TOKEN   sbx-cs-example\n";

    let error =
        parse_custom_secrets(short_row).refused_because("a row that fills four of five columns")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    assert_eq!(
        refusal_cause(&error)?,
        "a custom secret row holds 4 values for 5 columns"
    );
    Ok(())
}

#[test]
fn a_heading_with_no_table_under_it_is_refused_rather_than_read_as_none() -> Checked {
    // 見出しの直後で途切れた出力を「1件もない」と読むと、残っている登録を消えたものと
    // して扱う。空行しか続かない場合も同じで、列が並ばない限り読めていない。
    for truncated in [
        "SCOPE           TYPE      NAME     SECRET\n\
             sbxm-example    service   github   (stored)\n\
             \n\
             CUSTOM SECRETS",
        "CUSTOM SECRETS\n   \n",
    ] {
        let error = parse_custom_secrets(truncated)
            .refused_because("a listing that stops at the heading")?;
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
        assert_eq!(
            refusal_cause(&error)?,
            "the custom secret listing has no header"
        );
    }
    Ok(())
}

#[test]
fn an_answer_with_nothing_in_it_is_not_read_as_an_absence_of_secrets() -> Checked {
    // 何も書かれていない出力は観測ではない。1件もないことは文で示される。
    let error = parse_custom_secrets("   \n").refused_because("nothing was said at all")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    assert_eq!(refusal_cause(&error)?, "the output is empty");
    assert!(
        parse_custom_secrets("No secrets found for scope \"sbxm-example\".\n")
            .required_because("a stated absence")?
            .is_empty()
    );
    Ok(())
}

#[test]
fn a_column_the_listing_does_not_carry_is_named_in_the_refusal() -> Checked {
    // 欠けた列ごとに読めなくなるものが違う。SCOPEがなければ消してよい登録を選べず、
    // PLACEHOLDERがなければ登録をやり直せない。どの列かを示す。
    for (output, cause) in [
        (
            "CUSTOM SECRETS\nTARGETS      ENV        PLACEHOLDER\ngithub.com   GH_TOKEN   sbx-cs-a\n",
            "the custom secret listing has no SCOPE column",
        ),
        (
            "CUSTOM SECRETS\nSCOPE          ENV        PLACEHOLDER\nsbxm-example   GH_TOKEN   sbx-cs-a\n",
            "the custom secret listing has no TARGETS column",
        ),
        (
            "CUSTOM SECRETS\nSCOPE          TARGETS      PLACEHOLDER\nsbxm-example   github.com   sbx-cs-a\n",
            "the custom secret listing has no ENV column",
        ),
        (
            "CUSTOM SECRETS\nSCOPE          TARGETS      ENV\nsbxm-example   github.com   GH_TOKEN\n",
            "the custom secret listing has no PLACEHOLDER column",
        ),
    ] {
        let error = parse_custom_secrets(output).refused_because("a listing missing a column")?;
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
        assert_eq!(refusal_cause(&error)?, cause);
    }
    Ok(())
}
