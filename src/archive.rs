//! Image archiveの検証。
//!
//! `docker image save`が書いたarchiveが、buildして検証したimageそのものであることを、
//! Templateへloadする前に確かめる。archive全体を読まず、対応を判定できる最小限の
//! entryだけを取り出す。

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::error::{Error, ErrorId, Result};
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
            format!(
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
                    format!(
                        "the image in the archive declares {key}: {}, expected {expected}",
                        observed.map(String::as_str).unwrap_or("<absent>")
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
        return Err(unusable(path, format!("the archive has no {config_entry}")));
    };
    let document: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| unusable(path, format!("{config_entry} is not JSON: {error}")))?;

    // image configはOCIとDockerのどちらの表記でも`config`objectの下にlabelを持つ。
    let config = document
        .get("config")
        .or_else(|| document.get("Config"))
        .and_then(|value| value.as_object())
        .ok_or_else(|| unusable(path, format!("{config_entry} has no image configuration")))?;

    let declared = match config.get("Labels").or_else(|| config.get("labels")) {
        Some(serde_json::Value::Object(declared)) => declared.clone(),
        // labelを1つも持たないimageでは`null`になる。
        Some(serde_json::Value::Null) | None => serde_json::Map::new(),
        Some(_) => {
            return Err(unusable(
                path,
                format!("{config_entry} declares labels that are not an object"),
            ));
        }
    };

    let mut labels = std::collections::BTreeMap::new();
    for (key, value) in declared {
        let value = value.as_str().ok_or_else(|| {
            unusable(
                path,
                format!("label {key} in {config_entry} is not a string"),
            )
        })?;
        labels.insert(key, value.to_string());
    }
    Ok(labels)
}

