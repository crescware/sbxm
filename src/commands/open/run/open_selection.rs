use crate::config::ConfigLocation;
use crate::diagnostics::Result;
use crate::msg;
use crate::project::ProjectId;
use crate::support::select::{self, Candidate, ProjectPrompt};

/// 修復の後にも同じ案件とindexで準備をやり直すための選択。
#[derive(Clone)]
pub struct OpenSelection {
    pub candidate: Candidate,
    pub index: Option<u32>,
}

impl OpenSelection {
    pub fn resolve(
        location: &ConfigLocation,
        requested: Option<&ProjectId>,
        index: Option<u32>,
        prompt: &mut dyn ProjectPrompt,
    ) -> Result<Self> {
        let (candidate, index) = if requested.is_none() && index.is_none() {
            let (candidate, index) = select::open(location, &msg!("select-open-heading"), prompt)?;
            (candidate, Some(index))
        } else {
            (
                select::one(location, requested, &msg!("select-open-heading"), prompt)?,
                index,
            )
        };
        Ok(Self { candidate, index })
    }
}
