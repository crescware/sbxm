use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;
use crate::hash::sha256_hex;
use crate::support::retrieve;

use super::{CREATE_BUNDLE, MAX_BUNDLE_BYTES, REF_KINDS, Receipt, ReceivedBundle};

/// Sandboxが、hostに保存済みのものと同じだったときに書く答え。
const UNCHANGED: &[u8] = b"unchanged\n";

/// Sandboxのbare repositoryを、hostの`directory`へbundleとして受け取る。
///
/// file名は`stamp`から作る。同じ名前が既にあれば番号を足し、既存のbundleを上書き
/// しない。`saved`はhostに保存済みの種類ごとのrefであり、Sandboxのrefがこれと同じなら
/// bundleを運ばない。
pub fn receive_bundle(
    host: &dyn HostEnvironment,
    sandbox: &str,
    git_dir: &str,
    directory: &Path,
    stamp: &str,
    saved: &BTreeMap<String, String>,
) -> Result<Receipt> {
    let mut label = stamp.to_string();
    let mut attempt = 1;
    while directory.join(format!("{label}.bundle")).exists() {
        attempt += 1;
        label = format!("{stamp}-{attempt}");
    }
    let known = listing_digest(saved);
    let path = retrieve::receive(
        host,
        sandbox,
        &["sh", "-c", CREATE_BUNDLE, "sh", git_dir, &known],
        directory,
        &format!("{label}.bundle"),
        MAX_BUNDLE_BYTES,
        TimeoutClass::RepositoryTransfer,
    )?;
    // bundleでない答えは、受け取ったfileとして残さない。bundleは先頭だけを読む。
    let mut start = Vec::new();
    let _ = File::open(&path).and_then(|file| file.take(16).read_to_end(&mut start));
    let receipt = if start.is_empty() {
        Receipt::Nothing
    } else if start == UNCHANGED {
        Receipt::Unchanged
    } else {
        return Ok(Receipt::Bundle(ReceivedBundle { path, label }));
    };
    let _ = fs::remove_file(&path);
    Ok(receipt)
}

/// hostに保存済みのrefを、Sandboxでの名前に戻して並べた一覧のdigest。
///
/// Sandboxの`git for-each-ref`と同じく、ref名のbyte順に`<object名> <ref名>`の行を並べる。
/// 保存済みのrefが無ければ、どの一覧とも一致しない空の文字列を返す。
fn listing_digest(saved: &BTreeMap<String, String>) -> String {
    if saved.is_empty() {
        return String::new();
    }
    let mut listed = BTreeMap::new();
    for (name, tip) in saved {
        for (source, kind) in REF_KINDS {
            if let Some(rest) = name.strip_prefix(kind) {
                listed.insert(format!("{source}{rest}"), tip);
            }
        }
    }
    let lines: Vec<String> = listed
        .into_iter()
        .map(|(reference, tip)| format!("{tip} {reference}\n"))
        .collect();
    sha256_hex(lines.concat().as_bytes())
}
