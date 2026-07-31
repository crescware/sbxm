use std::path::Path;

use crate::command::HostEnvironment;
use crate::diagnostics::Result;
use crate::paths;

use crate::support::sandbox;

use super::AGENT_HOME;

pub(super) fn copy_steps(
    host: &dyn HostEnvironment,
    sandbox: &str,
    source: &Path,
    staged: &str,
    destination: &str,
    pending: &str,
) -> Result<()> {
    let spec = crate::command::CommandSpec::capture(
        "sbx",
        &[
            "cp",
            "--follow-link",
            &paths::display(source),
            &format!("{sandbox}:{staged}"),
        ],
    )
    .env(crate::command::EnvPolicy::InheritWithoutSshAgent)
    .timeout(crate::command::TimeoutClass::SandboxLifecycle);
    host.run(&spec)?.require_success()?;

    let parent = destination
        .rsplit_once('/')
        .map_or_else(|| AGENT_HOME.to_string(), |(parent, _)| parent.to_string());
    sandbox::exec_as_root(
        host,
        sandbox,
        &[
            "install", "-d", "-o", "agent", "-g", "agent", "-m", "0700", &parent,
        ],
    )?
    .require_success()?;

    sandbox::exec_as_root(
        host,
        sandbox,
        &[
            "install", "-o", "agent", "-g", "agent", "-m", "0600", staged, pending,
        ],
    )?
    .require_success()?;

    // 置き換えはrenameで行い、読み手へ半端な内容を見せない。
    sandbox::exec_as_root(host, sandbox, &["mv", "-f", pending, destination])?.require_success()?;
    Ok(())
}
