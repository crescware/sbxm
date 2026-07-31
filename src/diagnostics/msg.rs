/// FTL message IDと、その引数。
///
/// 利用者向け文字列はすべてFTL resourceから生成するため、診断は表示文字列ではなく
/// message参照として持ち回る。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Msg {
    pub id: &'static str,
    pub args: Vec<(&'static str, String)>,
}

impl Msg {
    pub fn new(id: &'static str) -> Self {
        Msg {
            id,
            args: Vec::new(),
        }
    }

    pub fn with(mut self, key: &'static str, value: impl std::fmt::Display) -> Self {
        self.args.push((key, value.to_string()));
        self
    }
}

/// FTL messageを組み立てる。
///
/// ```ignore
/// msg!("config-missing");
/// msg!("config-invalid-syntax", path = display_path, detail = err);
/// ```
#[macro_export]
macro_rules! msg {
    ($id:expr) => {
        $crate::diagnostics::Msg::new($id)
    };
    ($id:expr, $($key:ident = $value:expr),+ $(,)?) => {
        $crate::diagnostics::Msg::new($id)
            $(.with(stringify!($key), &$value))+
    };
}