/// archiveのmanifestを読む。
pub fn read_manifest(path: &Path) -> Result<ArchiveManifest> {
    let Some(bytes) = read_entry(path, MANIFEST_ENTRY)? else {
        return Err(unusable(
            path,
            format!("the archive has no {MANIFEST_ENTRY}"),
        ));
    };
    let document: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| unusable(path, format!("{MANIFEST_ENTRY} is not JSON: {error}")))?;
    let items = document
        .as_array()
        .ok_or_else(|| unusable(path, format!("{MANIFEST_ENTRY} is not an array")))?;
    let [item] = items.as_slice() else {
        return Err(unusable(
            path,
            format!("the archive holds {} images instead of one", items.len()),
        ));
    };

    let config = item
        .get("Config")
        .and_then(|value| value.as_str())
        .ok_or_else(|| unusable(path, format!("{MANIFEST_ENTRY} names no image config")))?;
    let digest = config_digest(config)
        .ok_or_else(|| unusable(path, format!("{config} is not an image config digest")))?;

    let repo_tags = match item.get("RepoTags") {
        Some(serde_json::Value::Array(tags)) => tags
            .iter()
            .map(|tag| {
                tag.as_str()
                    .map(|tag| tag.to_string())
                    .ok_or_else(|| unusable(path, "a repository tag is not a string".to_string()))
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
        .map_err(|error| unusable(path, format!("the archive could not be opened: {error}")))?;

    loop {
        let mut header = [0_u8; BLOCK];
        match file.read_exact(&mut header) {
            Ok(()) => {}
            // 末尾に達した。要求されたentryは存在しない。
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(error) => {
                return Err(unusable(
                    path,
                    format!("the archive could not be read: {error}"),
                ));
            }
        }
        // 終端はNULで埋めたblockが並ぶ。
        if header.iter().all(|byte| *byte == 0) {
            return Ok(None);
        }

        let name = entry_name(&header)
            .ok_or_else(|| unusable(path, "an entry has no readable name".to_string()))?;
        let size = octal(&header[124..136])
            .ok_or_else(|| unusable(path, format!("entry {name} has no readable size")))?;

        if name == wanted {
            if size > MAX_ENTRY_BYTES {
                return Err(unusable(
                    path,
                    format!("{name} is {size} bytes, which is larger than sbxm reads"),
                ));
            }
            let mut data = vec![0_u8; size as usize];
            file.read_exact(&mut data)
                .map_err(|error| unusable(path, format!("{name} could not be read: {error}")))?;
            return Ok(Some(data));
        }

        // entry本体は512 byte単位で詰められている。
        let padded = size.div_ceil(BLOCK as u64) * BLOCK as u64;
        file.seek(SeekFrom::Current(padded as i64))
            .map_err(|error| {
                unusable(path, format!("the archive could not be scanned: {error}"))
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

fn unusable(path: &Path, detail: String) -> Error {
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
mod tests {
    use super::*;
    use std::io::Write;

    const IMAGE_ID: &str =
        "sha256:1111111111111111111111111111111111111111111111111111111111111111";

    fn tar(entries: &[(&str, &[u8])]) -> Vec<u8> {
        tar_bytes(entries)
    }

    fn write_archive(entries: &[(&str, &[u8])]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("template.tar");
        let mut file = File::create(&path).unwrap();
        file.write_all(&tar(entries)).unwrap();
        file.sync_all().unwrap();
        (dir, path)
    }

    fn manifest(tag: &str, config: &str) -> String {
        format!(r#"[{{"Config":"{config}","RepoTags":["{tag}"],"Layers":[]}}]"#)
    }

    fn labels() -> Vec<(String, String)> {
        vec![
            (
                "io.crescware.sbxm.canonical-id".to_string(),
                "example-org/example-repo".to_string(),
            ),
            (
                "io.crescware.sbxm.dockerfile-sha256".to_string(),
                "3".repeat(64),
            ),
        ]
    }

    /// 期待するlabelを宣言するimage config blob。
    fn config_blob() -> Vec<u8> {
        let declared: String = labels()
            .iter()
            .map(|(key, value)| format!(r#""{key}":"{value}""#))
            .collect::<Vec<_>>()
            .join(",");
        format!(r#"{{"architecture":"arm64","config":{{"Labels":{{{declared}}}}}}}"#).into_bytes()
    }

    #[test]
    fn an_archive_that_holds_the_expected_image_is_accepted() {
        let hex = &IMAGE_ID["sha256:".len()..];
        for config in [
            format!("blobs/sha256/{hex}"),
            format!("{hex}.json"),
            format!("sha256:{hex}"),
        ] {
            let document = manifest("sbxm-example-template:111111111111", &config);
            let blob = config_blob();
            let (_dir, path) = write_archive(&[
                ("oci-layout", b"{}"),
                (config.as_str(), blob.as_slice()),
                (MANIFEST_ENTRY, document.as_bytes()),
            ]);
            verify_holds_image(&path, "sbxm-example-template:111111111111", &labels())
                .unwrap_or_else(|error| panic!("{config} must be accepted: {error:?}"));
        }
    }

    #[test]
    fn an_image_index_does_not_make_the_archive_foreign() {
        // buildがattestationを伴うOCI image indexを作る構成では、
        // `docker image inspect`のIdはindexのdigest、archiveが指すのはconfigの
        // digestになる。両者は別物であり、一致しないことが正常である。
        let config = "4".repeat(64);
        let index = format!("sha256:{}", "5".repeat(64));
        assert_ne!(format!("sha256:{config}"), index);

        let tag = "sbxm-example-template:111111111111";
        let document = manifest(tag, &format!("blobs/sha256/{config}"));
        let blob = config_blob();
        let (_dir, path) = write_archive(&[
            ("oci-layout", b"{}"),
            (&format!("blobs/sha256/{config}"), blob.as_slice()),
            (MANIFEST_ENTRY, document.as_bytes()),
        ]);

        verify_holds_image(&path, tag, &labels())
            .expect("the archive holds the image whose labels were verified");
    }

    #[test]
    fn an_entry_is_found_after_larger_entries_are_skipped() {
        let document = manifest(
            "sbxm-example-template:111111111111",
            &IMAGE_ID.replace("sha256:", "blobs/sha256/"),
        );
        let layer = vec![7_u8; BLOCK * 3 + 100];
        let (_dir, path) = write_archive(&[
            ("blobs/sha256/layer", layer.as_slice()),
            (MANIFEST_ENTRY, document.as_bytes()),
        ]);

        let read = read_manifest(&path).expect("the manifest is found behind the layers");
        assert_eq!(read.config_digest, IMAGE_ID);
    }

    #[test]
    fn an_archive_of_another_image_or_tag_is_refused() {
        let hex = &IMAGE_ID["sha256:".len()..];
        let document = manifest(
            "sbxm-other-template:222222222222",
            &format!("blobs/sha256/{hex}"),
        );
        let (_dir, path) = write_archive(&[(MANIFEST_ENTRY, document.as_bytes())]);
        let error = verify_holds_image(&path, "sbxm-example-template:111111111111", &labels())
            .expect_err("a different tag is refused");
        assert_eq!(error.first_id(), Some(ErrorId::ArchiveUnusable));

        // tagは同じでも、別の案件や別の世代を宣言するimageは受け付けない。
        let document = manifest(
            "sbxm-example-template:111111111111",
            &format!("blobs/sha256/{hex}"),
        );
        let foreign =
            br#"{"config":{"Labels":{"io.crescware.sbxm.canonical-id":"other-org/other-repo"}}}"#;
        let (_dir, path) = write_archive(&[
            (&format!("blobs/sha256/{hex}"), foreign.as_slice()),
            (MANIFEST_ENTRY, document.as_bytes()),
        ]);
        let error = verify_holds_image(&path, "sbxm-example-template:111111111111", &labels())
            .expect_err("a different image is refused");
        assert_eq!(error.first_id(), Some(ErrorId::ArchiveUnusable));
    }

    #[test]
    fn an_archive_that_cannot_be_interpreted_is_refused_rather_than_trusted() {
        let hex = &IMAGE_ID["sha256:".len()..];
        let cases: Vec<Vec<(&str, Vec<u8>)>> = vec![
            // manifestがない。
            vec![("oci-layout", b"{}".to_vec())],
            // manifestがJSONではない。
            vec![(MANIFEST_ENTRY, b"not json".to_vec())],
            // 2件のimageを含む。
            vec![(
                MANIFEST_ENTRY,
                format!(
                    r#"[{{"Config":"blobs/sha256/{hex}","RepoTags":["a"]}},{{"Config":"blobs/sha256/{hex}","RepoTags":["b"]}}]"#
                )
                .into_bytes(),
            )],
            // config digestを読めない。
            vec![(
                MANIFEST_ENTRY,
                br#"[{"Config":"blobs/sha256/short","RepoTags":["a"]}]"#.to_vec(),
            )],
        ];

        for entries in cases {
            let borrowed: Vec<(&str, &[u8])> = entries
                .iter()
                .map(|(name, data)| (*name, data.as_slice()))
                .collect();
            let (_dir, path) = write_archive(&borrowed);
            let error = verify_holds_image(&path, "sbxm-example-template:111111111111", &labels())
                .expect_err("an archive that cannot be interpreted is refused");
            assert_eq!(error.first_id(), Some(ErrorId::ArchiveUnusable));
        }
    }

    #[test]
    fn a_missing_archive_is_refused_with_the_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("absent.tar");
        let error = verify_holds_image(&path, "image", &labels()).expect_err("absent");
        assert_eq!(error.first_id(), Some(ErrorId::ArchiveUnusable));
    }
}
