//! Docker Sandboxes CLIの出力を解釈する。
//!
//! 解釈できない出力から状態を推測しない。parseできない出力はerrorとして扱う。

use crate::error::{Error, ErrorId, Result};
use crate::image_labels::{LabelDefect, labels_from_declared};
use crate::msg;

/// 要件となる最小version。
pub const MINIMUM_CLI_VERSION: CliVersion = CliVersion {
    major: 0,
    minor: 37,
    patch: 0,
};

/// 期待するnetwork policy。ほかのpolicyは、より制限が強い場合も含めて対応しない。
pub const EXPECTED_NETWORK_POLICY: &str = "Balanced";

/// `<major>.<minor>.<patch>`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CliVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl CliVersion {
    /// `0.37.0`のような厳密な3要素表記だけを受け付ける。
    pub fn parse(value: &str) -> Option<CliVersion> {
        let mut parts = value.trim().split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some(CliVersion {
            major,
            minor,
            patch,
        })
    }

    /// `sbx version`の出力から最初のversion表記を取り出す。
    ///
    /// 数値3要素の並びだけを認め、見つからない場合はparse不能として扱う。
    pub fn extract_from_output(output: &str) -> Option<CliVersion> {
        for token in output.split(|c: char| !(c.is_ascii_digit() || c == '.')) {
            if let Some(version) = CliVersion::parse(token) {
                return Some(version);
            }
        }
        None
    }
}

impl std::fmt::Display for CliVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// 検出したversionが最小要件を満たすかを判定する。
pub fn require_minimum_version(observed: CliVersion) -> Result<()> {
    if observed >= MINIMUM_CLI_VERSION {
        return Ok(());
    }
    Err(Error::new(
        ErrorId::SbxVersionBelowMinimum,
        msg!(
            "error-sbx-version-below-minimum",
            observed = observed,
            minimum = MINIMUM_CLI_VERSION
        ),
    ))
}

/// daemonの状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonState {
    Running,
    Stopped,
}

/// `sbx daemon status`をparseする。
pub fn parse_daemon_status(output: &str) -> Result<DaemonState> {
    // `sbx daemon status`はJSONを持たず、`<label>: <value>`の行を並べる。
    // socketとlogのpathはhostのuser名を含むため読まない。
    let state = output.lines().find_map(|line| {
        line.split_once(':')
            .filter(|(label, _)| label.trim().eq_ignore_ascii_case("status"))
            .map(|(_, value)| value.trim().to_ascii_lowercase())
    });

    match state.as_deref() {
        Some("running") => Ok(DaemonState::Running),
        Some("stopped") | Some("not running") | Some("not-running") => Ok(DaemonState::Stopped),
        Some(other) => Err(unparseable(
            "sbx daemon status",
            &format!("status {other} has no defined meaning in this build"),
        )),
        None => Err(unparseable(
            "sbx daemon status",
            "no line states the daemon status",
        )),
    }
}

/// `sbx policy ls`から現在のnetwork policyを取り出す。
pub fn parse_network_policy(output: &str) -> Result<String> {
    let document: serde_json::Value = serde_json::from_str(output)
        .map_err(|error| unparseable("sbx policy ls", &error.to_string()))?;

    let from_object = |object: &serde_json::Map<String, serde_json::Value>| -> Option<String> {
        string_field(object, "policy")
            .or_else(|| string_field(object, "current"))
            .or_else(|| string_field(object, "name"))
    };

    match &document {
        serde_json::Value::Object(object) => from_object(object)
            .ok_or_else(|| unparseable("sbx policy ls", "no policy field is present")),
        serde_json::Value::Array(items) => {
            // 一覧形式では、有効と印の付いた1件だけを現在値とする。
            let mut active: Vec<String> = Vec::new();
            for item in items {
                let Some(object) = item.as_object() else {
                    return Err(unparseable("sbx policy ls", "an entry is not an object"));
                };
                let selected = object
                    .get("active")
                    .or_else(|| object.get("current"))
                    .or_else(|| object.get("selected"))
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false);
                if selected && let Some(name) = from_object(object) {
                    active.push(name);
                }
            }
            match active.len() {
                1 => Ok(active.remove(0)),
                0 => Err(unparseable(
                    "sbx policy ls",
                    "no entry is marked as the active policy",
                )),
                _ => Err(unparseable(
                    "sbx policy ls",
                    "more than one entry is marked as the active policy",
                )),
            }
        }
        _ => Err(unparseable(
            "sbx policy ls",
            "the document is neither an array nor an object",
        )),
    }
}

/// `docker image inspect`から読むimageの同一性。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageIdentity {
    /// `sha256:<hex>`。archiveのconfig blobと同じ値になる。
    pub id: String,
    pub labels: std::collections::BTreeMap<String, String>,
}

