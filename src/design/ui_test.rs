use super::*;

/// bufferへ書くUI。testが独立したstreamを注入するために使う。
///
/// 注入するstreamの生存期間を戻り値へ持ち越すため、`'a`を省略できない。
impl<'a> Ui<'a> {
    pub fn capture(
        locale: Locale,
        policy: OutputPolicy,
        stdout: impl std::io::Write + 'a,
        stderr: impl std::io::Write + 'a,
    ) -> Ui<'a> {
        Ui {
            catalog: Catalog::new(locale),
            stdout: Renderer::new(stdout, policy.stdout),
            stderr: Renderer::new(stderr, policy.stderr),
            external: false,
        }
    }
}
