use std::path::Path;

use crate::design::Remediation;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Msg, Result};
use crate::msg;
use crate::project::CanonicalProjectId;

use super::{
    CloneTransport, Provider, Rejection, accepted_clone_url_forms, interpret, interpret_local,
};

/// 登録対象の不変なrepository identity。
///
/// 表示には入力の表記を、突き合わせにはcanonical project `IDとtransportを使う`。
/// GitHubのrepositoryでは、clone URLをこの構造から組み立て直すため、保存値と表示値が
/// 食い違わない。hostにあるrepositoryでは、そのpathをclone URLとして持つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryIdentity {
    pub(super) provider: Provider,
    pub(super) owner: String,
    pub(super) name: String,
    pub(super) canonical_id: CanonicalProjectId,
    pub(super) transport: CloneTransport,
    pub(super) clone_url: String,
}

impl RepositoryIdentity {
    /// `GitHubが表示するclone` URLを解釈する。
    ///
    /// 受理するのは次の2形式だけである。
    ///
    /// ```text
    /// git@github.com:<owner>/<repository>.git
    /// https://github.com/<owner>/<repository>.git
    /// ```
    pub fn parse_clone_url(value: &str) -> Result<RepositoryIdentity> {
        match interpret(value) {
            Ok(identity) => Ok(identity),
            // 予約されたrepository名のように、原因を名指しできる拒否はそのまま伝える。
            Err(Rejection::Project(error)) => Err(error),
            Err(Rejection::Form) => Err(invalid_clone_url(value)),
        }
    }

    /// hostにあるrepositoryを、そのpathと案件の名前から組み立てる。
    ///
    /// `path`は呼び出し側が実在を確かめ、正規化した絶対pathとする。案件IDは
    /// `local/<name>`になる。
    pub fn local(path: &str, name: &str) -> Result<RepositoryIdentity> {
        match interpret_local(path, name) {
            Ok(identity) => Ok(identity),
            Err(Rejection::Project(error)) => Err(error),
            Err(Rejection::Form) => Err(Error::new(
                ErrorId::InvalidLocalRepositoryPath,
                msg!("error-invalid-local-repository-path", value = path),
            )),
        }
    }

    /// 保存済みのfieldから復元する。
    ///
    /// clone URLを正本として読み直し、ほかのfieldがその解釈と一致することを確かめる。
    /// 一致しない保存値は、いずれか一方を正しいものとして採用せず、不一致として返す。
    pub fn from_parts(
        provider: &str,
        owner: &str,
        name: &str,
        canonical_id: &str,
        transport: &str,
        clone_url: &str,
    ) -> std::result::Result<RepositoryIdentity, Msg> {
        let identity =
            RepositoryIdentity::from_index_parts(provider, canonical_id, transport, clone_url)?;
        if identity.owner != owner || identity.name != name {
            return Err(msg!(
                "cause-clone-url-name-mismatch",
                observed = format!("{}/{}", identity.owner, identity.name),
                declared = format!("{owner}/{name}")
            ));
        }
        Ok(identity)
    }

    /// 索引が持つfieldから復元する。
    ///
    /// 表示上の綴りはclone URLから読み直す。索引は表示用のownerとrepositoryを二重に
    /// 保存しない。
    ///
    /// 保存されたproviderが、clone URLの読み方を決める。hostにあるrepositoryは
    /// pathと、canonical IDが持つ名前から読み直す。
    pub fn from_index_parts(
        provider: &str,
        canonical_id: &str,
        transport: &str,
        clone_url: &str,
    ) -> std::result::Result<RepositoryIdentity, Msg> {
        let provider = Provider::parse(provider)
            .ok_or_else(|| msg!("cause-provider-unsupported", observed = provider))?;
        let declared_transport = CloneTransport::parse(transport)
            .ok_or_else(|| msg!("cause-clone-transport-unsupported", observed = transport))?;
        let identity = match provider {
            Provider::Github => interpret(clone_url).map_err(|_| {
                msg!(
                    "cause-clone-url-unrecognized",
                    observed = clone_url,
                    accepted = accepted_clone_url_forms()
                )
            })?,
            Provider::Local => {
                let name = canonical_id
                    .split_once('/')
                    .map_or(canonical_id, |(_, name)| name);
                // pathの形の誤りと、名前が案件の名前にならないことを分けて示す。
                interpret_local(clone_url, name).map_err(|rejection| match rejection {
                    Rejection::Form => msg!(
                        "cause-local-repository-path-unrecognized",
                        observed = clone_url
                    ),
                    Rejection::Project(_) => msg!(
                        "cause-local-project-name-unrecognized",
                        observed = canonical_id
                    ),
                })?
            }
        };

        if identity.transport != declared_transport {
            return Err(msg!(
                "cause-clone-url-transport-mismatch",
                observed = identity.transport,
                declared = declared_transport
            ));
        }
        if identity.canonical_id.as_str() != canonical_id {
            return Err(msg!(
                "cause-clone-url-identity-mismatch",
                observed = identity.canonical_id,
                declared = canonical_id
            ));
        }
        Ok(identity)
    }

