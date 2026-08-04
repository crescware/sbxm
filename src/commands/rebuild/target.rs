use crate::config::ConfigLocation;
use crate::project::ProjectId;
use crate::support::select::ProjectPrompt;

/// `select::one`が対象を解決するために要るもの。
pub struct Target<'a> {
    pub location: &'a ConfigLocation,
    pub requested: Option<&'a ProjectId>,
    pub prompt: &'a mut dyn ProjectPrompt,
}
