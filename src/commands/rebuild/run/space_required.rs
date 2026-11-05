use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::diagnostics::{Result, unparseable};
use crate::git;
use crate::metadata::ProjectMetadata;
use crate::project::SandboxLayout;
use crate::support::sandbox;

/// repository転送の一時領域とcheckoutのための保守的な見積もり。KiB単位。
pub(super) fn space_required(
    host: &dyn HostEnvironment,
    name: &str,
    metadata: &ProjectMetadata,
) -> Result<(u64, u64)> {
    let layout = SandboxLayout::new(metadata.canonical_id());
    let size = sandbox::exec_with_timeout(
        host,
        name,
        &["du", "-sk", "--", &layout.bare_git_dir()],
        TimeoutClass::LocalFilesystem,
    )?
    .require_success()?
    .stdout_text();
    let repository: u64 = size
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .parse()
        .map_err(|_| unparseable("du -sk", "missing repository allocation"))?;
    let branch = metadata
        .provisioning
        .start_ref
        .as_deref()
        .ok_or_else(|| unparseable("rebuild capacity", "missing start branch"))?;
    let reference = git::origin_ref(branch);
    let local = format!("refs/heads/{branch}");
    let mut sources = std::collections::BTreeSet::from([reference]);
    let local_state = sandbox::exec(
        host,
        name,
        &[
            "git",
            "--git-dir",
            &layout.bare_git_dir(),
            "show-ref",
            "--verify",
            "--quiet",
            &local,
        ],
    )?;
    if local_state.success() {
        sources.insert(local);
    } else if local_state.status.code() != Some(1) {
        return Err(sandbox::unobservable(&local_state, "local checkout branch"));
    }
    sources.extend(checkout_heads(host, name, &layout.bare_git_dir())?);
    let mut bytes = 0;
    let mut entries = 0;
    for source in sources {
        let tree = sandbox::exec_with_timeout(
            host,
            name,
            &[
                "env",
                "GIT_OPTIONAL_LOCKS=0",
                "GIT_NO_LAZY_FETCH=1",
                "git",
                "--git-dir",
                &layout.bare_git_dir(),
                "ls-tree",
                "-rl",
                &source,
            ],
            TimeoutClass::LocalFilesystem,
        )?
        .require_success()?
        .stdout_text();
        let (size, files) = tree_size(&tree)?;
        bytes = bytes.max(size);
        entries = entries.max(files);
    }
    let count = u64::from(metadata.provisioning.requested_worktrees);
    Ok((
        repository
            .saturating_mul(2)
            .saturating_add(bytes.div_ceil(1024).saturating_mul(count))
            .saturating_add(128 * 1024),
        entries.saturating_mul(count).saturating_add(256),
    ))
}

fn tree_size(tree: &str) -> Result<(u64, u64)> {
    let mut bytes = 0_u64;
    let mut entries = 0_u64;
    for line in tree.lines() {
        let (header, path) = line
            .split_once('\t')
            .ok_or_else(|| unparseable("git ls-tree", "missing tree entry path"))?;
        let fields: Vec<_> = header.split_whitespace().collect();
        let [_, kind, _, size] = fields.as_slice() else {
            return Err(unparseable("git ls-tree", "invalid tree entry"));
        };
        if *kind == "commit" {
            continue;
        } // submoduleは自動展開しない。
        let size: u64 = size
            .parse()
            .map_err(|_| unparseable("git ls-tree", "invalid blob size"))?;
        if *kind != "blob" {
            return Err(unparseable("git ls-tree", "unexpected object kind"));
        }
        bytes = bytes.saturating_add(size);
        entries =
            entries.saturating_add(u64::try_from(path.split('/').count()).unwrap_or(u64::MAX));
    }
    Ok((bytes, entries))
}

#[cfg(test)]
#[path = "space_required_test.rs"]
mod space_required_test;

fn checkout_heads(
    host: &dyn HostEnvironment,
    name: &str,
    git_dir: &str,
) -> Result<std::collections::BTreeSet<String>> {
    let mut sources = std::collections::BTreeSet::new();
    let listed = sandbox::exec(
        host,
        name,
        &[
            "git",
            "--git-dir",
            git_dir,
            "worktree",
            "list",
            "--porcelain",
            "-z",
        ],
    )?
    .require_success()?
    .stdout_text();
    for (entry, record) in crate::support::worktree::parse_list(&listed)?
        .iter()
        .zip(listed.split("\0\0"))
    {
        if entry.bare {
            continue;
        }
        let head = record
            .split('\0')
            .find_map(|field| field.strip_prefix("HEAD "))
            .filter(|oid| {
                [40, 64].contains(&oid.len()) && oid.bytes().all(|b| b.is_ascii_hexdigit())
            })
            .ok_or_else(|| unparseable("git worktree list", "missing or invalid checkout HEAD"))?;
        sources.insert(head.into());
    }
    Ok(sources)
}
