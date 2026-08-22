use crate::command::HostEnvironment;
use crate::config::{self, GlobalConfig};
use crate::design::Remediation;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::GitIdentity;
use crate::msg;

use crate::commands::Context;
use crate::commands::add::Args;

use super::{IdentityPrompt, ask_git_identity};

/// この案件の名義を決め、必要なら一度だけ利用者へ訊く。
///
/// 宣言があればそれを使い、無ければ保存済みの既定を使う。どちらも無い場合にだけ訊き、
/// 訊けなければ、hostの値で代用せずmutationの前に拒否する。
///
/// `--git-user-name`と`--git-user-email`はそのprocessだけのoverrideであり、`--lang`と
/// 同じく、永続設定を利用者自身が選ぶという契約と混同しないため保存しない。
pub fn choose_git_identity(
    context: &Context,
    config: &GlobalConfig,
    args: &Args,
    host: &dyn HostEnvironment,
    prompt: &mut dyn IdentityPrompt,
) -> Result<GitIdentity> {
    if let Some(declared) = &args.git_identity {
        return Ok(declared.clone());
    }
    if let Some(saved) = &config.git_identity {
        return Ok(saved.clone());
    }
    if !context.can_prompt {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::GitIdentityUndecidable,
                msg!("error-git-identity-undecidable"),
            )
            .remediation(
                Remediation::text(msg!("remediation-git-identity-undecidable")).try_run(
                    "sbxm add <clone-url> --git-user-name <name> --git-user-email <email>",
                ),
            ),
        ));
    }

    let chosen = ask_git_identity(prompt, host)?;
    // 名義の保存はproject mutationではなく、利用者が明示的に選んだ独立した設定変更で
    // ある。以降のvalidationが失敗しても、選んだ名義をrollbackしない。
    config::save_git_identity(context.location, &chosen)?;
    Ok(chosen)
}
