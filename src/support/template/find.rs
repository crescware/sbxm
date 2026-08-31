use crate::boundary::host::protocol::{TemplateEntry, parse_template_list};
use crate::boundary::host::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;

/// 名前が完全一致するTemplateを探す。
pub fn find(host: &dyn HostEnvironment, name: &str) -> Result<Option<TemplateEntry>> {
    let spec = CommandSpec::capture("sbx", &["template", "ls", "--json"])
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::SandboxLifecycle);
    let outcome = host.run(&spec)?.require_success()?;
    let entries = parse_template_list(&outcome.stdout_text())?;
    Ok(entries.into_iter().find(|entry| entry.is_named(name)))
}
