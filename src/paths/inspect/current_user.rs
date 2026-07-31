/// このprocessの実効user ID。
///
/// permissionだけでは、ほかのaccountが所有する`0700`のdirectoryを自分のものと
/// 区別できない。所有関係は観測した値で判定する。
pub fn current_user() -> u32 {
    rustix::process::geteuid().as_raw()
}
