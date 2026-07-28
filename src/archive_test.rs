use super::*;
use std::io::Write;

const IMAGE_ID: &str = "sha256:1111111111111111111111111111111111111111111111111111111111111111";

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
