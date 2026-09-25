/// hostにあるrepositoryを登録した案件のowner。
///
/// 案件IDは`local/<name>`になる。GitHubの案件と同じ`<owner>/<repository>`の形を保ち、
/// 引数やpromptでの指定の仕方を変えない。
pub const LOCAL_OWNER: &str = "local";
