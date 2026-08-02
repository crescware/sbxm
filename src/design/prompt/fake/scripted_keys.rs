use std::collections::VecDeque;

use console::Key;

use super::super::Keys;

/// 決め打ちした打鍵を順に返すkey source。
///
/// 筋書きを使い切ったまま読もうとしたら失敗させる。読み続ける実装を、待ち続けるのでは
/// なくtestの失敗として現す。
pub struct ScriptedKeys {
    pending: VecDeque<Key>,
    failure: Option<std::io::ErrorKind>,
}

impl ScriptedKeys {
    /// 打鍵をそのまま並べる。
    pub fn pressing(keys: &[Key]) -> ScriptedKeys {
        ScriptedKeys {
            pending: keys.iter().cloned().collect(),
            failure: None,
        }
    }

    /// 1文字ずつ打ってEnterで確定する。
    pub fn typing(text: &str) -> ScriptedKeys {
        let mut keys: Vec<Key> = text.chars().map(Key::Char).collect();
        keys.push(Key::Enter);
        ScriptedKeys::pressing(&keys)
    }

    /// 何も打たずにEnterだけを押す。置かれた候補をそのまま確定する場合にあたる。
    pub fn confirming() -> ScriptedKeys {
        ScriptedKeys::pressing(&[Key::Enter])
    }

    /// 先頭から`index`まで下へ動いてEnterで確定する。
    pub fn choosing(index: usize) -> ScriptedKeys {
        let mut keys = vec![Key::ArrowDown; index];
        keys.push(Key::Enter);
        ScriptedKeys::pressing(&keys)
    }

    /// 昇順のindexをSpaceで選び、Enterで確定する。
    pub fn checking(indexes: &[usize]) -> ScriptedKeys {
        let mut keys = Vec::new();
        let mut current = 0;
        for index in indexes {
            keys.extend(std::iter::repeat_n(
                Key::ArrowDown,
                index.saturating_sub(current),
            ));
            keys.push(Key::Char(' '));
            current = *index;
        }
        keys.push(Key::Enter);
        ScriptedKeys::pressing(&keys)
    }

    /// 何も変更せず終える。
    pub fn canceling() -> ScriptedKeys {
        ScriptedKeys::pressing(&[Key::Escape])
    }

    /// 打鍵を読めない端末。
    pub fn failing(kind: std::io::ErrorKind) -> ScriptedKeys {
        ScriptedKeys {
            pending: VecDeque::new(),
            failure: Some(kind),
        }
    }
}

impl Keys for ScriptedKeys {
    fn read_key(&mut self) -> std::io::Result<Key> {
        if let Some(kind) = self.failure {
            return Err(std::io::Error::from(kind));
        }
        self.pending
            .pop_front()
            .ok_or_else(|| std::io::Error::other("the script ran out of keys"))
    }
}
