/// `GitHub`へのrequestで実際に使う`custom secret`登録の識別情報。
///
/// tokenそのものは持たない。更新時に同じ登録を指し続けるため、scopeと公開値である
/// placeholderだけを、選択から認証確認まで一緒に運ぶ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubRegistration {
    scope: String,
    placeholder: String,
}

impl GithubRegistration {
    pub(super) fn new(scope: &str, placeholder: &str) -> GithubRegistration {
        GithubRegistration {
            scope: scope.to_string(),
            placeholder: placeholder.to_string(),
        }
    }

    pub(super) fn scope(&self) -> &str {
        &self.scope
    }

    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }
}
