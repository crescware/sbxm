//! Docker Sandboxes Template。
//!
//! label検証を通したarchiveをloadし、期待する名前で登録されたことを確認してから、
//! Sandboxの作成に使う。
//!
//! runtimeのimage storeは、Templateがどのhost imageから来たかを示さない。
//! `sbx template ls --json`が持つのはrepository、tag、runtime内部のidだけであり、
//! host側の`docker image inspect`とは別のstoreの値である。対応の根拠は、
//! loadしたarchiveがlabelで宣言していた案件と世代、およびその名前で登録された
//! ことの2つになる。

use std::path::Path;

use crate::command::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::compatibility::{TemplateEntry, parse_template_list};
use crate::error::{Error, ErrorId, Result};
use crate::msg;
use crate::paths;

use super::image::BuiltImage;
use crate::ui::ProgressSink;

/// 使用できる状態のTemplate。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedTemplate {
    pub name: String,
    /// この実行でloadしたか。
    pub loaded: bool,
}

/// imageに対応するTemplateを用意する。
///
/// 期待する名前のTemplateが既にあれば再利用する。名前には世代のDockerfile hashが
/// 入るため、別世代のTemplateを取り違えることはない。load直後は、その名前が一覧に
/// 現れたことを確かめる。現れない場合はloadの成功を推測しない。
pub fn ensure(
    host: &dyn HostEnvironment,
    archive: &Path,
    image: &BuiltImage,
    progress: &mut dyn ProgressSink,
) -> Result<LoadedTemplate> {
    if find(host, &image.name)?.is_some() {
        return Ok(LoadedTemplate {
            name: image.name.clone(),
            loaded: false,
        });
    }

    progress.step(msg!("progress-loading-template"));
    let spec = CommandSpec::passthrough("sbx", &["template", "load", &paths::display(archive)])
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::SandboxLifecycle);
    host.run(&spec)?.require_success()?;

    if find(host, &image.name)?.is_none() {
        return Err(unusable(
            &image.name,
            "the template is absent right after it was loaded".to_string(),
        ));
    }

    Ok(LoadedTemplate {
        name: image.name.clone(),
        loaded: true,
    })
}

/// 期待する名前のTemplateが既にあるか。
pub fn existing(host: &dyn HostEnvironment, image: &BuiltImage) -> Result<Option<LoadedTemplate>> {
    if find(host, &image.name)?.is_none() {
        return Ok(None);
    }
    Ok(Some(LoadedTemplate {
        name: image.name.clone(),
        loaded: false,
    }))
}

/// 名前が完全一致するTemplateを探す。
fn find(host: &dyn HostEnvironment, name: &str) -> Result<Option<TemplateEntry>> {
    let spec = CommandSpec::capture("sbx", &["template", "ls", "--json"])
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::SandboxLifecycle);
    let outcome = host.run(&spec)?.require_success()?;
    let entries = parse_template_list(&outcome.stdout_text())?;
    Ok(entries.into_iter().find(|entry| entry.is_named(name)))
}

fn unusable(name: &str, detail: String) -> Error {
    Error::new(
        ErrorId::TemplateUnusable,
        msg!("error-template-unusable", template = name, detail = detail),
    )
}

#[cfg(test)]
#[path = "template_test.rs"]
mod template_test;
