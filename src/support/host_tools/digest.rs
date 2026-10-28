use crate::boundary::host::ExecutableIdentity;
use crate::boundary::host::protocol::CliVersion;
use crate::hash::sha256_hex;
use crate::paths;

/// 記録の内容のSHA-256。
///
/// program名、識別情報、observedをこの順に`\0`で区切って並べる。YAMLの描画形式には依存
/// させない。program名を含めるため、ある道具の記録を別の道具へ流用できない。
///
/// digestが検出するのは、意図しない書き換えと、digestを計算し直さない改変である。
/// digestも計算し直された改変は検出できない。
pub(super) fn digest(program: &str, identity: &ExecutableIdentity, observed: CliVersion) -> String {
    let fields = [
        program.to_string(),
        paths::display(&identity.path),
        identity.device.to_string(),
        identity.inode.to_string(),
        identity.size.to_string(),
        identity.mtime_secs.to_string(),
        identity.mtime_nanos.to_string(),
        identity.ctime_secs.to_string(),
        identity.ctime_nanos.to_string(),
        observed.to_string(),
    ];
    sha256_hex(fields.join("\0").as_bytes())
}
