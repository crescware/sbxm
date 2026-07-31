use crate::project::ProjectId;

use super::{
    CloneTransport, GIT_SUFFIX, GITHUB_HOST, Provider, Rejection, RepositoryIdentity, SSH_USER,
    split_repository_path, split_transport,
};

/// clone URLを解釈する。
pub(super) fn interpret(value: &str) -> std::result::Result<RepositoryIdentity, Rejection> {
    let (transport, path) = split_transport(value).ok_or(Rejection::Form)?;
    let (owner, name) = split_repository_path(path).ok_or(Rejection::Form)?;

    let id = ProjectId::parse(&format!("{owner}/{name}")).map_err(Rejection::Project)?;
    let canonical_id = id.canonical();
    let clone_url = match transport {
        CloneTransport::Ssh => format!("{SSH_USER}@{GITHUB_HOST}:{owner}/{name}{GIT_SUFFIX}"),
        CloneTransport::Https => format!("https://{GITHUB_HOST}/{owner}/{name}{GIT_SUFFIX}"),
    };

    Ok(RepositoryIdentity {
        provider: Provider::Github,
        owner: owner.to_string(),
        name: name.to_string(),
        canonical_id,
        transport,
        clone_url,
    })
}
