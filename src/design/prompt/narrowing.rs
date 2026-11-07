use std::ops::Range;

use super::Filter;

/// 絞り込み文字列と、それに一致する候補。端末を持たない。
///
/// 一致は表示labelの連続した部分文字列であり、大文字小文字を区別しない。飛び石の一致は
/// 採らない。何が一致したかを利用者が説明できる規則だけを使う。候補の並び順は変えず、
/// 一致しない候補を隠すだけである。
///
/// 現在位置は行ではなく候補に付く。ここでは候補indexを受け取り、一致の中で動かすか、
/// 絞り込みの変更後も見える候補へ置き直す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Narrowing {
    labels: Vec<String>,
    query: String,
    filter: Filter,
}

impl Narrowing {
    pub fn new(labels: &[String], filter: Filter) -> Narrowing {
        Narrowing {
            labels: labels.to_vec(),
            query: String::new(),
            filter,
        }
    }

    pub fn filter(&self) -> Filter {
        self.filter
    }

    /// 文字キーを受け付けるか。
    pub fn is_typed(&self) -> bool {
        self.filter == Filter::Typed
    }

    /// 打たれた絞り込み文字列。
    pub fn query(&self) -> &str {
        &self.query
    }

    /// 1文字以上が打たれているか。
    pub fn has_query(&self) -> bool {
        !self.query.is_empty()
    }

    /// 候補の総数。
    pub fn total(&self) -> usize {
        self.labels.len()
    }

    /// 文字を末尾へ足す。受け付けない設定では何も変えず`false`を返す。
    pub fn type_char(&mut self, character: char) -> bool {
        if !self.is_typed() {
            return false;
        }
        self.query.push(character);
        true
    }

    /// 末尾の1文字を消す。消すものが無ければ`false`を返す。
    pub fn erase(&mut self) -> bool {
        self.query.pop().is_some()
    }

    /// 一致する候補のindex。絞り込み文字列が空なら全候補。
    pub fn matches(&self) -> Vec<usize> {
        (0..self.labels.len())
            .filter(|index| self.is_match(*index))
            .collect()
    }

    /// その候補が見えているか。
    pub fn is_match(&self, index: usize) -> bool {
        let Some(label) = self.labels.get(index) else {
            return false;
        };
        !self.has_query() || find(label, &self.query).is_some()
    }

    /// labelの中で一致した範囲。byte offsetで返す。
    ///
    /// 絞り込み文字列が空のときと、一致しない候補は`None`。強調するのは最初の一致だけである。
    pub fn match_range(&self, index: usize) -> Option<Range<usize>> {
        if !self.has_query() {
            return None;
        }
        find(self.labels.get(index)?, &self.query)
    }

    /// `current`を、絞り込みの変更後も見える候補へ置き直す。
    ///
    /// 残っていればそのまま。隠れたら一致の先頭へ移す。一致が無ければ覚えたまま返し、
    /// 消して一致が戻ったときに同じ候補へ戻れるようにする。
    pub fn anchored(&self, current: usize) -> usize {
        if self.is_match(current) {
            return current;
        }
        self.matches().first().copied().unwrap_or(current)
    }

    /// 一致の中で1つ先の候補。末尾からは先頭へ巡回する。一致が無ければ`current`。
    pub fn next(&self, current: usize) -> usize {
        let matches = self.matches();
        Self::moved(&matches, current, 1)
    }

    /// 一致の中で1つ前の候補。先頭からは末尾へ巡回する。一致が無ければ`current`。
    pub fn previous(&self, current: usize) -> usize {
        let matches = self.matches();
        Self::moved(&matches, current, matches.len().saturating_sub(1))
    }

    /// `matches`の中で`offset`個先の候補。`current`が一致の中に無ければ一致の先頭。
    fn moved(matches: &[usize], current: usize, offset: usize) -> usize {
        let Some(position) = matches.iter().position(|index| *index == current) else {
            return matches.first().copied().unwrap_or(current);
        };
        matches
            .get((position + offset) % matches.len())
            .copied()
            .unwrap_or(current)
    }
}

/// `label`の中で`query`が最初に現れる範囲。大文字小文字を区別しない。
///
/// 文字単位で比べ、範囲は元のlabelのbyte offsetで返す。小文字化で長さが変わる文字が
/// あっても、返す範囲が元の文字の境界から外れない。
fn find(label: &str, query: &str) -> Option<Range<usize>> {
    let needle: Vec<char> = query.chars().collect();
    if needle.is_empty() {
        return None;
    }
    let haystack: Vec<(usize, char)> = label.char_indices().collect();
    let last_start = haystack.len().checked_sub(needle.len())?;
    (0..=last_start).find_map(|start| {
        let window = haystack.get(start..start + needle.len())?;
        let same = window
            .iter()
            .zip(&needle)
            .all(|((_, found), wanted)| same_letter(*found, *wanted));
        if !same {
            return None;
        }
        let begin = window.first()?.0;
        let end = haystack
            .get(start + needle.len())
            .map_or(label.len(), |(offset, _)| *offset);
        Some(begin..end)
    })
}

fn same_letter(left: char, right: char) -> bool {
    left == right || left.to_lowercase().eq(right.to_lowercase())
}

#[cfg(test)]
#[path = "narrowing_test.rs"]
mod narrowing_test;
