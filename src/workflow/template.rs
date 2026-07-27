//! Docker Sandboxes Template。
//!
//! 検証済みのarchiveをloadし、期待するimageに対応していることを確認してから、
//! Sandboxの作成に使う。

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
/// 同名のTemplateは、期待するimageに対応していることを観測できた場合だけ再利用する。
/// loadした直後も同じ判定を行い、対応関係を観測できないruntimeでは、既存Templateも
/// loadの結果も推測で受け入れない。
pub fn ensure(
    host: &dyn HostEnvironment,
    archive: &Path,
    image: &BuiltImage,
) -> Result<LoadedTemplate> {
    if let Some(entry) = find(host, &image.name)? {
        holds_image(&entry, image)?;
        return Ok(LoadedTemplate {
            name: image.name.clone(),
            loaded: false,
        });
    }

    let spec = CommandSpec::passthrough("sbx", &["template", "load", &paths::display(archive)])
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::SandboxLifecycle);
    host.run(&spec)?.require_success()?;

    let Some(entry) = find(host, &image.name)? else {
        return Err(unusable(
            &image.name,
            "the template is absent right after it was loaded".to_string(),
        ));
    };
    holds_image(&entry, image)?;

    Ok(LoadedTemplate {
        name: image.name.clone(),
        loaded: true,
    })
}

/// Templateが期待するimageを持つことを、runtimeの出力から確かめる。
fn holds_image(entry: &TemplateEntry, image: &BuiltImage) -> Result<()> {
    match &entry.image_id {
        Some(id) if *id == image.id => Ok(()),
        Some(id) => Err(unusable(
            &image.name,
            format!("the template holds image {id}, not {}", image.id),
        )),
        None => Err(unusable(
            &image.name,
            "this Docker Sandboxes version does not report which image a template holds"
                .to_string(),
        )),
    }
}

/// 名前が完全一致するTemplateを探す。
fn find(host: &dyn HostEnvironment, name: &str) -> Result<Option<TemplateEntry>> {
    let spec = CommandSpec::capture("sbx", &["template", "ls", "--json"])
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::SandboxLifecycle);
    let outcome = host.run(&spec)?.require_success()?;
    let entries = parse_template_list(&outcome.stdout_text())?;
    Ok(entries.into_iter().find(|entry| entry.name == name))
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
            built: true,
            warnings: Vec::new(),
        }
    }

    fn listing(name: &str, image_id: Option<&str>) -> String {
        match image_id {
            Some(id) => format!(r#"[{{"name":"{name}","image_id":"{id}"}}]"#),
            None => format!(r#"[{{"name":"{name}"}}]"#),
        }
    }

    #[test]
    fn an_archive_is_loaded_and_the_result_is_verified() {
        let image = image();
        let host = FakeSbx::listing(&["[]", &listing(&image.name, Some(&image.id))]);
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
        let host = FakeSbx::listing(&["[]", &listing(&image.name, Some(&image.id))]);
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
        let host = FakeSbx::listing(&[&listing(&image.name, Some(&image.id))]);

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
    fn a_template_of_the_same_name_that_holds_another_image_stops_the_run() {
        let image = image();
        let host = FakeSbx::listing(&[&listing(&image.name, Some("sha256:other"))]);

        let error = ensure(&host, Path::new("/tmp/template.tar"), &image)
            .expect_err("the name is taken by another image");
        assert_eq!(error.first_id(), Some(ErrorId::TemplateUnusable));
    }

    #[test]
    fn a_runtime_that_hides_the_image_of_a_template_is_not_guessed_at() {
        let image = image();
        let host = FakeSbx::listing(&[&listing(&image.name, None)]);

        let error = ensure(&host, Path::new("/tmp/template.tar"), &image)
            .expect_err("an unobservable correspondence is not assumed to match");
        assert_eq!(error.first_id(), Some(ErrorId::TemplateUnusable));
        assert!(
            !host
                .calls()
                .iter()
                .any(|args| args.get(1).is_some_and(|arg| arg == "load")),
            "nothing is loaded over a template that cannot be identified"
        );
    }

    #[test]
    fn a_load_whose_result_cannot_be_identified_is_not_accepted() {
        let image = image();
        // loadは成功するが、runtimeはどのimageを持つTemplateなのかを示さない。
        let host = FakeSbx::listing(&["[]", &listing(&image.name, None)]);

        let error = ensure(&host, Path::new("/tmp/template.tar"), &image)
            .expect_err("a load is only done when the result proves it");
        assert_eq!(error.first_id(), Some(ErrorId::TemplateUnusable));
    }

    #[test]
    fn a_load_that_leaves_no_template_behind_is_a_failure() {
        let image = image();
        let host = FakeSbx::listing(&["[]", "[]"]);

        let error = ensure(&host, Path::new("/tmp/template.tar"), &image)
            .expect_err("the load has to produce the template");
        assert_eq!(error.first_id(), Some(ErrorId::TemplateUnusable));
    }
}