    pub fn provider(&self) -> Provider {
        self.provider
    }

    /// 表示上のowner。GitHubのrepositoryではGitHub上の表記のまま、hostにある
    /// repositoryでは`local`である。
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// 表示上のrepository名。GitHubのrepositoryではGitHub上の表記のまま、hostにある
    /// repositoryでは案件の名前である。
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn canonical_id(&self) -> &CanonicalProjectId {
        &self.canonical_id
    }

    pub fn transport(&self) -> CloneTransport {
        self.transport
    }

    /// 正規化したclone URL。hostにあるrepositoryでは、そのpathである。
    pub fn clone_url(&self) -> &str {
        &self.clone_url
    }

    /// GitHub tokenをsbxmへ登録し、Sandboxのcloneに使うか。
    ///
    /// GitHubのrepositoryはSandboxからcloneするため、tokenを要する。hostにある
    /// repositoryはhostから送るため、要らない。
    pub fn uses_github_token(&self) -> bool {
        self.provider == Provider::Github
    }

    /// hostにあるrepositoryのpath。GitHubのrepositoryには無い。
    pub fn host_path(&self) -> Option<&Path> {
        match self.provider {
            Provider::Github => None,
            Provider::Local => Some(Path::new(&self.clone_url)),
        }
    }

    /// 表示に使う`<owner>/<repository>`。
    pub fn display_id(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }

    /// 同じ案件をもう一度登録する`sbxm add`の引数。
    ///
    /// GitHub repositoryには登録時と同じclone URLを示す。
    /// hostにあるrepositoryにはpathを示し、directory名と別の名前で登録した案件には
    /// `--name`を添える。
    pub fn add_arguments(&self) -> String {
        match self.provider {
            Provider::Github => self.clone_url.clone(),
            Provider::Local => {
                let path = shell_word(&self.clone_url);
                let directory = std::path::Path::new(&self.clone_url)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(str::to_ascii_lowercase);
                if directory.as_deref() == Some(self.canonical_id.repository()) {
                    format!("--local {path}")
                } else {
                    format!("--local {path} --name {}", self.canonical_id.repository())
                }
            }
        }
    }

    /// 同じrepositoryを同じ方式でcloneする構成か。
    ///
    /// `GitHubではownerとrepositoryの表示上の大文字小文字だけが異なっても同じidentity`
    /// として扱う。transportとproviderの差異は同一構成とみなさない。hostにある
    /// repositoryは、同じ名前でも別のpathなら別の構成とする。
    pub fn same_target(&self, other: &RepositoryIdentity) -> bool {
        self.provider == other.provider
            && self.transport == other.transport
            && self.canonical_id == other.canonical_id
            && (self.provider != Provider::Local || self.clone_url == other.clone_url)
    }
}

impl std::fmt::Display for RepositoryIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.clone_url)
    }
}

/// shellへそのまま貼れる1語。安全な文字だけのpathは囲まない。
fn shell_word(value: &str) -> String {
    let plain = value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "/._-+@%:,=~".contains(c));
    if plain {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

/// 受理する形式を示して拒否する。
fn invalid_clone_url(value: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::InvalidCloneUrl,
            msg!(
                "error-invalid-clone-url",
                value = value,
                accepted = accepted_clone_url_forms()
            ),
        )
        .remediation(Remediation::text(msg!("remediation-invalid-clone-url"))),
    )
}