/// `docker image inspect <image>`のstructured outputをparseする。
///
/// 1件のimageを指すため、要素が1個の配列だけを受け付ける。labelを持たないimageは
/// 空のlabel集合として扱い、labelの不足は呼び出し側が判定する。
pub fn parse_image_inspect(output: &str) -> Result<ImageIdentity> {
    let document: serde_json::Value = serde_json::from_str(output)
        .map_err(|error| unparseable("docker image inspect", &error.to_string()))?;
    let items = document
        .as_array()
        .ok_or_else(|| unparseable("docker image inspect", "the document is not an array"))?;
    let [item] = items.as_slice() else {
        return Err(unparseable(
            "docker image inspect",
            &format!(
                "the document describes {} images instead of one",
                items.len()
            ),
        ));
    };
    let object = item
        .as_object()
        .ok_or_else(|| unparseable("docker image inspect", "the entry is not an object"))?;

    let id = string_field(object, "Id")
        .filter(|id| !id.is_empty())
        .ok_or_else(|| unparseable("docker image inspect", "the image has no Id"))?;

    let Some(config) = object.get("Config").and_then(|config| config.as_object()) else {
        return Err(unparseable(
            "docker image inspect",
            "the image has no Config section",
        ));
    };
    let labels = labels_from_declared(config.get("Labels")).map_err(|defect| match defect {
        LabelDefect::NotAnObject => unparseable(
            "docker image inspect",
            "Labels is neither an object nor null",
        ),
        LabelDefect::ValueNotAString(key) => unparseable(
            "docker image inspect",
            &format!("label {key} does not hold a string"),
        ),
    })?;

    Ok(ImageIdentity { id, labels })
}

/// `sbx login status`からlogin済みかどうかを読む。
///
/// 真偽を示すfieldがない出力から、login済みだと推測しない。
pub fn parse_login_status(output: &str) -> Result<bool> {
    let document: serde_json::Value = serde_json::from_str(output.trim())
        .map_err(|error| unparseable("sbx login status", &error.to_string()))?;
    let object = document
        .as_object()
        .ok_or_else(|| unparseable("sbx login status", "the document is not an object"))?;

    for key in ["logged_in", "loggedIn", "authenticated", "signed_in"] {
        if let Some(value) = object.get(key) {
            return value.as_bool().ok_or_else(|| {
                unparseable("sbx login status", &format!("{key} is not a boolean"))
            });
        }
    }
    Err(unparseable(
        "sbx login status",
        "no field states whether this host is signed in",
    ))
}

/// Sandboxのruntime状態。未知の値を既知の値へ丸めない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxState {
    Running,
    Stopped,
}

/// `sbx ls`が示すSandbox 1件。
///
/// 対応versionの`sbx ls --json`は、元にしたTemplateも接続中のsession数も示さない。
/// 示されない値をOptionで持つと、その`None`をどう読むかを利用側がそれぞれ決めることに
/// なる。読めないものは持たない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxEntry {
    pub name: String,
    pub state: SandboxState,
    /// runtimeが示したままのstate。管理外Sandboxの表示に使う。
    pub raw_state: String,
    /// Sandboxへ渡したworkspace。示されない場合は`None`。
    pub workspace: Option<String>,
}

/// `sbx ls --json`のstructured outputをparseする。
pub fn parse_sandbox_list(output: &str) -> Result<Vec<SandboxEntry>> {
    let documents = sandbox_documents(output)?;

    let mut entries = Vec::with_capacity(documents.len());
    for document in documents {
        let object = document
            .as_object()
            .ok_or_else(|| unparseable("sbx ls", "an entry is not an object"))?;

        let name = string_field(object, "name")
            .or_else(|| string_field(object, "Name"))
            .filter(|name| !name.is_empty())
            .ok_or_else(|| unparseable("sbx ls", "an entry has no name"))?;

        let observed = string_field(object, "state")
            .or_else(|| string_field(object, "status"))
            .or_else(|| string_field(object, "State"))
            .or_else(|| string_field(object, "Status"))
            .ok_or_else(|| unparseable("sbx ls", &format!("sandbox {name} has no state")))?;
        let state = match observed.to_ascii_lowercase().as_str() {
            "running" => SandboxState::Running,
            "stopped" => SandboxState::Stopped,
            other => {
                return Err(unparseable(
                    "sbx ls",
                    &format!("state {other} has no defined meaning in this build"),
                ));
            }
        };

        let workspace = workspace_of(object)?;

        entries.push(SandboxEntry {
            name,
            state,
            raw_state: observed,
            workspace,
        });
    }
    Ok(entries)
}

