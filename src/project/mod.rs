//! Project識別子。
//!
//! stringを直接workflowへ渡さず、validation済みの型を使う。案件の突き合わせには
//! ASCII lowercaseのcanonical形式を使い、表示にはGitHub上の表記を使う。

mod canonical_project_id;
mod project_id;
mod sandbox_layout;
mod sandbox_name;
mod sandbox_name_max_bytes;

pub use canonical_project_id::CanonicalProjectId;
pub use project_id::ProjectId;
pub use sandbox_layout::SandboxLayout;
pub use sandbox_name::SandboxName;
use sandbox_name_max_bytes::SANDBOX_NAME_MAX_BYTES;

#[cfg(test)]
#[path = "project_test.rs"]
mod project_test;
