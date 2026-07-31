//! GitHub repository identity。
//!
//! `利用者がGitHubからそのままcopyできるclone` URLだけを入力として受け取り、provider、
//! 表示上のowner・repository、canonical project ID、clone transport、正規化した
//! clone URLへ分離する。
//!
//! 入力を寛容に推測して未対応形式へ対応しない。未対応形式は、受理する2形式を示して
//! 拒否する。Sandbox内で使うremoteは`crate::git`が別に組み立てる。本moduleが扱うのは
//! 登録対象そのものの不変なidentityである。

use crate::error::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::project::{CanonicalProjectId, ProjectId};
use crate::ui::Remediation;

/// 受理するhost。初期versionはGitHubだけを対象とする。
const GITHUB_HOST: &str = "github.com";
/// SSH clone `URLのuser`。
const SSH_USER: &str = "git";
/// clone URLの接尾辞。
const GIT_SUFFIX: &str = ".git";

/// 受理するSSH形式。
pub const SSH_CLONE_URL_FORM: &str = "git@github.com:<owner>/<repository>.git";
/// 受理するHTTPS形式。
pub const HTTPS_CLONE_URL_FORM: &str = "https://github.com/<owner>/<repository>.git";
/// clone URLを利用者が差し替える位置。実行できるcommandを示せない案内で使う。
pub const CLONE_URL_PLACEHOLDER: &str = "<github-clone-url>";

/// 受理する形式の一覧。error本文へ観測値と並べて示す。
pub fn accepted_clone_url_forms() -> String {
    format!("{SSH_CLONE_URL_FORM}, {HTTPS_CLONE_URL_FORM}")
}

/// repositoryをhostしているservice。
///
/// 初期versionはGitHubだけを持つ。値はconfigやmetadataへ保存するため翻訳しない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Github,
}

impl Provider {
    /// 翻訳しない安定した表記。
    pub fn as_str(self) -> &'static str {
        match self {
            Provider::Github => "github",
        }
    }

    /// 保存済みの値を読む。未知のproviderは推測せず`None`とする。
    pub fn parse(value: &str) -> Option<Provider> {
        match value {
            "github" => Some(Provider::Github),
            _ => None,
        }
    }
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// host cloneが使うclone方式。
///
/// SSHとHTTPSは同じrepositoryを指していても別の構成として扱う。暗黙に変換しない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloneTransport {
    Ssh,
    Https,
}

impl CloneTransport {
    /// 翻訳しない安定した表記。
    pub fn as_str(self) -> &'static str {
        match self {
            CloneTransport::Ssh => "ssh",
            CloneTransport::Https => "https",
        }
    }

    /// 保存済みの値を読む。未知のtransportは推測せず`None`とする。
    pub fn parse(value: &str) -> Option<CloneTransport> {
        match value {
            "ssh" => Some(CloneTransport::Ssh),
            "https" => Some(CloneTransport::Https),
            _ => None,
        }
    }
}

impl std::fmt::Display for CloneTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 登録対象の不変なrepository identity。
///
/// 表示にはGitHub上の表記を、突き合わせにはcanonical project `IDとtransportを使う`。
/// clone URLはこの構造から組み立て直すため、保存値と表示値が食い違わない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryIdentity {
    provider: Provider,
    owner: String,
    name: String,
    canonical_id: CanonicalProjectId,
    transport: CloneTransport,
    clone_url: String,
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

/// 解釈できなかった理由。
enum Rejection {
    /// 形式そのものが受理する2形式ではない。
    Form,
    /// 形式は合っているが、ownerまたはrepositoryが案件IDの規則に違反する。
    Project(Error),
}

/// clone URLを解釈する。
fn interpret(value: &str) -> std::result::Result<RepositoryIdentity, Rejection> {
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

/// transportを決め、`<owner>/<repository>.git`にあたる部分を返す。
///
/// `ssh://`と`http://`は受理しない。HTTPSはcredential、port、query、fragmentを
/// 持たないものだけを受理する。
fn split_transport(value: &str) -> Option<(CloneTransport, &str)> {
    if let Some(rest) = value.strip_prefix(&format!("{SSH_USER}@")) {
        let (host, path) = rest.split_once(':')?;
        require_github(host)?;
        return Some((CloneTransport::Ssh, path));
    }

    let rest = value.strip_prefix("https://")?;
    let (authority, path) = rest.split_once('/')?;
    // credentialとportを持つauthorityは受理しない。
    if authority.contains('@') || authority.contains(':') {
        return None;
    }
    require_github(authority)?;
    if path.contains('?') || path.contains('#') {
        return None;
    }
    Some((CloneTransport::Https, path))
}

/// `<owner>/<repository>.git`だけを受理し、ownerとrepositoryへ分ける。
fn split_repository_path(path: &str) -> Option<(&str, &str)> {
    let path = path.strip_suffix(GIT_SUFFIX)?;
    let mut parts = path.split('/');
    let (Some(owner), Some(name), None) = (parts.next(), parts.next(), parts.next()) else {
        return None;
    };
    if owner.is_empty() || name.is_empty() {
        return None;
    }
    Some((owner, name))
}

fn require_github(host: &str) -> Option<()> {
    host.eq_ignore_ascii_case(GITHUB_HOST).then_some(())
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

#[cfg(test)]
#[path = "repository_test.rs"]
mod repository_test;
