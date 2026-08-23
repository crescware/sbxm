use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::config::{ConfigLocation, GlobalConfig};
use crate::design::ProgressSink;
use crate::diagnostics::Result;
use crate::msg;
use crate::project::{ProjectId, SandboxLayout, SandboxName};
use crate::support::select::ProjectPrompt;
use crate::support::{generation, image, provisioning};

use super::PrepareOutput;

/// 対象を引数またはpromptで解決し、登録済み案件のSandboxを構築する。
pub fn run(
    location: &ConfigLocation,
    config: &GlobalConfig,
    requested: Option<&ProjectId>,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    prompt: &mut dyn ProjectPrompt,
    progress: &mut dyn ProgressSink,
) -> Result<PrepareOutput> {
    // 対象が決まる前にhostの状態へ触れない。
    let mut locked =
        crate::support::select::one(location, requested, &msg!("select-prepare-heading"), prompt)?
            .lock()?;
    let mut warnings = image::cleanup_stale_archives(&locked.paths)?;
    generation::require_no_rebuild(&locked.metadata)?;

    let canonical = locked.metadata.canonical_id().clone();
    let name = SandboxName::derive(&canonical);
    let layout = SandboxLayout::new(&canonical);

    if let Some(mut output) = provisioning::already_built(
        host,
        &locked.paths,
        &name,
        &locked.metadata,
        &layout,
        workspace_root,
    )? {
        output.warnings = warnings;
        return Ok(output);
    }

    // custom secretはSandboxの作成時に結び付く。あとから登録しても既存のSandboxには
    // 届かないため、作成より前に、そしてimageを組む前に確認する。Dockerの到達性も
    // ここで一度だけ確認し、以降のgeneration解決や`provision`の中では再確認しない。
    let preconditions = provisioning::verify_external_preconditions(host, &name)?;

    let (generation, target_warnings) =
        provisioning::fresh_target(host, &locked.paths, &mut locked.metadata, &name)?;
    warnings.extend(target_warnings);

    provisioning::provision(
        &mut locked,
        config,
        &generation,
        preconditions,
        host,
        workspace_root,
        progress,
        warnings,
    )
}

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;

#[cfg(test)]
#[path = "resume_test.rs"]
mod resume_test;

#[cfg(test)]
#[path = "generation_test.rs"]
mod generation_test;

#[cfg(test)]
#[path = "worktree_test.rs"]
mod worktree_test;

#[cfg(test)]
#[path = "secret_test.rs"]
mod secret_test;

#[cfg(test)]
#[path = "tools_test.rs"]
mod tools_test;

#[cfg(test)]
#[path = "output_test.rs"]
mod output_test;

#[cfg(test)]
#[path = "external_calls_test.rs"]
mod external_calls_test;
