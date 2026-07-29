use super::*;
use crate::command::CommandOutcome;
use crate::testing::image::template_listing;
use std::cell::RefCell;

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
        Ok(crate::testing::command::outcome(spec, 0, &stdout))
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

#[test]
fn an_archive_is_loaded_and_the_result_is_verified() {
    let image = image();
    let host = FakeSbx::listing(&[r#"{"images":[]}"#, &template_listing(&image.name)]);
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
    let host = FakeSbx::listing(&[r#"{"images":[]}"#, &template_listing(&image.name)]);
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
    let host = FakeSbx::listing(&[&template_listing(&image.name)]);

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
    let host = FakeSbx::listing(&[&template_listing(&image.name)]);

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
