/// branchとtagは通常のrefとして読み、worktree HEADはOIDを直接要求する。
pub(super) const REF_KINDS: [(&str, &str); 2] =
    [("refs/heads/", "heads/"), ("refs/tags/", "tags/")];
