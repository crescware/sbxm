//! `sbxm init`。
//!
//! configがない場合だけ新規作成する。既存の有効configは再利用し、無効configは停止する。
//! host環境の診断、login、setupは行わない。

use std::path::{Path, PathBuf};

use crate::cli::Interactivity;

use super::Mode;
use crate::command::{CommandSpec, HostEnvironment, TimeoutClass};
use crate::config::{
    self, ConfigLocation, ConfigState, GitIdentity, GlobalConfig, validate_git_identity_value,
};
use crate::error::{Error, ErrorId, Result, fail};
use crate::i18n::{Catalog, Locale, shell_locale};
use crate::msg;
use crate::paths::{self, AbsoluteBasePath, LOCK_TIMEOUT, PRIVATE_FILE_MODE, PathScope};
use crate::ui::{PromptUi, Warning};

/// 対話modeの入力。
///
/// promptはstdinから読み、stderrへ表示する。
pub trait Prompt {
    fn select_language(&mut self, catalog: &Catalog) -> Result<Locale>;
    fn base_path(&mut self, catalog: &Catalog) -> Result<String>;
    fn git_user_name(&mut self, catalog: &Catalog, candidate: &str) -> Result<String>;
    fn git_user_email(&mut self, catalog: &Catalog, candidate: &str) -> Result<String>;
    fn confirm_create_base_path(&mut self, catalog: &Catalog, path: &Path) -> Result<bool>;
}

/// `init`の入力。
pub struct InitRequest {
    pub mode: Mode,
    /// 有効な`--lang`。mode判定には含めない。
    pub lang: Option<Locale>,
    pub interactivity: Interactivity,
}

/// `init`の結果。
#[derive(Debug)]
pub struct InitOutput {
    /// この実行で確定した表示言語。
    pub locale: Locale,
    /// 既に初期化済みで、何も変更しなかったか。
    pub already_initialized: bool,
    pub config_path: PathBuf,
    pub warnings: Vec<Warning>,
}

/// `sbxm init`を実行する。
pub fn run(
    location: &ConfigLocation,
    request: &InitRequest,
    host: &dyn HostEnvironment,
    prompt: &mut dyn Prompt,
) -> Result<InitOutput> {
    // 3. configをread-onlyで事前確認する。
    // 5. 無効なconfigはここで伝播し、自動修復も上書きもしない。
    if let Some(output) = already_initialized(location, request)? {
        return Ok(output);
    }

    // 6. 新規作成へ進む対話modeはstdinとstderrの両方がTTYであることを必須とする。
    if matches!(request.mode, Mode::Interactive) && !request.interactivity.can_prompt() {
        return fail(ErrorId::InitRequiresTty, msg!("error-init-requires-tty"));
    }

    // 7. `~/.sbxm`を検証または作成し、init.lockを取得する。
    config::ensure_config_dir(location)?;
    let lock_path = location.init_lock();
    let _lock = paths::acquire_exclusive_lock(
        &lock_path,
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ConfigFile,
    )?;

    // 8-9. lock取得後にconfigの有無と妥当性を再確認する。
    if let Some(output) = already_initialized(location, request)? {
        return Ok(output);
    }

    let locale = resolve_locale(request, host, prompt)?;
    let catalog = Catalog::new(locale);

    let (base_path, git) = match &request.mode {
        Mode::Options {
            base_path,
            git_user_name,
            git_user_email,
        } => {
            // 11. option modeでは完全指定された値をpromptなしで検証する。
            let base_path = AbsoluteBasePath::new(Path::new(base_path))?;
            let git = GitIdentity {
                user_name: check_identity(&catalog, "user.name", git_user_name)?,
                user_email: check_identity(&catalog, "user.email", git_user_email)?,
            };
            ensure_base_path_exists(&base_path, &catalog, prompt, false)?;
            (base_path, git)
        }
        Mode::Interactive => {
            // 10. 対話modeでは、base path、Git name、Git emailをpromptで取得・検証する。
            let declared = prompt.base_path(&catalog)?;
            let base_path = AbsoluteBasePath::new(Path::new(declared.trim()))?;
            ensure_base_path_exists(&base_path, &catalog, prompt, true)?;

            let git = GitIdentity {
                user_name: check_identity(
                    &catalog,
                    "user.name",
                    &prompt.git_user_name(&catalog, &git_candidate(host, "user.name"))?,
                )?,
                user_email: check_identity(
                    &catalog,
                    "user.email",
                    &prompt.git_user_email(&catalog, &git_candidate(host, "user.email"))?,
                )?,
            };
            (base_path, git)
        }
    };

    // 12. configをatomic writeする。
    let config = GlobalConfig {
        language: locale,
        base_path,
        git,
        files: Vec::new(),
    };
    let config_path = config::create(location, &config)?;

    Ok(InitOutput {
        locale,
        already_initialized: false,
        config_path,
        warnings: Vec::new(),
    })
}

