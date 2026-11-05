use crate::boundary::host::{CommandOutcome, CommandSpec};

/// 容量確認用VMの寿命。元の案件のscripted listingを進めずに再現する。
#[derive(Default)]
pub struct FakeCapacityProbe {
    current: Option<(String, String)>,
    removed: bool,
}

impl FakeCapacityProbe {
    pub fn answer(&mut self, spec: &CommandSpec) -> Option<CommandOutcome> {
        let args = spec.args.iter().map(String::as_str).collect::<Vec<_>>();
        if let ["create", "--name", name, "--template", _, _, workspace] = args.as_slice()
            && name.starts_with("sbxm-capacity-check-")
        {
            self.current = Some(((*name).into(), (*workspace).into()));
            return Some(crate::testing::command::outcome(spec, 0, ""));
        }
        if spec.program == "sbx" && args == ["ls", "--json"] && self.removed {
            self.removed = false;
            return Some(crate::testing::command::outcome(spec, 0, "[]"));
        }
        let (name, workspace) = self.current.as_ref()?;
        if spec.program == "sbx" && args == ["ls", "--json"] {
            let value =
                serde_json::json!([{"name":name,"status":"running","workspaces":[workspace]}]);
            return Some(crate::testing::command::outcome(
                spec,
                0,
                &value.to_string(),
            ));
        }
        if spec.program == "curl"
            && args.contains(&"DELETE")
            && args.iter().any(|arg| arg.contains(name.as_str()))
        {
            self.current = None;
            self.removed = true;
            return Some(crate::testing::command::outcome(spec, 0, ""));
        }
        if let ["exec", sandbox, "--", "df", flag, path] = args.as_slice()
            && *sandbox == name
        {
            let counters = if *flag == "-Pi" {
                "Inodes IUsed IFree IUse%"
            } else {
                "1024-blocks Used Available Capacity"
            };
            let output = format!(
                "Filesystem {counters} Mounted on\noverlay 20000000 100000 19900000 1% {path}\n"
            );
            return Some(crate::testing::command::outcome(spec, 0, &output));
        }
        None
    }
}