/// `sbx ls --json`が並べるSandbox。
///
/// 対象versionは`{"sandboxes": [...]}`で包む。包みのない形も、行区切りの形も
/// 受け付けるが、Sandbox以外のkeyを持つ包みは推測せずerrorにする。
fn sandbox_documents(output: &str) -> Result<Vec<serde_json::Value>> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if let Ok(serde_json::Value::Object(object)) =
        serde_json::from_str::<serde_json::Value>(trimmed)
        && let Some(listed) = object.get("sandboxes")
    {
        return match listed {
            serde_json::Value::Array(items) => Ok(items.clone()),
            // Sandboxが1件もない場合の表現として受け付ける。
            serde_json::Value::Null => Ok(Vec::new()),
            _ => Err(unparseable("sbx ls", "sandboxes is not a list")),
        };
    }
    json_documents("sbx ls", output)
}

/// Sandboxが使っているWorkspace。
///
/// 対象versionは`workspaces`を配列で示す。sbxmが作るSandboxは中立Workspaceを
/// 1つだけ持つため、2つ以上ある一覧からはこの案件の成果物と判定しない。
fn workspace_of(object: &serde_json::Map<String, serde_json::Value>) -> Result<Option<String>> {
    let listed = object
        .get("workspaces")
        .or_else(|| object.get("Workspaces"));
    let Some(listed) = listed else {
        return Ok(string_field(object, "workspace")
            .or_else(|| string_field(object, "Workspace"))
            .filter(|value| !value.is_empty()));
    };

    match listed {
        serde_json::Value::Array(items) => match items.as_slice() {
            [] => Ok(None),
            [only] => {
                let value = only
                    .as_str()
                    .ok_or_else(|| unparseable("sbx ls", "a workspace is not a string"))?;
                Ok(Some(value.to_string()).filter(|value| !value.is_empty()))
            }
            _ => Err(unparseable(
                "sbx ls",
                &format!(
                    "the sandbox works in {} workspaces instead of one",
                    items.len()
                ),
            )),
        },
        serde_json::Value::Null => Ok(None),
        _ => Err(unparseable("sbx ls", "workspaces is not a list")),
    }
}

/// `sbx secret ls`が示すcustom secretの登録。
///
/// 対象hostと環境変数名だけを持つ。PLACEHOLDER列とSECRET列は読まない。前者は
/// sandboxの中から観測できる値であり、後者にはtokenの一部が現れる。
#[derive(Debug, PartialEq, Eq)]
pub struct CustomSecret {
    /// proxyが認証を差し替える対象host。
    pub targets: Vec<String>,
    /// placeholderを受け取るSandbox内の環境変数名。
    pub env: String,
    /// Sandboxが実際に見る値。
    ///
    /// tokenそのものではなく、tokenの居場所を指す公開の目印である。同じenvへ登録を
    /// やり直すとき、この値を`--placeholder`へ渡せばSandboxが持つ値と一致したまま
    /// 更新できる。読むのはそのためだけであり、隣の`SECRET`列は読まない。
    pub placeholder: String,
}

/// custom secretの表が始まる見出し。
const CUSTOM_SECRETS_HEADING: &str = "CUSTOM SECRETS";

/// `sbx secret ls`が示すcustom secretを読む。
///
/// 値は取得も表示もしない。存在と宛先だけを読む。service secretとregistry secretは
/// 別の表に並び、custom secretとは扱いが異なるため読み飛ばす。
pub fn parse_custom_secrets(output: &str) -> Result<Vec<CustomSecret>> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return Err(unparseable("sbx secret ls", "the output is empty"));
    }
    // 1件もない場合は表ではなく文で示す。
    if trimmed.starts_with("No secrets found") {
        return Ok(Vec::new());
    }

    let mut lines = trimmed.lines();
    // 見出しがない出力は、custom secretが1件もないことを示す。`any`は一致した行まで
    // 読み進めるので、続く`find`が見出し直後の行から始まる。
    if !lines.any(|line| line.trim() == CUSTOM_SECRETS_HEADING) {
        return Ok(Vec::new());
    }
    let header = lines
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| unparseable("sbx secret ls", "the custom secret listing has no header"))?;
    let columns = table_fields(header);
    let column_of = |wanted: &str| -> Result<usize> {
        columns
            .iter()
            .position(|column| *column == wanted)
            .ok_or_else(|| {
                unparseable(
                    "sbx secret ls",
                    &format!("the custom secret listing has no {wanted} column"),
                )
            })
    };
    let targets_at = column_of("TARGETS")?;
    let env_at = column_of("ENV")?;
    let placeholder_at = column_of("PLACEHOLDER")?;

    let mut customs = Vec::new();
    for line in lines {
        // 空行が表の終わりを示す。
        if line.trim().is_empty() {
            break;
        }
        let fields = table_fields(line);
        // 列数の合わない行からは、どの値がどの列かを決められない。
        if fields.len() != columns.len() {
            return Err(unparseable(
                "sbx secret ls",
                &format!(
                    "a custom secret row holds {} values for {} columns",
                    fields.len(),
                    columns.len()
                ),
            ));
        }
        customs.push(CustomSecret {
            targets: fields[targets_at]
                .split([',', ' '])
                .filter(|target| !target.is_empty())
                .map(str::to_string)
                .collect(),
            env: fields[env_at].to_string(),
            placeholder: fields[placeholder_at].to_string(),
        });
    }
    Ok(customs)
}