/// 既に有効なconfigがあれば、何も変更せず終了するための結果を返す。
fn already_initialized(
    location: &ConfigLocation,
    request: &InitRequest,
) -> Result<Option<InitOutput>> {
    match config::load(location)? {
        ConfigState::Valid { config, warnings } => Ok(Some(InitOutput {
            locale: request.lang.unwrap_or(config.language),
            already_initialized: true,
            config_path: location.config_file(),
            warnings,
        })),
        ConfigState::Missing => Ok(None),
    }
}

/// Git identityの値を検証する。
///
/// 不正の理由もFTLから生成するため、確定済みのcatalogで解決してから埋め込む。
fn check_identity(catalog: &Catalog, field: &str, value: &str) -> Result<String> {
    match validate_git_identity_value(value) {
        Ok(()) => Ok(value.to_string()),
        Err(detail_id) => {
            let detail = catalog
                .text(detail_id)
                .unwrap_or_else(|failure| failure.to_string());
            fail(
                ErrorId::GitIdentityInvalid,
                msg!("error-git-identity-invalid", field = field, detail = detail),
            )
        }
    }
}

/// base pathが存在しなければ、確認のうえ作成する。
fn ensure_base_path_exists(
    base_path: &AbsoluteBasePath,
    catalog: &Catalog,
    prompt: &mut dyn Prompt,
    confirm: bool,
) -> Result<()> {
    let path = base_path.as_path();
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => fail(
            ErrorId::BasePathNotDirectory,
            msg!("error-base-path-not-directory", path = paths::display(path)),
        ),
        Err(_) => {
            if confirm && !prompt.confirm_create_base_path(catalog, path)? {
                return Err(Error::Canceled);
            }
            std::fs::create_dir_all(path).map_err(|error| {
                Error::new(
                    ErrorId::AtomicWriteFailed,
                    msg!(
                        "error-atomic-write-failed",
                        path = paths::display(path),
                        detail = error
                    ),
                )
            })
        }
    }
}

/// `init`実行時のlocale決定。
///
/// 1. 有効な`--lang`
/// 2. macOS優先言語
/// 3. shell locale
/// 4. 正本locale
fn resolve_locale(
    request: &InitRequest,
    host: &dyn HostEnvironment,
    prompt: &mut dyn Prompt,
) -> Result<Locale> {
    if let Some(locale) = request.lang {
        return Ok(locale);
    }

    let preferred = macos_preferred_language(host);
    match (&request.mode, preferred) {
        // 対話modeで正本locale以外が推測された場合だけ、その推測を確認させる。
        (Mode::Interactive, Some(locale)) if !locale.is_source() => {
            let bootstrap = Catalog::new(locale);
            prompt.select_language(&bootstrap)
        }
        // option modeではpromptを表示しない。
        (_, Some(locale)) => Ok(locale),
        (_, None) => Ok(shell_locale().unwrap_or(Locale::SOURCE)),
    }
}

