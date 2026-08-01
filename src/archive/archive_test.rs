use crate::diagnostics::ErrorId;
use std::fs::File;

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::testing::value::IMAGE_ID;
use std::io::Write;

fn tar(entries: &[(&str, &[u8])]) -> Vec<u8> {
    tar_bytes(entries)
}

fn write_archive(entries: &[(&str, &[u8])]) -> Checked<(tempfile::TempDir, std::path::PathBuf)> {
    let dir = tempfile::tempdir().required()?;
    let path = dir.path().join("template.tar");
    let mut file = File::create(&path).required()?;
    file.write_all(&tar(entries)).required()?;
    file.sync_all().required()?;
    Ok((dir, path))
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
fn an_archive_that_holds_the_expected_image_is_accepted() -> Checked {
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
        ])?;
        verify_holds_image(&path, "sbxm-example-template:111111111111", &labels())
            .required_because(&format!("{config} must be accepted"))?;
    }
    Ok(())
}

#[test]
fn an_image_index_does_not_make_the_archive_foreign() -> Checked {
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
    ])?;

    verify_holds_image(&path, tag, &labels())
        .required_because("the archive holds the image whose labels were verified")?;
    Ok(())
}

#[test]
fn an_entry_is_found_after_larger_entries_are_skipped() -> Checked {
    let document = manifest(
        "sbxm-example-template:111111111111",
        &IMAGE_ID.replace("sha256:", "blobs/sha256/"),
    );
    let layer = vec![7_u8; BLOCK * 3 + 100];
    let (_dir, path) = write_archive(&[
        ("blobs/sha256/layer", layer.as_slice()),
        (MANIFEST_ENTRY, document.as_bytes()),
    ])?;

    let read = read_manifest(&path).required_because("the manifest is found behind the layers")?;
    assert_eq!(read.config_digest, IMAGE_ID);
    Ok(())
}

#[test]
fn an_archive_of_another_image_or_tag_is_refused() -> Checked {
    let hex = &IMAGE_ID["sha256:".len()..];
    let document = manifest(
        "sbxm-other-template:222222222222",
        &format!("blobs/sha256/{hex}"),
    );
    let (_dir, path) = write_archive(&[(MANIFEST_ENTRY, document.as_bytes())])?;
    let error = verify_holds_image(&path, "sbxm-example-template:111111111111", &labels())
        .refused_because("a different tag is refused")?;
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
    ])?;
    let error = verify_holds_image(&path, "sbxm-example-template:111111111111", &labels())
        .refused_because("a different image is refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::ArchiveUnusable));
    Ok(())
}

#[test]
fn an_archive_that_cannot_be_interpreted_is_refused_rather_than_trusted() -> Checked {
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
        let (_dir, path) = write_archive(&borrowed)?;
        let error = verify_holds_image(&path, "sbxm-example-template:111111111111", &labels())
            .refused_because("an archive that cannot be interpreted is refused")?;
        assert_eq!(error.first_id(), Some(ErrorId::ArchiveUnusable));
    }
    Ok(())
}

/// entry本体を伴わないustar headerを1つ書く。
///
/// `tar_bytes`は整合の取れたarchiveしか作れない。宣言と中身が食い違う実物を
/// 再現するため、headerだけを組み立てる。
fn header(name: &[u8], prefix: &[u8], size: &[u8]) -> Vec<u8> {
    let mut block = [0_u8; BLOCK];
    block[..name.len()].copy_from_slice(name);
    block[124..124 + size.len()].copy_from_slice(size);
    block[156] = b'0';
    block[257..262].copy_from_slice(b"ustar");
    block[345..345 + prefix.len()].copy_from_slice(prefix);
    block.to_vec()
}

/// 生のbyte列をarchiveとして書き出す。
fn write_bytes(bytes: &[u8]) -> Checked<(tempfile::TempDir, std::path::PathBuf)> {
    let dir = tempfile::tempdir().required()?;
    let path = dir.path().join("template.tar");
    let mut file = File::create(&path).required()?;
    file.write_all(bytes).required()?;
    file.sync_all().required()?;
    Ok((dir, path))
}

