use crate::design::{OutputPolicy, Ui};
use crate::i18n::Locale;

use crate::testing::outcome::{Checked, Required};

use super::*;

/// 世代のhash全体。表示は先頭だけを使う。
const APPLIED: &str = "4a0f8d41e27e53198137451dd09bc8aa8b8704b1f879a77655d643302029e33a";

fn output() -> RebuildOutput {
    RebuildOutput {
        project: "Example-Org/Example-Repo".to_string(),
        sandbox: "sbxm-example-org-example-repo-99a40327a69b".to_string(),
        applied: APPLIED.to_string(),
        warnings: Vec::new(),
    }
}

fn rendered(output: &RebuildOutput) -> Checked<String> {
    let mut written: Vec<u8> = Vec::new();
    {
        let mut ui = Ui::capture(
            Locale::En,
            OutputPolicy::plain(),
            &mut written,
            std::io::sink(),
        );
        ui.stdout(&document(output));
    }
    String::from_utf8(written).required_because("the rendered document is UTF-8")
}

#[test]
fn the_summary_names_the_project_the_sandbox_and_the_generation_it_applied() -> Checked {
    let output = output();
    let text = rendered(&output)?;

    assert!(text.contains(&output.project), "{text}");
    assert!(text.contains(&output.sandbox), "{text}");
    // 世代はimage名と同じ短縮表記で示す。hash全体は読み手の役に立たない。
    assert!(text.contains("4a0f8d41e27e"), "{text}");
    assert!(!text.contains(APPLIED), "{text}");
    Ok(())
}
