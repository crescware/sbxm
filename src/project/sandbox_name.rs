use crate::hash::{SHORT_HEX_LENGTH, sha256_hex};

use super::{CanonicalProjectId, SANDBOX_NAME_MAX_BYTES};

/// Sandbox名の固定接頭辞。
const SANDBOX_NAME_PREFIX: &str = "sbxm-";

/// canonical project `IDから決定的に導出したSandbox名`。
///
/// 同じcanonical project IDは常に同じ名前となり、異なるIDは通常hashで区別する。
/// hash prefixの理論上の衝突は、案件一覧を突き合わせる側がname collisionとして扱う。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SandboxName {
    value: String,
}

impl SandboxName {
    /// 1. `/`を`-`へ変え、`[a-z0-9-]`以外を`-`へ置換する
    /// 2. 連続する`-`を1個へ畳み、前後の`-`を除く
    /// 3. canonical project IDのSHA-256先頭12桁を求める
    /// 4. `sbxm-<slug>-<hash>`が63 byte以内になるようslugの末尾を切る
    pub fn derive(id: &CanonicalProjectId) -> SandboxName {
        let hash = sha256_hex(id.as_str().as_bytes());
        let hash = &hash[..SHORT_HEX_LENGTH];
        let budget = SANDBOX_NAME_MAX_BYTES - SANDBOX_NAME_PREFIX.len() - 1 - SHORT_HEX_LENGTH;

        // slugifyの出力は`[a-z0-9-]`だけであり、byte境界とchar境界が一致する。
        let mut slug = slugify(id.as_str());
        slug.truncate(budget);
        while slug.ends_with('-') {
            slug.pop();
        }

        let value = if slug.is_empty() {
            format!("{SANDBOX_NAME_PREFIX}{hash}")
        } else {
            format!("{SANDBOX_NAME_PREFIX}{slug}-{hash}")
        };
        SandboxName { value }
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }
}

impl std::fmt::Display for SandboxName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.value)
    }
}

/// `[a-z0-9-]`だけの文字列へ落とし、連続する`-`を1個へ畳む。
fn slugify(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        let mapped = match byte {
            b'a'..=b'z' | b'0'..=b'9' => byte as char,
            _ => '-',
        };
        if mapped == '-' && out.ends_with('-') {
            continue;
        }
        out.push(mapped);
    }
    out.trim_matches('-').to_string()
}
