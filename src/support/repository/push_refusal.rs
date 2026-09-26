/// hostからSandboxへのpushで、gitが断ったref。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushRefusal {
    /// Sandboxのref。
    pub reference: String,
    /// gitが示した理由。
    pub reason: String,
}
