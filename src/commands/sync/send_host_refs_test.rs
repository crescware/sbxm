use std::path::PathBuf;

use crate::support::repository::SandboxOrigin;
use crate::testing::outcome::{Checked, Required};
use crate::testing::repository::git_in;
use crate::testing::sandbox::LocalSandbox;

use super::super::SentChange;
use super::send_host_refs;

/// hostのrepositoryと、Sandboxの中を模したbare repository。
struct Sending {
    _root: tempfile::TempDir,
    host: PathBuf,
    sandbox: String,
}

impl Sending {
    fn new() -> Checked<Sending> {
        let root = tempfile::tempdir().required()?;
        let host = root.path().join("host");
        std::fs::create_dir(&host).required()?;
        git_in(&host, &["init", "--quiet"])?;
        git_in(
            &host,
            &["commit", "--quiet", "--allow-empty", "-m", "first"],
        )?;
        git_in(&host, &["branch", "--quiet", "topic"])?;
        git_in(&host, &["tag", "v1"])?;
        git_in(root.path(), &["init", "--quiet", "--bare", "sandbox.git"])?;
        let sandbox = root
            .path()
            .join("sandbox.git")
            .to_string_lossy()
            .into_owned();
        Ok(Sending {
            _root: root,
            host,
            sandbox,
        })
    }

    fn send(&self) -> Checked<Vec<SentChange>> {
        let origin = SandboxOrigin::Host {
            repository: self.host.clone(),
            project: "local/app".to_string(),
        };
        send_host_refs(&LocalSandbox, &origin, "sbxm-local-app", &self.sandbox).required()
    }
}

fn created(reference: &str) -> SentChange {
    SentChange::Created {
        reference: reference.to_string(),
    }
}

#[test]
fn what_changed_in_the_sandbox_origin_is_listed_with_what_git_refused() -> Checked {
    let sending = Sending::new()?;

    assert_eq!(
        sending.send()?,
        vec![
            created("refs/remotes/origin/main"),
            created("refs/remotes/origin/topic"),
            created("refs/tags/v1"),
        ]
    );

    // hostで消したbranchはoriginから消える。Sandboxが別の先で持つtagは、そのまま残る。
    git_in(&sending.host, &["branch", "--quiet", "-D", "topic"])?;
    git_in(
        &sending.host,
        &["commit", "--quiet", "--allow-empty", "-m", "second"],
    )?;
    git_in(&sending.host, &["tag", "--force", "v1"])?;
    let changes = sending.send()?;

    assert_eq!(
        changes,
        vec![
            SentChange::Updated {
                reference: "refs/remotes/origin/main".to_string(),
            },
            SentChange::Removed {
                reference: "refs/remotes/origin/topic".to_string(),
            },
            SentChange::Refused {
                reference: "refs/tags/v1".to_string(),
                reason: "already exists".to_string(),
            },
        ]
    );
    Ok(())
}
