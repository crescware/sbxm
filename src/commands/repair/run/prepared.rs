use crate::design::Warning;
use crate::paths::{ExclusiveLock, ProjectPaths};

use crate::support::provisioning::{ExternalPreconditions, Observation};
use crate::support::select;

use super::RepairPlan;

/// project lockと、repair可能な場合のexclusive session leaseを保持する状態。
#[derive(Debug)]
pub struct Prepared {
    pub plan: RepairPlan,
    pub(super) paths: ProjectPaths,
    pub(super) locked: select::Locked,
    pub(super) observation: Observation,
    pub(super) target: String,
    pub(super) preconditions: Option<ExternalPreconditions>,
    pub(super) session_lease: Option<ExclusiveLock>,
    pub(super) warnings: Vec<Warning>,
}
