/// `sbx secret ls --json`が示すservice secretの登録。
///
/// serviceは`github`のようにDocker Sandboxesが組み込みで宣言する識別子であり、
/// 対象hostと、Sandbox内の環境変数は、Docker Sandboxes側が決める。値は読まない。
#[derive(Debug, PartialEq, Eq)]
pub struct ServiceSecret {
    /// この登録が属するscope。Sandbox名か、globalを示す表記。
    pub scope: String,
    /// service識別子。
    pub name: String,
}
