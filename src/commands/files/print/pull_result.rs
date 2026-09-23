use crate::design::Document;
use crate::msg;
use crate::paths;

use crate::commands::files::{PullOutcome, Pulled};

/// `files pull`の結果。置き換えたときは、ほかの案件へ広げる手順も示す。
pub fn pull_result(pulled: &Pulled, outcome: &PullOutcome) -> Document {
    let destination = paths::display(pulled.declaration.destination.as_path());
    let source = paths::display(pulled.declaration.source.as_path());
    let project = pulled.project.clone();
    match outcome {
        PullOutcome::Same => Document::new().summary(msg!(
            "files-pull-same",
            destination = destination,
            project = project,
            source = source
        )),
        PullOutcome::Adopted(path) => Document::new()
            .summary(msg!(
                "files-pull-adopted",
                destination = destination,
                project = project,
                source = paths::display(path)
            ))
            .note(msg!("files-pull-spread-hint"))
            .try_command("sbxm apply --files --all"),
        PullOutcome::Kept => {
            Document::new().summary(msg!("files-pull-kept", project = project, source = source))
        }
        PullOutcome::Undecided => Document::new().note(msg!("files-pull-undecided")),
    }
}
