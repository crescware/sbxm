use crate::project::ProjectId;

use super::{CloneTransport, LOCAL_OWNER, Provider, Rejection, RepositoryIdentity, local_name};

/// hostにあるrepositoryのpathと案件の名前を、identityへまとめる。
///
/// pathは正規化済みの絶対pathだけを受け取る。`.`や`..`を含むpath、末尾がslashのpath、
/// 制御文字を含むpathは、同じrepositoryを別の綴りで指せてしまうため拒否する。
pub(super) fn interpret_local(
    path: &str,
    name: &str,
) -> std::result::Result<RepositoryIdentity, Rejection> {
    if !is_normalized_absolute(path) {
        return Err(Rejection::Form);
    }
    let id = ProjectId::parse(&format!("{LOCAL_OWNER}/{name}")).map_err(Rejection::Project)?;
    let canonical_id = id.canonical();
    Ok(RepositoryIdentity {
        provider: Provider::Local,
        owner: LOCAL_OWNER.to_string(),
        name: local_name(path, canonical_id.repository()),
        canonical_id,
        transport: CloneTransport::File,
        clone_url: path.to_string(),
    })
}

fn is_normalized_absolute(path: &str) -> bool {
    // `Path::components`は途中の`.`を黙って読み飛ばすため、区切りごとに確かめる。
    let Some(rest) = path.strip_prefix('/') else {
        return false;
    };
    !path.chars().any(char::is_control)
        && rest
            .split('/')
            .all(|segment| !matches!(segment, "" | "." | ".."))
}
