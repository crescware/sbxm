use crate::design::Remediation;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::project::CanonicalProjectId;

use super::{CloneTransport, Provider, Rejection, accepted_clone_url_forms, interpret};

/// 登録対象の不変なrepository identity。
///
/// 表示にはGitHub上の表記を、突き合わせにはcanonical project `IDとtransportを使う`。
/// clone URLはこの構造から組み立て直すため、保存値と表示値が食い違わない。
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
    ) -> std::result::Result<RepositoryIdentity, String> {
        let identity =
            RepositoryIdentity::from_index_parts(provider, canonical_id, transport, clone_url)?;
        if identity.owner != owner || identity.name != name {
            return Err(format!(
                "the clone URL names {}/{}, not {owner}/{name}",
                identity.owner, identity.name
            ));
        }
        Ok(identity)
    }

    /// 索引が持つfieldから復元する。
    ///
    /// 表示上の綴りはclone URLから読み直す。索引は表示用のownerとrepositoryを二重に
    /// 保存しない。
    pub fn from_index_parts(
        provider: &str,
        canonical_id: &str,
        transport: &str,
        clone_url: &str,
    ) -> std::result::Result<RepositoryIdentity, String> {
        let declared_provider = Provider::parse(provider)
            .ok_or_else(|| format!("{provider} is not a supported provider"))?;
        let declared_transport = CloneTransport::parse(transport)
            .ok_or_else(|| format!("{transport} is not a supported clone transport"))?;
        let identity = interpret(clone_url)
            .map_err(|_| format!("{clone_url} is not one of {}", accepted_clone_url_forms()))?;

        if identity.provider != declared_provider {
            return Err(format!(
                "the clone URL names provider {}, not {declared_provider}",
                identity.provider
            ));
        }
        if identity.transport != declared_transport {
            return Err(format!(
                "the clone URL uses the {} transport, not {declared_transport}",
                identity.transport
            ));
        }
        if identity.canonical_id.as_str() != canonical_id {
            return Err(format!(
                "the clone URL folds to {}, not {canonical_id}",
                identity.canonical_id
            ));
        }
        Ok(identity)
    }

    pub fn provider(&self) -> Provider {
        self.provider
    }

    /// `GitHub上の表記のままのowner`。
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// `GitHub上の表記のままのrepository`。
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn canonical_id(&self) -> &CanonicalProjectId {
        &self.canonical_id
    }

    pub fn transport(&self) -> CloneTransport {
        self.transport
    }

    /// 正規化したclone URL。
    pub fn clone_url(&self) -> &str {
        &self.clone_url
    }

    /// 表示に使う`<owner>/<repository>`。
    pub fn display_id(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }

    /// 同じrepositoryを同じ方式でcloneする構成か。
    ///
    /// `GitHubではownerとrepositoryの表示上の大文字小文字だけが異なっても同じidentity`
    /// として扱う。transportとproviderの差異は同一構成とみなさない。
    pub fn same_target(&self, other: &RepositoryIdentity) -> bool {
        self.provider == other.provider
            && self.transport == other.transport
            && self.canonical_id == other.canonical_id
    }
}

impl std::fmt::Display for RepositoryIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.clone_url)
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
