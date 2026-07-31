//! 案件を指す位置引数。
//!
//! 対象の省略を許すcommandは、非TTYで省略された場合に外部状態を読む前に終了する。

mod clone_url_value_name;
mod optional_project;
mod project_value_name;
mod require_prompt_capability;
mod required_clone_url;
mod required_project;

pub use clone_url_value_name::CLONE_URL_VALUE_NAME;
pub use optional_project::optional_project;
pub use project_value_name::PROJECT_VALUE_NAME;
pub use require_prompt_capability::require_prompt_capability;
pub use required_clone_url::required_clone_url;
pub use required_project::required_project;
