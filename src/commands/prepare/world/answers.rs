//! host上のcommandが返す応答。

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use crate::command::CommandSpec;
use crate::hash::sha256_hex;
use crate::paths;
use crate::testing::archive::image_archive_bytes;

use super::{IMAGE_ID, SandboxRow, World};

impl World {
    pub fn host_git(spec: &CommandSpec) -> (i32, String) {
        let args: Vec<&str> = spec.args.iter().map(String::as_str).collect();
        match args.as_slice() {
            ["clone", _, target] => {
                // cloneが成功したときだけ、working treeができる。作れなければcloneの失敗である。
                if fs::create_dir_all(Path::new(target).join(".git")).is_err() {
                    return (128, String::new());
                }
                (0, String::new())
            }
            // 新規登録が読むhostのGit identity。
            ["config", "--global", "--get-all", "user.name"] => (0, "Example User\n".to_string()),
            ["config", "--global", "--get-all", "user.email"] => {
                (0, "user@example.com\n".to_string())
            }
            ["rev-parse", "--is-bare-repository"] => (0, "false\n".to_string()),
            ["rev-parse", "--show-toplevel"] => match &spec.working_dir {
                // gitはrepositoryの外で128を返す。作業directoryの指定が無い場合も同じ扱いとする。
                None => (128, String::new()),
                Some(directory) => (0, format!("{}\n", paths::display(directory))),
            },
            ["config", "--get-all", "remote.origin.url"] => (
                0,
                "git@github.com:Example-Org/Example-Repo.git\n".to_string(),
            ),
            _ => (0, String::new()),
        }
    }

    pub fn docker(&self, spec: &CommandSpec) -> (i32, String) {
        let args: Vec<&str> = spec.args.iter().map(String::as_str).collect();
        match args.as_slice() {
            ["build", rest @ ..] => {
                let mut labels = Vec::new();
                let mut tag = String::new();
                let mut index = 0;
                while index < rest.len() {
                    match rest[index] {
                        "--label" => {
                            if let Some((key, value)) = rest[index + 1].split_once('=') {
                                labels.push((key.to_string(), value.to_string()));
                            }
                            index += 2;
                        }
                        "--tag" => {
                            tag = rest[index + 1].to_string();
                            index += 2;
                        }
                        _ => index += 1,
                    }
                }
                self.images.borrow_mut().insert(tag, labels);
                (0, String::new())
            }
            ["image", "ls", "--quiet", name] => (
                0,
                if self.images.borrow().contains_key(*name) {
                    "0123456789ab\n".to_string()
                } else {
                    String::new()
                },
            ),
            ["image", "inspect", name] => match self.images.borrow().get(*name) {
                Some(labels) => {
                    let rendered = labels
                        .iter()
                        .map(|(key, value)| format!("\"{key}\":\"{value}\""))
                        .collect::<Vec<_>>()
                        .join(",");
                    (
                        0,
                        format!(r#"[{{"Id":"{IMAGE_ID}","Config":{{"Labels":{{{rendered}}}}}}}]"#),
                    )
                }
                None => (1, String::new()),
            },
            ["image", "save", name, "--output", output] => {
                let owned = self.images.borrow().get(*name).cloned().unwrap_or_default();
                let labels: Vec<(&str, &str)> = owned
                    .iter()
                    .map(|(key, value)| (key.as_str(), value.as_str()))
                    .collect();
                if fs::write(output, image_archive_bytes(name, IMAGE_ID, &labels)).is_err() {
                    return (1, String::new());
                }
                (0, String::new())
            }
            _ => (0, String::new()),
        }
    }

    pub fn sbx(&self, spec: &CommandSpec) -> (i32, String) {
        let args: Vec<&str> = spec.args.iter().map(String::as_str).collect();
        match args.as_slice() {
            ["ls", "--json"] => {
                let rendered = self
                    .sandboxes
                    .borrow()
                    .iter()
                    .map(|row| {
                        format!(
                            r#"{{"name":"{}","state":"running","workspace":"{}","template":"{}","active_sessions":0}}"#,
                            row.name, row.workspace, row.template
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                (0, format!("[{rendered}]"))
            }
            ["template", "ls", "--json"] => {
                // runtimeのimage storeはrepositoryとtagで示し、prefixを補う。
                let rendered = self
                        .templates
                        .borrow()
                        .iter()
                        .filter_map(|(name, id)| {
                            let (repository, tag) = name.rsplit_once(':')?;
                            Some(format!(
                                r#"{{"id":"{id}","repository":"docker.io/library/{repository}","tag":"{tag}"}}"#
                            ))
                        })
                        .collect::<Vec<_>>()
                        .join(",");
                (0, format!(r#"{{"images":[{rendered}]}}"#))
            }
            ["template", "load", archive] => {
                let Ok(manifest) = crate::archive::read_manifest(Path::new(archive)) else {
                    return (1, String::new());
                };
                self.templates.borrow_mut().insert(
                    manifest.repo_tags[0].clone(),
                    manifest.config_digest.clone(),
                );
                (0, String::new())
            }
            [
                "create",
                "--name",
                name,
                "--template",
                template,
                _kit,
                workspace,
            ] => {
                let registered = self
                    .secrets
                    .borrow()
                    .iter()
                    .any(|target| target == crate::support::secret::GITHUB_HOST);
                self.sandboxes.borrow_mut().push(SandboxRow {
                    name: (*name).to_string(),
                    workspace: (*workspace).to_string(),
                    template: (*template).to_string(),
                    placeholder: registered,
                });
                (0, String::new())
            }
            ["secret", "ls", name] => {
                let secrets = self.secrets.borrow();
                if secrets.is_empty() {
                    return (0, format!("No secrets found for scope \"{name}\".\n"));
                }
                // 1件のcustom secretが複数hostを覆う。TARGETS列は空白1つで並ぶ。
                let mut table =
                    String::from("CUSTOM SECRETS\nSCOPE   TARGETS   ENV   PLACEHOLDER   SECRET\n");
                // Stringへの書き込みは失敗しない。
                let _ = writeln!(
                    table,
                    "{name}   {}   GH_TOKEN   sbx-cs-example   ghp_example",
                    secrets.join(" ")
                );
                (0, table)
            }
            ["cp", "--follow-link", source, target] => {
                let (Ok(bytes), Some((_, path))) = (fs::read(source), target.split_once(':'))
                else {
                    return (1, String::new());
                };
                let digest = sha256_hex(&bytes);
                let path = path.to_string();
                self.present.borrow_mut().insert(path.clone());
                self.digests.borrow_mut().insert(path, digest);
                (0, String::new())
            }
            ["daemon", ..] => (0, String::new()),
            _ => self.sandbox_exec(&args),
        }
    }
}
