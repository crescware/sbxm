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
    /// image configのdigest。`docker image inspect`のIdと同じ値になる。
    pub config_digest: String,
}

/// archiveが、指定したimageを1件だけ含むことを確認する。
pub fn verify_holds_image(path: &Path, image_name: &str, image_id: &str) -> Result<()> {
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
    if manifest.config_digest != image_id {
        return Err(unusable(
            path,
            format!(
                "the archive holds image {}, not {image_id}",
                manifest.config_digest
            ),
        ));
    }
    Ok(())
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

    #[test]
    fn an_archive_that_holds_the_expected_image_is_accepted() {
        let hex = &IMAGE_ID["sha256:".len()..];
        for config in [
            format!("blobs/sha256/{hex}"),
            format!("{hex}.json"),
            format!("sha256:{hex}"),
        ] {
            let document = manifest("sbxm-example-template:111111111111", &config);
            let (_dir, path) =
                write_archive(&[("oci-layout", b"{}"), (MANIFEST_ENTRY, document.as_bytes())]);
            verify_holds_image(&path, "sbxm-example-template:111111111111", IMAGE_ID)
                .unwrap_or_else(|error| panic!("{config} must be accepted: {error:?}"));
        }
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
        let error = verify_holds_image(&path, "sbxm-example-template:111111111111", IMAGE_ID)
            .expect_err("a different tag is refused");
        assert_eq!(error.first_id(), Some(ErrorId::ArchiveUnusable));

        let other = "2".repeat(64);
        let document = manifest(
            "sbxm-example-template:111111111111",
            &format!("blobs/sha256/{other}"),
        );
        let (_dir, path) = write_archive(&[(MANIFEST_ENTRY, document.as_bytes())]);
        let error = verify_holds_image(&path, "sbxm-example-template:111111111111", IMAGE_ID)
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
            let error = verify_holds_image(&path, "sbxm-example-template:111111111111", IMAGE_ID)
                .expect_err("an archive that cannot be interpreted is refused");
            assert_eq!(error.first_id(), Some(ErrorId::ArchiveUnusable));
        }
    }

    #[test]
    fn a_missing_archive_is_refused_with_the_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("absent.tar");
        let error = verify_holds_image(&path, "image", IMAGE_ID).expect_err("absent");
        assert_eq!(error.first_id(), Some(ErrorId::ArchiveUnusable));
    }
}
