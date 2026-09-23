use yaml_edit::Mapping;

use super::{declaration, edit_config};

/// 既存configの原文で、`files`の末尾へ宣言を1件足す。
///
/// yaml-editの木へmappingを足すと、block sequenceでは字下げや改行が崩れる。木は差し込む
/// 位置と字下げを決めるためだけに使い、宣言の原文はここで組み立てて差し込む。既存の
/// 宣言があればその`-`の書き方と字下げに揃え、flow sequenceなら同じ形で足す。どちらでも
/// ない書き方は扱わず`None`を返す。読み直した値が一致することは`edit_config`が確かめる。
pub(super) fn append_file_entry(text: &str, source: &str, destination: &str) -> Option<String> {
    edit_config(
        text,
        |_, root| {
            let (at, insertion) = insertion(text, root, source, destination)?;
            let mut updated = text.to_string();
            updated.insert_str(at, &insertion);
            Some(updated)
        },
        |expected| {
            let mut entry = yaml_serde::Mapping::new();
            entry.insert("source".into(), source.into());
            entry.insert("destination".into(), destination.into());
            match expected.get_mut("files") {
                Some(yaml_serde::Value::Sequence(files)) => files.push(entry.into()),
                _ => {
                    expected.insert("files".into(), vec![yaml_serde::Value::from(entry)].into());
                }
            }
        },
    )
}

/// 差し込む位置と原文。
fn insertion(
    text: &str,
    root: &Mapping,
    source: &str,
    destination: &str,
) -> Option<(usize, String)> {
    if root.is_flow_style() {
        return None;
    }
    let Some(entry) = root.find_entry_by_key("files") else {
        // `files`がまだ無い。top-levelの末尾へ、sbxmの文書と同じ字下げで足す。
        let lines = block_entry("  - ", "    ", source, destination)?;
        return Some(at_line_end(
            text,
            range_end(root)?,
            &format!("files:\n{lines}"),
        ));
    };
    let Some(files) = entry
        .value_node()
        .and_then(|value| value.as_sequence().cloned())
    else {
        // `files:`だけが書かれ、値が無い。keyの行の直後へ最初の宣言を足す。
        let key_end = usize::try_from(entry.key_node()?.as_scalar()?.byte_range().end).ok()?;
        let line_end = text[key_end..]
            .find('\n')
            .map_or(text.len(), |at| key_end + at);
        let rest = text[key_end..line_end].trim_start();
        let rest = rest.strip_prefix(':')?.trim_start();
        if !(rest.is_empty() || rest.starts_with('#')) {
            return None;
        }
        let lines = block_entry("  - ", "    ", source, destination)?;
        return Some((line_end, following_line(&lines)));
    };
    if files.is_flow_style() {
        let close = usize::try_from(files.byte_range().end)
            .ok()?
            .checked_sub(1)?;
        if text.as_bytes().get(close) != Some(&b']') {
            return None;
        }
        let item = format!(
            "{{source: {}, destination: {}}}",
            serde_json::to_string(source).ok()?,
            serde_json::to_string(destination).ok()?
        );
        let separator = if files.is_empty() { "" } else { ", " };
        return Some((close, format!("{separator}{item}")));
    }

    // 最後の宣言の行頭から、`-`とそれに続く空白をそのまま写す。
    let last = files.get(files.len().checked_sub(1)?)?;
    let last = last.as_mapping()?;
    let start = usize::try_from(last.byte_range().start).ok()?;
    let line_start = text[..start].rfind('\n').map_or(0, |at| at + 1);
    let marker = &text[line_start..start];
    let dash = marker.trim_start();
    if !(dash.starts_with('-') && dash[1..].chars().all(|c| c == ' ') && dash.len() > 1) {
        return None;
    }
    let indent = " ".repeat(marker.len());
    let lines = block_entry(marker, &indent, source, destination)?;
    Some(at_line_end(
        text,
        range_end(last)?,
        lines.trim_end_matches('\n'),
    ))
}

/// `source`と`destination`の2行。値の引用は`declaration`と同じくserializerが決める。
fn block_entry(first: &str, rest: &str, source: &str, destination: &str) -> Option<String> {
    Some(format!(
        "{first}{}\n{rest}{}\n",
        declaration("source", source).ok()?,
        declaration("destination", destination).ok()?
    ))
}

/// nodeが占める範囲の終わり。
fn range_end(node: &Mapping) -> Option<usize> {
    usize::try_from(node.byte_range().end).ok()
}

/// `end`より前にある最後の中身の行の終わりへ、`block`を独立した行として差し込む。
///
/// nodeの範囲は、末尾の改行を含むこともあれば、同じ行のcommentの手前で終わることもある。
/// どちらでも、その行を書き終えた位置へ足す。
fn at_line_end(text: &str, end: usize, block: &str) -> (usize, String) {
    let content_end = text[..end].trim_end().len();
    match text[content_end..].find('\n') {
        Some(at) => (content_end + at, following_line(block)),
        // 原文が改行で終わっていなくても、足した宣言は改行で終える。
        None => (text.len(), following_line(block) + "\n"),
    }
}

/// 直前の行を終える改行から始まる`block`。差し込む位置の行はまだ終わっていない。
fn following_line(block: &str) -> String {
    let mut line = String::from('\n');
    line.push_str(block.trim_end_matches('\n'));
    line
}
