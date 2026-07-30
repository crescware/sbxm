//! `add`の出力。
//!
//! 登録とhost cloneまでを示し、次にやることを続けて出す。GitHub tokenの登録先は
//! Sandbox名であり、その名前はここで確定する。
//!
//! 実行を求めるcommandは説明文へ混ぜず、独立blockとして渡す。番号は説明行に付き、
//! command行には付かない。

use crate::msg;
use crate::paths;
use crate::support::secret;
use crate::ui::{Document, Field, GuidanceItem, Inline};

use super::super::present;
use super::run::AddOutput;

/// `add`が並べるもの。
pub fn document(output: &AddOutput) -> Document {
    let summary = if output.already_registered {
        msg!("add-already-registered", project = output.project)
    } else {
        msg!("add-registered", project = output.project)
    };

    Document::new()
        .summary(summary)
        .fields(
            None,
            vec![
                Field::new(
                    msg!("add-field-project"),
                    Inline::important(output.project.clone()),
                ),
                Field::new(
                    msg!("add-field-sandbox"),
                    Inline::important(output.sandbox.clone()),
                ),
                Field::new(
                    msg!("add-field-creation-mode"),
                    present::creation_mode(output.mode),
                ),
                Field::new(
                    msg!("add-field-start-branch"),
                    Inline::text(output.start_ref.clone().unwrap_or_else(|| "-".to_string())),
                ),
                Field::new(
                    msg!("add-field-managed-worktrees"),
                    Inline::text(output.requested_worktrees.to_string()),
                ),
                Field::new(
                    msg!("add-field-host-clone"),
                    Inline::path(paths::display(&output.host_clone)),
                ),
            ],
        )
        // tokenの権限は、失敗したときだけでなくここでも示す。暗記させない。
        .guidance(
            Some(msg!("add-next-heading")),
            vec![
                GuidanceItem::Ordered {
                    number: 1,
                    text: msg!("add-next-secret"),
                },
                GuidanceItem::Plain(msg!("add-next-token")),
            ],
        )
        .try_command(secret::register_command(&output.sandbox, None))
        .guidance(
            None,
            vec![GuidanceItem::Ordered {
                number: 2,
                text: msg!("add-next-prepare"),
            }],
        )
        .try_command(format!("sbxm prepare {}", output.project))
}
