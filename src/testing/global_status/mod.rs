//! `status --global`のtestが使うhost fake。
//!
//! 診断は外部commandの応答から決まるため、応答そのものをtestが組み立てる。

mod fake_host;
mod items;
mod location_with_config;
mod status_of;
mod valid_config;

pub use fake_host::FakeHost;
pub use items::items;
pub use location_with_config::location_with_config;
pub use status_of::status_of;
pub use valid_config::valid_config;
