/// `sbx template ls`が示すTemplate 1件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateEntry {
    /// このentryを指す名前。registry prefixを補う前後の両方を持つ。
    pub names: Vec<String>,
    /// runtimeのimage storeが割り当てたid。名前だけでは、別内容のTemplateへ同じ名前が
    /// 再利用されたことを見分けられない。
    pub id: Option<String>,
}

impl TemplateEntry {
    /// 与えられた参照がこのentryを指すか。
    pub fn is_named(&self, reference: &str) -> bool {
        self.names.iter().any(|name| name == reference)
    }
}
