use super::*;
use crate::error::ErrorId;

#[test]
fn the_secret_listing_of_the_target_version_is_read_as_it_is() {
    // 対象versionが実際に出力する形。service secretの表のあとに、見出しを挟んで
    // custom secretの表が続く。
    let observed = "SCOPE           TYPE      NAME     SECRET\n\
                        sbxm-example    service   github   (stored)\n\
                        \n\
                        CUSTOM SECRETS\n\
                        SCOPE          TARGETS      ENV        PLACEHOLDER      SECRET\n\
                        sbxm-example   github.com   GH_TOKEN   sbx-cs-example   ghp_example\n";
    assert_eq!(
        parse_custom_secrets(observed).unwrap(),
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
    assert!(parse_custom_secrets(services_only).unwrap().is_empty());

    // 1件もない場合は表ではなく文で示す。
    let absent = "No secrets found for scope \"sbxm-example\".\n";
    assert!(parse_custom_secrets(absent).unwrap().is_empty());

    // 1つの列が複数のhostを並べることがある。空白1つで切ると列がずれる。
    let several = "CUSTOM SECRETS\n\
                       SCOPE          TARGETS                  ENV        PLACEHOLDER      SECRET\n\
                       sbxm-example   github.com gitlab.com    GH_TOKEN   sbx-cs-example   ghp_example\n";
    assert_eq!(
        parse_custom_secrets(several).unwrap()[0].targets,
        vec!["github.com".to_string(), "gitlab.com".to_string()]
    );

    // 実機がwildcardを登録したscopeで出す形。`TARGETS`はcommaと空白1つで区切り、
    // wildcardは展開せず書いたまま並べる。scope名とsecretは記録から伏せてある。
    let wildcards = "CUSTOM SECRETS\n\
                         SCOPE          TARGETS                                                        ENV        PLACEHOLDER               SECRET\n\
                         sbxm-example   github.com, **.github.com, **.githubusercontent.com, ghcr.io   GH_TOKEN   sbx-cs-Y1k0SfTWbkN6HzCO   ghp_redacted\n";
    let parsed = parse_custom_secrets(wildcards).unwrap();
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
        let error = parse_custom_secrets(output).expect_err("{output} must be refused");
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    }
}