/// macOS優先言語の先頭要素。
///
/// command失敗またはparse失敗時だけshell localeへfallbackする。
fn macos_preferred_language(host: &dyn HostEnvironment) -> Option<Locale> {
    let spec = CommandSpec::probe("defaults", &["read", "-g", "AppleLanguages"])
        .timeout(TimeoutClass::Probe);
    let outcome = host.run(&spec).ok()?;
    if !outcome.success() {
        return None;
    }
    parse_apple_languages(&outcome.stdout_text())
}

/// `defaults read -g AppleLanguages`の出力から先頭のlanguage tagを取り出す。
fn parse_apple_languages(output: &str) -> Option<Locale> {
    let start = output.find('"')?;
    let rest = &output[start + 1..];
    let end = rest.find('"')?;
    Locale::from_language_tag(&rest[..end])
}

/// hostのGit identityを既定候補として読む。取得できない場合は空文字とする。
fn git_candidate(host: &dyn HostEnvironment, key: &str) -> String {
    let spec = CommandSpec::probe("git", &["config", "--global", key])
        .timeout(TimeoutClass::LocalFilesystem);
    match host.run(&spec) {
        Ok(outcome) if outcome.success() => outcome.stdout_text().trim().to_string(),
        _ => String::new(),
    }
}

/// 共通promptを使う対話実装。
///
/// 言語選択もproject選択と同じ単一選択themeで描く。`init`だけが別のpromptを持つと、
/// 最初に触れる画面がほかのcommandと違って見える。
pub struct TerminalPrompt {
    prompt: PromptUi,
}

impl TerminalPrompt {
    pub fn new(prompt: PromptUi) -> TerminalPrompt {
        TerminalPrompt { prompt }
    }

    fn text(catalog: &Catalog, id: &str) -> String {
        catalog
            .text(id)
            .unwrap_or_else(|failure| failure.to_string())
    }

    /// 中断は何も変更せず終える。それ以外はTTYの不足として報告する。
    ///
    /// `init`は端末が要る唯一のcommandであり、読めない端末は引数不足でも一時障害でもなく
    /// 対話modeを選べない状態そのものである。
    pub(super) fn require_tty(error: Error) -> Error {
        match error {
            Error::Canceled => Error::Canceled,
            _ => Error::new(ErrorId::InitRequiresTty, msg!("error-init-requires-tty")),
        }
    }
}

impl Prompt for TerminalPrompt {
    fn select_language(&mut self, catalog: &Catalog) -> Result<Locale> {
        // 推測されたlocaleを先頭に、残りを定義順で並べる。
        let guessed = catalog.locale();
        let mut choices = vec![guessed];
        choices.extend(Locale::ALL.into_iter().filter(|locale| *locale != guessed));

        // 言語の名称は、その言語自身のresourceが持つ自称表記を使う。
        let items: Vec<String> = choices
            .iter()
            .map(|locale| TerminalPrompt::text(&Catalog::new(*locale), "locale-name"))
            .collect();

        let index = self
            .prompt
            .select_one(msg!("init-prompt-language"), &items)
            .map_err(TerminalPrompt::require_tty)?;
        Ok(*choices
            .get(index)
            .expect("the selection index stays within the offered items"))
    }

    fn base_path(&mut self, _catalog: &Catalog) -> Result<String> {
        self.prompt
            .input(msg!("init-prompt-base-path"), "")
            .map_err(TerminalPrompt::require_tty)
    }

    fn git_user_name(&mut self, _catalog: &Catalog, candidate: &str) -> Result<String> {
        // 候補を表示して明示確定させる。
        self.prompt
            .input(msg!("init-prompt-git-user-name"), candidate)
            .map_err(TerminalPrompt::require_tty)
    }

    fn git_user_email(&mut self, _catalog: &Catalog, candidate: &str) -> Result<String> {
        self.prompt
            .input(msg!("init-prompt-git-user-email"), candidate)
            .map_err(TerminalPrompt::require_tty)
    }

    fn confirm_create_base_path(&mut self, _catalog: &Catalog, path: &Path) -> Result<bool> {
        self.prompt
            .confirm(msg!(
                "init-prompt-create-base-path",
                path = paths::display(path)
            ))
            .map_err(TerminalPrompt::require_tty)
    }
}

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;