#[test]
fn an_entry_name_may_be_split_across_the_ustar_prefix() -> Checked {
    // 100 byteを超える名前は、prefixとnameへ分けて記録される。両方を読んで元へ戻す。
    let hex = &IMAGE_ID["sha256:".len()..];
    let document = manifest(
        "sbxm-example-template:111111111111",
        &format!("blobs/sha256/{hex}"),
    );
    let mut bytes = header(b"manifest.json", b"", b"00000000000");
    bytes[124..124 + 11].copy_from_slice(format!("{:011o}", document.len()).as_bytes());
    bytes.extend_from_slice(document.as_bytes());
    bytes.extend(std::iter::repeat_n(
        0_u8,
        (BLOCK - document.len() % BLOCK) % BLOCK,
    ));
    // 名前を分けて記録したconfig blob。
    let blob = config_blob();
    let mut split = header(hex.as_bytes(), b"blobs/sha256", b"00000000000");
    split[124..124 + 11].copy_from_slice(format!("{:011o}", blob.len()).as_bytes());
    bytes.extend_from_slice(&split);
    bytes.extend_from_slice(&blob);
    bytes.extend(std::iter::repeat_n(
        0_u8,
        (BLOCK - blob.len() % BLOCK) % BLOCK,
    ));
    bytes.extend(std::iter::repeat_n(0_u8, BLOCK * 2));

    let (_dir, path) = write_bytes(&bytes)?;
    verify_holds_image(&path, "sbxm-example-template:111111111111", &labels())
        .required_because("the split name names the same entry")?;
    Ok(())
}

#[test]
fn an_archive_that_stops_before_its_own_end_holds_nothing_more() -> Checked {
    // 途中で切れたarchiveから、要求したentryが在ると推測しない。
    for bytes in [
        b"short".to_vec(),
        header(b"oci-layout", b"", b"00000000002"),
    ] {
        let (_dir, path) = write_bytes(&bytes)?;
        let error = read_manifest(&path).refused_because("the archive ends early")?;
        assert_eq!(error.first_id(), Some(ErrorId::ArchiveUnusable));
    }
    Ok(())
}

#[test]
fn a_header_that_cannot_be_read_stops_the_scan() -> Checked {
    let readable_size = format!("{:011o}", 4_u64);
    let cases: Vec<Vec<u8>> = vec![
        // 名前がUTF-8ではない。
        header(&[0xff, 0xfe], b"", readable_size.as_bytes()),
        // 大きさが8進数ではない。
        header(b"manifest.json", b"", b"not-octal--"),
        // metadataとして読む上限を超える大きさを宣言している。
        header(b"manifest.json", b"", b"77777777777"),
        // 宣言した大きさのentry本体が続いていない。
        header(b"manifest.json", b"", readable_size.as_bytes()),
    ];
    for bytes in cases {
        let (_dir, path) = write_bytes(&bytes)?;
        let error = read_manifest(&path).refused_because("the header is not usable")?;
        assert_eq!(error.first_id(), Some(ErrorId::ArchiveUnusable));
    }
    Ok(())
}

#[test]
fn an_entry_that_declares_no_size_is_read_as_empty() -> Checked {
    // 大きさの欄が空のheaderは0 byteのentryを指す。0 byteのmanifestはJSONではない。
    let mut bytes = header(b"manifest.json", b"", b"");
    bytes.extend(std::iter::repeat_n(0_u8, BLOCK * 2));
    let (_dir, path) = write_bytes(&bytes)?;
    let error = read_manifest(&path).refused_because("an empty manifest is not JSON")?;
    assert_eq!(error.first_id(), Some(ErrorId::ArchiveUnusable));
    Ok(())
}

