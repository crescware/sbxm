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
) -> Result<LoadedTemplate> {
    if find(host, &image.name)?.is_some() {
        return Ok(LoadedTemplate {
            name: image.name.clone(),
            loaded: false,
        });
    }

    crate::progress::step(&msg!("progress-loading-template"));
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
mod tests {
    use super::*;
    use crate::command::CommandOutcome;
    use std::cell::RefCell;
    use std::os::unix::process::ExitStatusExt;

    struct FakeSbx {
        /// `template ls`が返す出力。呼び出しごとに先頭から使う。
        listings: RefCell<Vec<String>>,
        calls: RefCell<Vec<CommandSpec>>,
    }

    impl FakeSbx {
        fn listing(outputs: &[&str]) -> FakeSbx {
            FakeSbx {
                listings: RefCell::new(
                    outputs
                        .iter()
                        .rev()
                        .map(|value| value.to_string())
                        .collect(),
                ),
                calls: RefCell::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<Vec<String>> {
            self.calls
                .borrow()
                .iter()
                .map(|spec| spec.args.clone())
                .collect()
        }
    }

    impl HostEnvironment for FakeSbx {
        fn command_exists(&self, _program: &str) -> bool {
            true
        }

        fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
            self.calls.borrow_mut().push(spec.clone());
            let stdout = if spec.args.get(1).is_some_and(|arg| arg == "ls") {
                self.listings.borrow_mut().pop().unwrap_or_default()
            } else {
                String::new()
            };
            Ok(CommandOutcome {
                program: spec.program.clone(),
                args: spec.args.clone(),
                working_dir: spec.working_dir.clone(),
                status: std::process::ExitStatus::from_raw(0),
                stdout: stdout.into_bytes(),
                stderr: Vec::new(),
                stderr_lossy: false,
            })
        }
    }

    fn image() -> BuiltImage {
        BuiltImage {
            name: "sbxm-example-template:111111111111".to_string(),
            id: "sha256:abc".to_string(),
            labels: Vec::new(),
            built: true,
            warnings: Vec::new(),
        }
    }

    /// runtimeのimage storeが示す一覧。registry prefixを補って表示する。
    fn listing(name: &str) -> String {
        let (repository, tag) = name.rsplit_once(':').expect("an image reference");
        format!(
            r#"{{"images":[{{"id":"a3d0f4449170","repository":"docker.io/library/{repository}","tag":"{tag}"}}]}}"#
        )
    }

    #[test]
    fn an_archive_is_loaded_and_the_result_is_verified() {
        let image = image();
        let host = FakeSbx::listing(&[r#"{"images":[]}"#, &listing(&image.name)]);
        let archive = Path::new("/tmp/template-111111111111.tar");

        let template = ensure(&host, archive, &image).expect("load");
        assert!(template.loaded);
        assert_eq!(template.name, image.name);

        let calls = host.calls();
        assert_eq!(
            calls[1],
            vec![
                "template".to_string(),
                "load".to_string(),
                "/tmp/template-111111111111.tar".to_string()
            ]
        );
        assert_eq!(calls.len(), 3, "the load is verified afterwards: {calls:?}");
    }

    #[test]
    fn every_sandbox_command_runs_without_the_ssh_agent() {
        let image = image();
        let host = FakeSbx::listing(&[r#"{"images":[]}"#, &listing(&image.name)]);
        ensure(&host, Path::new("/tmp/template.tar"), &image).expect("load");

        for spec in host.calls.borrow().iter() {
            assert_eq!(
                spec.env,
                EnvPolicy::InheritWithoutSshAgent,
                "{:?} must not reach the host SSH agent",
                spec.args
            );
        }
    }

    #[test]
    fn a_template_that_already_holds_the_image_is_reused() {
        let image = image();
        let host = FakeSbx::listing(&[&listing(&image.name)]);

        let template = ensure(&host, Path::new("/tmp/template.tar"), &image).expect("reuse");
        assert!(!template.loaded);
        assert!(
            !host
                .calls()
                .iter()
                .any(|args| args.get(1).is_some_and(|arg| arg == "load")),
            "a template that already holds the image is not loaded again"
        );
    }

    #[test]
    fn the_registry_prefix_the_runtime_adds_still_names_the_same_template() {
        let image = image();
        // runtimeは`docker.io/library/`を補って表示する。sbxmが渡すのは補う前の表記。
        let host = FakeSbx::listing(&[&listing(&image.name)]);

        let template = ensure(&host, Path::new("/tmp/template.tar"), &image)
            .expect("the prefixed listing names the same template");
        assert!(!template.loaded);
    }

    #[test]
    fn a_load_that_leaves_no_template_behind_is_a_failure() {
        let image = image();
        let host = FakeSbx::listing(&[r#"{"images":[]}"#, r#"{"images":[]}"#]);

        let error = ensure(&host, Path::new("/tmp/template.tar"), &image)
            .expect_err("the load has to produce the template");
        assert_eq!(error.first_id(), Some(ErrorId::TemplateUnusable));
    }
}
