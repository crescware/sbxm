//! OS file lockによる保護区間。

mod acquire_exclusive_lock;
mod acquire_lock;
mod acquire_shared_lock;
mod exclusive_lock;
mod shared_lock;

pub use acquire_exclusive_lock::acquire_exclusive_lock;
pub use acquire_shared_lock::acquire_shared_lock;
pub use exclusive_lock::ExclusiveLock;
pub use shared_lock::SharedLock;

#[cfg(test)]
#[path = "lock_test.rs"]
mod lock_test;

#[cfg(test)]
#[path = "shared_lock_test.rs"]
mod shared_lock_test;