#[test]
fn a_manifest_that_does_not_describe_one_image_is_refused() -> Checked {
    let hex = &IMAGE_ID["sha256:".len()..];
    let cases: Vec<&[u8]> = vec![
        // 配列ではない。
        br#"{"Config":"blobs/sha256/x"}"#,
        // image configを名前で指していない。
        br#"[{"RepoTags":["a"]}]"#,
        // tagが文字列ではない。
        br#"[{"Config":"blobs/sha256/short","RepoTags":[1]}]"#,
    ];
    for document in cases {
        let (_dir, path) = write_archive(&[(MANIFEST_ENTRY, document)])?;
        let error = read_manifest(&path).refused_because("the manifest is not usable")?;
        assert_eq!(error.first_id(), Some(ErrorId::ArchiveUnusable));
    }

    // tagを持たないarchiveからは、どのimageを保存したかを判定できない。
    let document = format!(r#"[{{"Config":"blobs/sha256/{hex}"}}]"#);
    let (_dir, path) = write_archive(&[(MANIFEST_ENTRY, document.as_bytes())])?;
    assert_eq!(
        read_manifest(&path)
            .required_because("a manifest without tags still names its config")?
            .repo_tags,
        Vec::<String>::new()
    );
    let error = verify_holds_image(&path, "sbxm-example-template:111111111111", &labels())
        .refused_because("an untagged archive cannot be matched to an image")?;
    assert_eq!(error.first_id(), Some(ErrorId::ArchiveUnusable));
    Ok(())
}

#[test]
fn the_image_configuration_is_read_in_either_spelling() -> Checked {
    // OCIとDockerでkeyの綴りが違う。どちらでも同じlabelを読む。
    let hex = &IMAGE_ID["sha256:".len()..];
    let entry = format!("blobs/sha256/{hex}");
    let declared: String = labels()
        .iter()
        .map(|(key, value)| format!(r#""{key}":"{value}""#))
        .collect::<Vec<_>>()
        .join(",");
    let document = manifest("sbxm-example-template:111111111111", &entry);
    let blob = format!(r#"{{"Config":{{"labels":{{{declared}}}}}}}"#);
    let (_dir, path) = write_archive(&[
        (entry.as_str(), blob.as_bytes()),
        (MANIFEST_ENTRY, document.as_bytes()),
    ])?;
    verify_holds_image(&path, "sbxm-example-template:111111111111", &labels())
        .required_because("the other spelling names the same configuration")?;
    Ok(())
}

#[test]
fn an_image_configuration_that_cannot_be_read_is_refused() -> Checked {
    let hex = &IMAGE_ID["sha256:".len()..];
    let entry = format!("blobs/sha256/{hex}");
    let document = manifest("sbxm-example-template:111111111111", &entry);

    // manifestが指すconfigがarchiveに無い。
    let (_dir, path) = write_archive(&[(MANIFEST_ENTRY, document.as_bytes())])?;
    let error = verify_holds_image(&path, "sbxm-example-template:111111111111", &labels())
        .refused_because("the configuration the manifest names is absent")?;
    assert_eq!(error.first_id(), Some(ErrorId::ArchiveUnusable));

    for blob in [
        // JSONではない。
        "not json",
        // image configurationを持たない。
        r#"{"architecture":"arm64"}"#,
        // labelがobjectではない。
        r#"{"config":{"Labels":"none"}}"#,
        // labelの値が文字列ではない。
        r#"{"config":{"Labels":{"io.crescware.sbxm.canonical-id":1}}}"#,
        // labelを1つも持たない。
        r#"{"config":{}}"#,
    ] {
        let (_dir, path) = write_archive(&[
            (entry.as_str(), blob.as_bytes()),
            (MANIFEST_ENTRY, document.as_bytes()),
        ])?;
        let error = verify_holds_image(&path, "sbxm-example-template:111111111111", &labels())
            .refused_because("the configuration is not usable")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::ArchiveUnusable),
            "{blob} produced the wrong error"
        );
    }
    Ok(())
}

#[test]
fn a_missing_archive_is_refused_with_the_path() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let path = dir.path().join("absent.tar");
    let error = verify_holds_image(&path, "image", &labels()).refused_because("absent")?;
    assert_eq!(error.first_id(), Some(ErrorId::ArchiveUnusable));
    Ok(())
}
