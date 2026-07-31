//! Image archiveの検証。
//!
//! `docker image save`が書いたarchiveが、buildして検証したimageそのものであることを、
//! Templateへloadする前に確かめる。archive全体を読まず、対応を判定できる最小限の
//! entryだけを取り出す。

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::error::{Error, ErrorId, Result};
use crate::image_labels::{LabelDefect, labels_from_declared};
use crate::msg;
use crate::paths;

/// tarの1 blockの大きさ。
const BLOCK: usize = 512;

/// 読み込みを許すmetadata entryの上限。archive本体は読まない。
const MAX_ENTRY_BYTES: u64 = 1024 * 1024;

/// imageとarchiveの対応を宣言するentry。
const MANIFEST_ENTRY: &str = "manifest.json";

/// archiveが宣言するimageの同一性。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveManifest {
    /// archiveへ保存されたときのtag。
    pub repo_tags: Vec<String>,
    /// image configのdigest。archive内でconfigを指す名前でもある。
    ///
    /// `docker image inspect`の`Id`とは別物である。buildがOCI image indexを
    /// 作る構成では、`Id`はindexのdigestになり、この値と一致しない。
    pub config_digest: String,
    /// archive内でimage configを指すentry名。
    pub config_entry: String,
}

/// archiveが、指定したimageを1件だけ含むことを確認する。
///
/// 判定はarchiveが宣言するtagと、image configが持つlabelで行う。digestは
/// image storeとattestationの有無で意味が変わるため、対応の根拠にしない。
pub fn verify_holds_image(
    path: &Path,
    image_name: &str,
    expected_labels: &[(String, String)],
) -> Result<()> {
    let manifest = read_manifest(path)?;

    if !manifest.repo_tags.iter().any(|tag| tag == image_name) {
        return Err(unusable(
            path,
            &format!(
                "the archive holds {}, not {image_name}",
                manifest.repo_tags.join(", ")
            ),
        ));
    }

    let labels = read_config_labels(path, &manifest.config_entry)?;
    for (key, expected) in expected_labels {
        match labels.get(key) {
            Some(observed) if observed == expected => {}
            observed => {
                return Err(unusable(
                    path,
                    &format!(
                        "the image in the archive declares {key}: {}, expected {expected}",
                        observed.map_or("<absent>", String::as_str)
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// archiveへ保存されたimage configのlabel。
///
/// configはmanifestが名前で指すentryであり、archive本体のlayerは読まない。
fn read_config_labels(
    path: &Path,
    config_entry: &str,
) -> Result<std::collections::BTreeMap<String, String>> {
    let Some(bytes) = read_entry(path, config_entry)? else {
        return Err(unusable(
            path,
            &format!("the archive has no {config_entry}"),
        ));
    };
    let document: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| unusable(path, &format!("{config_entry} is not JSON: {error}")))?;

    // image configはOCIとDockerのどちらの表記でも`config`objectの下にlabelを持つ。
    let config = document
        .get("config")
        .or_else(|| document.get("Config"))
        .and_then(|value| value.as_object())
        .ok_or_else(|| unusable(path, &format!("{config_entry} has no image configuration")))?;

    let declared = config.get("Labels").or_else(|| config.get("labels"));
    labels_from_declared(declared).map_err(|defect| match defect {
        LabelDefect::NotAnObject => unusable(
            path,
            &format!("{config_entry} declares labels that are not an object"),
        ),
        LabelDefect::ValueNotAString(key) => unusable(
            path,
            &format!("label {key} in {config_entry} is not a string"),
        ),
    })
}

/// archiveのmanifestを読む。
pub fn read_manifest(path: &Path) -> Result<ArchiveManifest> {
    let Some(bytes) = read_entry(path, MANIFEST_ENTRY)? else {
        return Err(unusable(
            path,
            &format!("the archive has no {MANIFEST_ENTRY}"),
        ));
    };
    let document: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| unusable(path, &format!("{MANIFEST_ENTRY} is not JSON: {error}")))?;
    let items = document
        .as_array()
        .ok_or_else(|| unusable(path, &format!("{MANIFEST_ENTRY} is not an array")))?;
    let [item] = items.as_slice() else {
        return Err(unusable(
            path,
            &format!("the archive holds {} images instead of one", items.len()),
        ));
    };

    let config = item
        .get("Config")
        .and_then(|value| value.as_str())
        .ok_or_else(|| unusable(path, &format!("{MANIFEST_ENTRY} names no image config")))?;
    let digest = config_digest(config)
        .ok_or_else(|| unusable(path, &format!("{config} is not an image config digest")))?;

    let repo_tags = match item.get("RepoTags") {
        Some(serde_json::Value::Array(tags)) => tags
            .iter()
            .map(|tag| {
                tag.as_str()
                    .map(std::string::ToString::to_string)
                    .ok_or_else(|| unusable(path, "a repository tag is not a string"))
            })
            .collect::<Result<Vec<String>>>()?,
        // tagを持たないarchiveからは、どのimageを保存したかを判定できない。
        _ => Vec::new(),
    };

    Ok(ArchiveManifest {
        repo_tags,
        config_digest: digest,
        config_entry: config.to_string(),
    })
}

/// `blobs/sha256/<hex>`と`<hex>.json`のどちらの書き方でも、`sha256:<hex>`へ寄せる。
fn config_digest(config: &str) -> Option<String> {
    let name = config.rsplit('/').next()?;
    let hex = name.strip_suffix(".json").unwrap_or(name);
    let hex = hex.strip_prefix("sha256:").unwrap_or(hex);
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("sha256:{}", hex.to_ascii_lowercase()))
}

/// tar archiveから1件のentryを取り出す。
///
/// entry本体を読み飛ばしながらheaderだけを辿るため、archiveの大きさに依存しない。
fn read_entry(path: &Path, wanted: &str) -> Result<Option<Vec<u8>>> {
    let mut file = File::open(path)
        .map_err(|error| unusable(path, &format!("the archive could not be opened: {error}")))?;

    loop {
        let mut header = [0_u8; BLOCK];
        match file.read_exact(&mut header) {
            Ok(()) => {}
            // 末尾に達した。要求されたentryは存在しない。
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(error) => {
                return Err(unusable(
                    path,
                    &format!("the archive could not be read: {error}"),
                ));
            }
        }
        // 終端はNULで埋めたblockが並ぶ。
        if header.iter().all(|byte| *byte == 0) {
            return Ok(None);
        }

        let name =
            entry_name(&header).ok_or_else(|| unusable(path, "an entry has no readable name"))?;
        let size = octal(&header[124..136])
            .ok_or_else(|| unusable(path, &format!("entry {name} has no readable size")))?;

        if name == wanted {
            if size > MAX_ENTRY_BYTES {
                return Err(unusable(
                    path,
                    &format!("{name} is {size} bytes, which is larger than sbxm reads"),
                ));
            }
            let capacity = usize::try_from(size).map_err(|_| {
                unusable(
                    path,
                    &format!("{name} is {size} bytes, which is larger than sbxm reads"),
                )
            })?;
            let mut data = vec![0_u8; capacity];
            file.read_exact(&mut data)
                .map_err(|error| unusable(path, &format!("{name} could not be read: {error}")))?;
            return Ok(Some(data));
        }

        // entry本体は512 byte単位で詰められている。
        let padded = size.div_ceil(BLOCK as u64) * BLOCK as u64;
        let padded = i64::try_from(padded)
            .map_err(|_| unusable(path, &format!("entry {name} declares an unusable size")))?;
        file.seek(SeekFrom::Current(padded)).map_err(|error| {
            unusable(path, &format!("the archive could not be scanned: {error}"))
        })?;
    }
}

/// ustar headerのnameとprefixからentry名を組み立てる。
fn entry_name(header: &[u8; BLOCK]) -> Option<String> {
    let name = trimmed(&header[0..100])?;
    let prefix = trimmed(&header[345..500])?;
    if prefix.is_empty() {
        Some(name)
    } else {
        Some(format!("{prefix}/{name}"))
    }
}

fn trimmed(field: &[u8]) -> Option<String> {
    let end = field
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(field.len());
    std::str::from_utf8(&field[..end])
        .ok()
        .map(|value| value.trim().to_string())
}

fn octal(field: &[u8]) -> Option<u64> {
    let text = trimmed(field)?;
    if text.is_empty() {
        return Some(0);
    }
    u64::from_str_radix(&text, 8).ok()
}

fn unusable(path: &Path, detail: &str) -> Error {
    Error::new(
        ErrorId::ArchiveUnusable,
        msg!(
            "error-archive-unusable",
            path = paths::display(path),
            detail = detail
        ),
    )
}

/// 検証したい形のarchiveを組み立てる、最小限のtar writer。
///
/// 外部commandが書くarchiveを、testの中で再現するために使う。
#[cfg(test)]
pub fn tar_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    for (name, data) in entries {
        let mut header = [0_u8; BLOCK];
        let name_bytes = name.as_bytes();
        header[..name_bytes.len()].copy_from_slice(name_bytes);
        let size = format!("{:011o}\0", data.len());
        header[124..124 + size.len()].copy_from_slice(size.as_bytes());
        header[156] = b'0';
        header[257..262].copy_from_slice(b"ustar");
        out.extend_from_slice(&header);
        out.extend_from_slice(data);
        let padding = (BLOCK - data.len() % BLOCK) % BLOCK;
        out.extend(std::iter::repeat_n(0_u8, padding));
    }
    out.extend(std::iter::repeat_n(0_u8, BLOCK * 2));
    out
}

/// `manifest.json`の内容。
#[cfg(test)]
pub fn manifest_json(image_name: &str, image_id: &str) -> String {
    let hex = image_id.strip_prefix("sha256:").unwrap_or(image_id);
    format!(r#"[{{"Config":"blobs/sha256/{hex}","RepoTags":["{image_name}"],"Layers":[]}}]"#)
}

#[cfg(test)]
#[path = "archive_test.rs"]
mod archive_test;