/// 桁を揃えた表の1行を列へ分ける。
///
/// 区切りは2つ以上の空白とする。1つの列が複数の値を空白で並べることがあり、
/// 空白1つで切ると列の対応が崩れる。
fn table_fields(line: &str) -> Vec<&str> {
    line.split("  ")
        .map(str::trim)
        .filter(|field| !field.is_empty())
        .collect()
}

/// 一覧形式と1行1件のJSON形式のどちらでも読む。
fn json_documents(program: &str, output: &str) -> Result<Vec<serde_json::Value>> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        // 1件もないことは観測できた状態であり、推測ではない。
        return Ok(Vec::new());
    }
    match serde_json::from_str(trimmed) {
        Ok(serde_json::Value::Array(items)) => Ok(items),
        Ok(serde_json::Value::Object(object)) => Ok(vec![serde_json::Value::Object(object)]),
        Ok(_) => Err(unparseable(
            program,
            "the document is neither an array nor an object",
        )),
        Err(_) => trimmed
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                serde_json::from_str(line).map_err(|error| unparseable(program, &error.to_string()))
            })
            .collect(),
    }
}

/// `sbx template ls`が示すTemplate 1件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateEntry {
    /// このentryを指す名前。registry prefixを補う前後の両方を持つ。
    pub names: Vec<String>,
}

impl TemplateEntry {
    /// 与えられた参照がこのentryを指すか。
    pub fn is_named(&self, reference: &str) -> bool {
        self.names.iter().any(|name| name == reference)
    }
}

/// `sbx template ls`のstructured outputをparseする。
///
/// 一覧形式と、1行1件のJSON形式のどちらでも読む。名前を持たないentryがある出力は、
/// 一覧として信用できないためparse不能として扱う。
pub fn parse_template_list(output: &str) -> Result<Vec<TemplateEntry>> {
    let documents = template_documents(output)?;

    let mut entries = Vec::with_capacity(documents.len());
    for document in documents {
        let object = document
            .as_object()
            .ok_or_else(|| unparseable("sbx template ls", "an entry is not an object"))?;

        // runtimeのimage storeはrepositoryとtagで1件を示す。
        let repository = string_field(object, "repository")
            .or_else(|| string_field(object, "Repository"))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| unparseable("sbx template ls", "an entry has no repository"))?;
        let tag = string_field(object, "tag")
            .or_else(|| string_field(object, "Tag"))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                unparseable(
                    "sbx template ls",
                    &format!("the entry for {repository} has no tag"),
                )
            })?;

        entries.push(TemplateEntry {
            names: reference_names(&repository, &tag),
        });
    }
    Ok(entries)
}

/// `sbx template ls --json`が並べるimage。
///
/// 対象versionは`{"images": [...]}`で包む。包みのない形も受け付ける。
fn template_documents(output: &str) -> Result<Vec<serde_json::Value>> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if let Ok(serde_json::Value::Object(object)) =
        serde_json::from_str::<serde_json::Value>(trimmed)
        && let Some(listed) = object.get("images")
    {
        return match listed {
            serde_json::Value::Array(items) => Ok(items.clone()),
            serde_json::Value::Null => Ok(Vec::new()),
            _ => Err(unparseable("sbx template ls", "images is not a list")),
        };
    }
    json_documents("sbx template ls", output)
}

/// 同じimageを指す参照の書き方。
///
/// runtimeは`docker.io/library/`を補って表示する。sbxmが渡す名前は補われる前の
/// 表記であるため、両方を同じimageの名前として扱う。
fn reference_names(repository: &str, tag: &str) -> Vec<String> {
    let mut names = vec![format!("{repository}:{tag}")];
    for prefix in ["docker.io/library/", "docker.io/"] {
        if let Some(short) = repository.strip_prefix(prefix) {
            names.push(format!("{short}:{tag}"));
            break;
        }
    }
    names
}

fn string_field(object: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<String> {
    object
        .get(key)
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn unparseable(program: &str, detail: &str) -> Error {
    Error::new(
        ErrorId::ExternalOutputUnparseable,
        msg!(
            "error-external-output-unparseable",
            program = program,
            detail = detail
        ),
    )
}

#[cfg(test)]
#[path = "compatibility_test.rs"]
mod compatibility_test;
