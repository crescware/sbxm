//! GitHub repository identity。
//!
//! `利用者がGitHubからそのままcopyできるclone` URLだけを入力として受け取り、provider、
//! 表示上のowner・repository、canonical project ID、clone transport、正規化した
//! clone URLへ分離する。
//!
//! 入力を寛容に推測して未対応形式へ対応しない。未対応形式は、受理する2形式を示して
//! 拒否する。Sandbox内で使うremoteは`crate::git`が別に組み立てる。本moduleが扱うのは
//! 登録対象そのものの不変なidentityである。

mod accepted_clone_url_forms;
mod clone_transport;
mod clone_url_placeholder;
mod git_suffix;
mod github_host;
mod https_clone_url_form;
mod interpret;
mod provider;
mod rejection;
mod repository_identity;
mod require_github;
mod split_repository_path;
mod split_transport;
mod ssh_clone_url_form;
mod ssh_user;

pub use accepted_clone_url_forms::accepted_clone_url_forms;
pub use clone_transport::CloneTransport;
pub use clone_url_placeholder::CLONE_URL_PLACEHOLDER;
use git_suffix::GIT_SUFFIX;
use github_host::GITHUB_HOST;
pub use https_clone_url_form::HTTPS_CLONE_URL_FORM;
use interpret::interpret;
pub use provider::Provider;
use rejection::Rejection;
pub use repository_identity::RepositoryIdentity;
use require_github::require_github;
use split_repository_path::split_repository_path;
use split_transport::split_transport;
pub use ssh_clone_url_form::SSH_CLONE_URL_FORM;
use ssh_user::SSH_USER;

#[cfg(test)]
#[path = "repository_test.rs"]
mod repository_test;
