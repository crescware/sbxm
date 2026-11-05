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
    allocation_unit: u64,
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
    for source in checkout_trees(host, name, &layout.bare_git_dir(), &sources)? {
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
        let (size, files) = tree_size(&tree, allocation_unit)?;
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

fn tree_size(tree: &str, allocation_unit: u64) -> Result<(u64, u64)> {
    if allocation_unit == 0 {
        return Err(unparseable("rebuild capacity", "zero allocation unit"));
    }
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
        let depth = u64::try_from(path.split('/').count()).unwrap_or(u64::MAX);
        // 新filesystemの割当単位でfileごとに切り上げる。directoryのblockと、
        // index・directory entryの名前・固定fieldも保守的に含める。
        let content = size
            .div_ceil(allocation_unit)
            .saturating_mul(allocation_unit);
        let directories = depth.saturating_sub(1).saturating_mul(allocation_unit);
        let name_bytes = u64::try_from(path.len()).unwrap_or(u64::MAX);
        bytes = bytes
            .saturating_add(content)
            .saturating_add(directories)
            .saturating_add(name_bytes.saturating_mul(2).saturating_add(256));
        entries = entries.saturating_add(depth);
    }
    Ok((
        bytes
            .div_ceil(allocation_unit)
            .saturating_mul(allocation_unit),
        entries,
    ))
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
    for entry in crate::support::worktree::parse_list(&listed)? {
        if entry.bare {
            continue;
        }
        sources.insert(entry.head_oid()?.into());
    }
    Ok(sources)
}

/// refの別名や、同じtreeを指す別commitは、一括でtree OIDへ解決してから重複排除する。
fn checkout_trees(
    host: &dyn HostEnvironment,
    name: &str,
    git_dir: &str,
    sources: &std::collections::BTreeSet<String>,
) -> Result<std::collections::BTreeSet<String>> {
    let expressions: Vec<_> = sources
        .iter()
        .map(|source| format!("{source}^{{tree}}"))
        .collect();
    let mut args = vec![
        "env",
        "GIT_OPTIONAL_LOCKS=0",
        "GIT_NO_LAZY_FETCH=1",
        "git",
        "--git-dir",
        git_dir,
        "rev-parse",
        "--revs-only",
        "--end-of-options",
    ];
    args.extend(expressions.iter().map(String::as_str));
    let output = sandbox::exec(host, name, &args)?
        .require_success()?
        .stdout_text();
    let trees: Vec<_> = output.lines().collect();
    if trees.len() != sources.len() || !trees.iter().all(|tree| git::is_object_id(tree)) {
        return Err(unparseable(
            "git rev-parse",
            "missing or invalid checkout tree IDs",
        ));
    }
    Ok(trees.into_iter().map(str::to_string).collect())
}
