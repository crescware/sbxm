/// Sandboxから保存したrefを、hostのrepositoryで置く名前空間。`refs/sbx/<sandbox>/`。
///
/// Sandbox名は`sbxm-`で始まる英小文字、数字、`-`だけからなる。ref名の区切りを
/// 持ち込まず、別のSandboxの名前空間と前方一致で重ならない。
pub fn saved_namespace(sandbox: &str) -> String {
    format!("refs/sbx/{sandbox}/")
}
