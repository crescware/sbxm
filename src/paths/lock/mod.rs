//! OS file lockによる保護区間。

mod acquire_exclusive_lock;
mod exclusive_lock;

pub use acquire_exclusive_lock::acquire_exclusive_lock;
pub use exclusive_lock::ExclusiveLock;

#[cfg(test)]
#[path = "lock_test.rs"]
mod lock_test;
