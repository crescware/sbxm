//! platformの診断。

mod check_platform;
mod expected_architecture;
mod expected_platform;
mod minimum_macos_major;

pub(super) use check_platform::check_platform;
pub(super) use expected_architecture::EXPECTED_ARCHITECTURE;
pub(super) use expected_platform::EXPECTED_PLATFORM;
pub(super) use minimum_macos_major::MINIMUM_MACOS_MAJOR;
