//! host上のcommandが返す応答。

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use crate::boundary::host::CommandSpec;
use crate::paths;
use crate::testing::archive::{image_archive_bytes, index_id};

use super::{IMAGE_ID, SandboxRow, World};

impl World {
    pub fn host_git(spec: &CommandSpec) -> (i32, String) {
        let args: Vec<&str> = spec.args.iter().map(String::as_str).collect();
        match args.as_slice() {
            ["clone", "--progress", _, target] => {
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
            // hostにあるrepositoryは、送るbranchを持つ。
            [
                "for-each-ref",
                "--count=1",
                "--format=%(refname)",
                "refs/heads/",
                "refs/tags/",
            ] => (0, "refs/heads/main\n".to_string()),
            // bundleは中身を読まれない。送った範囲だけを書いておく。
            ["bundle", "create", "--quiet", path, revisions @ ..] => {
                match fs::write(path, format!("bundle of {}\n", revisions.join(" "))) {
                    Ok(()) => (0, String::new()),
                    Err(_) => (128, String::new()),
                }
            }
            ["config", "--get-all", "remote.origin.url"] => (
                0,
                "git@github.com:Example-Org/Example-Repo.git\n".to_string(),
            ),
            // 2つのfileの差分。実物のgitと同じく、差分があれば`1`で答える。
            [.., "diff", "--no-index", _, _, _, "--", before, after] => {
                match (fs::read_to_string(before), fs::read_to_string(after)) {
                    (Ok(old), Ok(new)) if old == new => (0, String::new()),
                    (Ok(old), Ok(new)) => (1, format!("--- {before}\n+++ {after}\n-{old}+{new}")),
                    _ => (128, String::new()),
                }
            }
            _ => (0, String::new()),
        }
    }

    pub fn docker(&self, spec: &CommandSpec) -> (i32, String) {
        let args: Vec<&str> = spec.args.iter().map(String::as_str).collect();
        match args.as_slice() {
            ["version", "--format", "{{.Server.Version}}"] => (0, "27.0.3\n".to_string()),
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
                            r#"{{"name":"{}","status":"{}","workspaces":["{}"]}}"#,
                            row.name,
                            if row.running { "running" } else { "stopped" },
                            row.workspace
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                (0, format!(r#"{{"sandboxes":[{rendered}]}}"#))
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
                // containerd image storeを持つ実物のruntimeと同じく、登録するidは
                // `image save`が`index.json`へ書いたdigestであり、image configの
                // digestではない。`image_archive_bytes`が書く値をここで独立に再現する。
                let Ok(manifest) = crate::archive::read_manifest(Path::new(archive)) else {
                    return (1, String::new());
                };
                self.templates.borrow_mut().insert(
                    manifest.repo_tags[0].clone(),
                    index_id(&manifest.config_digest),
                );
                (0, String::new())
            }
            [
                "create",
                "--name",
                name,
                "--template",
                _template,
                _kit,
                workspace,
            ] => {
                self.sandboxes.borrow_mut().push(SandboxRow {
                    name: (*name).to_string(),
                    workspace: (*workspace).to_string(),
                    running: true,
                });
                (0, String::new())
            }
            ["secret", "ls"] => {
                let secrets = self.secrets.borrow();
                if secrets.is_empty() {
                    return (0, "No secrets found.\n".to_string());
                }
                // 1件のcustom secretが複数hostを覆う。TARGETS列は空白1つで並ぶ。
                let mut table =
                    String::from("CUSTOM SECRETS\nSCOPE   TARGETS   ENV   PLACEHOLDER   SECRET\n");
                // Stringへの書き込みは失敗しない。
                let _ = writeln!(
                    table,
                    "(global)   {}   GH_TOKEN   sbx-cs-example   ghp_example",
                    secrets.join(" ")
                );
                (0, table)
            }
            ["daemon", ..] => (0, String::new()),
            _ => match spec.input().map(<[u8]>::to_vec).or_else(|| {
                // 実物と同じく、つながれたfileは読み切った中身として届く。
                spec.input_file().and_then(|path| std::fs::read(path).ok())
            }) {
                Some(input) => self.receive(&args, &input),
                None => self.sandbox_exec(&args),
            },
        }
    }
}
