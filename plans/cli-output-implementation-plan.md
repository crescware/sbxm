# CLIデザインシステム実装計画

## 位置付け

本書は [`cli-output-visual-design.md`](./cli-output-visual-design.md) を実装へ移すための計画である。
視覚規則を各 command の printer へ個別実装せず、最初に CLI デザインシステムを作り、以後の利用者向け出力はその public API だけから生成する。

`plans/` は実装完了後に削除され得る。したがって完了条件は「画面が設計書どおりに見える」だけではない。次を満たし、実装自体を読むと規則、禁止事項、拡張方法が分かる状態を完成とする。

- `src/ui/README.md` が視覚言語、判断理由、stdout/stderr の契約を説明する
- public type の名前が block、role、state の意味を表す
- 不正な組み合わせを API で作りにくくする
- invariant test が色、改行、command block、prompt state の規則を固定する
- command printer は内容と順序だけを宣言し、ANSI、prefix、空行を組み立てない
- 新しい出力を追加する開発者が `plans/` を参照せず実装できる

## 実装上の課題

現状の共通化は文字列整形の単位に留まる。

- `Reporter` は table、field、warning、error を描画するが、意味 role や block 間隔を型として持たない
- command printer が `println!`、`print!`、先頭 `\n` を直接使う
- `support::display` と `progress` に出力経路が分散する
- `progress` は locale だけを global `OnceLock` に保持し、stream の color policy を共有できない
- `Diagnostic.remediation` は一個の `Msg` であり、説明文と実行 command を分離できない
- 多数の FTL remediation が `{ $command }` や literal command を本文へ埋め込んでいる
- 状態値は文字列しか返さず、同じ文字列でも文脈ごとに異なる semantic state を指定できない
- project 選択と `init` の言語選択が別々に dialoguer の既定 theme を生成する
- stdout/stderr、TTY、環境変数をまとめて判定する application-level UI context がない

この状態で style helper だけを足すと、改行判断、command 抽出、prompt 表現が caller に残り、実装が再び散る。先に出力 model と renderer の境界を定める。

## 目標アーキテクチャ

top-level に `src/ui/` を置く。これは workflow helper ではなく application 全体の利用者 interface であるため、`support` の一部にはしない。

```text
src/ui/
├── mod.rs
├── README.md
├── policy.rs
├── style.rs
├── text.rs
├── document.rs
├── table.rs
├── diagnostic.rs
├── prompt.rs
├── renderer.rs
└── test.rs
```

各 file の責務は次のとおり。

| file | 責務 |
| --- | --- |
| `mod.rs` | `README.md` を module doc として取り込み、component を re-export する |
| `README.md` | 意思決定、視覚規則、禁止事項、追加手順、review観点の恒久的な正本 |
| `policy.rs` | color mode、TTY、環境変数、Unicode capability、stream 別 policy |
| `style.rs` | semantic role から端末 style への唯一の写像 |
| `text.rs` | localized text、plain value、command、path、ID など typed fragment |
| `document.rs` | summary、section、guidance、command、diagnostic など block model と間隔 |
| `table.rs` | ANSI を除く元値での幅計算、header、field、state cell |
| `diagnostic.rs` | error、warning、remediation、外部 invocation/output の構造化 |
| `prompt.rs` | 単一/複数選択 theme、操作説明、現在位置、checked state |
| `renderer.rs` | stdout/stderr writer への描画と flush。ANSI と単色glyph fallback の生成 |
| `test.rs` | module 全体にかかる invariant test |

`src/progress.rs`、`src/support/reporter.rs`、`src/support/display.rs` は移行完了時に削除する。互換 wrapper を最終構造として残さない。

## 中核となる型

以下は概念 API であり、実装時に lifetime や所有権は調整してよい。ただし責務を caller へ戻してはならない。

### `Ui`

一実行の locale、policy、stdout/stderr renderer を束ねる application service とする。

```rust
pub struct Ui<'a> {
    catalog: Catalog,
    stdout: Renderer<'a>,
    stderr: Renderer<'a>,
}

impl Ui<'_> {
    pub fn progress(&mut self, message: &Msg);
    pub fn warning(&mut self, warning: &Warning);
    pub fn error(&mut self, error: &Error);
    pub fn stdout(&mut self, document: &Document);
    pub fn prompt(&mut self) -> PromptUi<'_>;
}
```

実際の production constructor は `std::io::stdout()` と `std::io::stderr()` を使い、test constructor は独立した buffer、TTY flag、environment snapshot を注入できるようにする。

`Ui` は `Catalog` を所有するか共有し、format failure の fallback も一箇所で処理する。caller が `format_or_report` や `text_or_report` を呼ぶ経路をなくす。

### `OutputPolicy`

```rust
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

pub enum CharacterSet {
    Unicode,
    Ascii,
}

pub struct StreamPolicy {
    pub color: bool,
    pub characters: CharacterSet,
    pub width: Option<usize>,
}

pub struct OutputPolicy {
    pub stdout: StreamPolicy,
    pub stderr: StreamPolicy,
}
```

policy 判定は純粋関数へ分け、`std::env` と `IsTerminal` を直接読むのは production adapter だけにする。

優先順位は `src/ui/README.md` と test name の両方に残す。

1. 明示 `--color`
2. `NO_COLOR` の存在
3. `CLICOLOR_FORCE != 0`
4. `TERM=dumb`
5. stream ごとの TTY

初回 implementation で `--color` を公開しない場合も、parser から `ColorMode::Auto` を渡す形にする。後から option を追加するとき policy の署名を変えない。

`CharacterSet` の自動判定を locale 名だけで推測しない。初期値は Unicode とし、`TERM=dumb` または明示的な test/将来 option で ASCII を選べるようにする。罫線は意味の必須要素にしない。

### semantic style

caller に色名を公開しない。

```rust
pub enum Role {
    Heading,
    ProgressMarker,
    SuccessMarker,
    WarningMarker,
    ErrorMarker,
    Command,
    Important,
    Muted,
    Link,
    PromptCurrent,
    PromptChecked,
}

pub enum VisualState {
    Positive,
    Attention,
    Negative,
    Neutral,
}
```

`Role -> StyleSpec` と `VisualState -> StyleSpec` の写像は `style.rs` にだけ置く。
command printer、status enum、prompt は `green`、`cyan`、ANSI code を知らない。

`StyleSpec` は `bold`、`underline`、foreground を持てるが `italic` field を持たせない。禁止事項を comment で守るのではなく、表現不能にする。背景色と点滅も model に追加しない。

underline は `Role::Link` と interactive な短い操作対象に限る。罫線は style ではなく `CharacterSet` が選ぶ glyph set とする。

foreground は ANSI 標準16色の named color だけを表現する。RGB tuple、256色 index、terminal capability に応じた自動的な truecolor 昇格を初期 model に入れない。これは機能不足ではなく、利用者の terminal theme と contrast 設定を尊重するための意図的な制約である。

将来の明示 theme option を妨げないよう、具体色の写像は private に保つ。ただし拡張時も `Role` と `VisualState` は変更せず、標準 theme は ANSI named color のまま維持する。

marker と罫線は private な `GlyphSet` に集約する。

```rust
struct GlyphSet {
    progress: &'static str,
    success: &'static str,
    warning: &'static str,
    error: &'static str,
    current: &'static str,
    vertical_rule: &'static str,
}
```

glyph はASCIIまたは単色のUnicode text symbolだけを許可する。emoji variation selector、ZWJ、regional indicator、keycap sequenceなど、二色以上のpictographとして描画され得るemojiを定数へ追加しない。Unicode/ASCIIのどちらも意味を同じにし、rendererがsemantic foregroundを一色だけ付ける。

### typed text

翻訳済みの一文字列へ substring style を当てない。意味の違う fragment を先に分ける。

```rust
pub enum Inline<'a> {
    Text(Cow<'a, str>),
    Important(Cow<'a, str>),
    Path(Cow<'a, str>),
    Id(Cow<'a, str>),
    State {
        text: Cow<'a, str>,
        state: VisualState,
    },
}
```

実行指示 command は `Inline` に入れない。専用型にする。

```rust
pub struct CommandLine {
    value: String,
}

impl CommandLine {
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidCommandLine>;
}
```

constructor は空文字、`\n`、`\r` を拒否する。command 行へ説明や複数行 snippet を入れられないことを型で保証する。renderer は `$`、backtick、番号、bullet を足さず、command の前後へ空行一行を作る。

secret を含む command は既存どおり生成時点で redact/safety を保証する。`CommandLine` は secret sanitizer ではなく表示構造であることを doc に明記する。

### `Document` と block

```rust
pub struct Document {
    blocks: Vec<Block>,
}

pub enum Block {
    Progress(Progress),
    Summary(Summary),
    Section(Section),
    Guidance(Guidance),
    Command(CommandLine),
    Diagnostic(DiagnosticView),
    Rule(Rule),
}
```

`DocumentBuilder` は `println!("\n...")` の代わりに block を追加する。renderer は block 間へちょうど一個の空行を入れ、先頭空行と過剰な末尾改行を作らない。

同じ種類を一 block にまとめるため、次の builder を用意する。

```rust
Document::new()
    .summary(msg)
    .fields(section_heading, fields)
    .table(section_heading, table)
    .guidance(heading, paragraphs)
    .command(command)
    .legend(entries);
```

builder の `.command()` は常に独立 block となる。`.paragraph()` や localized `Msg` に command value を渡す API は提供しない。

`Progress` は stderr の連続工程であり、工程間に空行を入れない。通常の `Document` とは別に `Ui::progress()` が直ちに一行描画・flush する。最終 summary/error の前の空行は stderr renderer の block state が管理する。

### table

既存 `support::width` の Unicode display width と padding logic は `ui::table` へ移すか private helper として利用する。style 適用前の plain cell で幅を決め、padding 後に cell fragment を装飾する。

```rust
pub enum Cell {
    Text(String),
    State { text: String, state: VisualState },
}

pub struct Table {
    headers: Vec<Msg>,
    rows: Vec<Vec<Cell>>,
}
```

table renderer は次を保証する。

- header は role として bold + muted
- state cell だけ semantic color を持つ
- 全 cell を罫線で囲まない
- section 境界に罫線を使う判断は `Document` 側に置く
- ANSI on/off で列開始位置が一致する
- 日本語の全角幅を維持する

既存の `StatusValue::as_str()` だけでは文脈を表せないため、row 作成時に `VisualState` を明示する。各 domain enum に UI color を直接持たせず、command の presentation adapter で `value + semantic state` に写像する。domain layer を端末表現へ依存させないためである。

## warning と diagnostic の構造化

### `Warning`

warning を `Msg` 一個で扱わず、必要なら後続 action を保持できる構造にする。

```rust
pub struct Warning {
    pub description: Msg,
    pub guidance: Vec<Msg>,
    pub commands: Vec<CommandLine>,
}
```

単純 warning は description だけを持つ。command を含む場合も、renderer が必ず独立 command block にする。

既存 API を一度に変えるための機械的 wrapper `Warning::text(Msg)` は許容するが、移行完了後も `print_warning(&Msg)` は残さない。

### remediation

`Diagnostic.remediation: Option<Msg>` を次へ変更する。

```rust
pub struct Remediation {
    pub explanation: Vec<Msg>,
    pub commands: Vec<CommandLine>,
}

pub struct Diagnostic {
    pub id: ErrorId,
    pub description: Msg,
    pub remediation: Option<Remediation>,
    pub external: Option<ExternalFailure>,
}
```

builder は用途を表す。

```rust
Diagnostic::new(id, description)
    .explain(msg!("remediation-start-docker"))
    .run(CommandLine::new("sbxm status --global")?);
```

「同じ command をもう一度実行する」のように exact command が model に存在しない案内は曖昧なまま表示しない。呼び出し元が再実行 command を安全に構成できる場合は `CommandLine` として渡す。構成できない場合は「前の command を再実行」の説明だけを出し、架空の argv を生成しない。

FTL は説明だけを持つように分割する。

変更前:

```ftl
remediation-sandbox-not-created = Run { $command } to build the sandbox.
```

変更後:

```ftl
remediation-sandbox-not-created = Build the sandbox.
```

command 自体は Rust model から独立 block として渡す。英語と日本語の両方で command placeholder を除き、literal な `sbxm ...`、`sbx ...`、`git ...` も同じ基準で棚卸しする。

### external failure

外部 invocation は利用者への実行指示ではないが、一行の typed command として diagnostic 内で独立表示する。

- heading
- 空行
- program と safe args だけの一行
- 空行
- working directory
- 空行
- 罫線付きまたは四空白 indent の外部 stderr

外部 stderr に含まれる ANSI/control sequence はそのまま style として解釈しない。既存 byte preservation の契約と control-character sanitization の関係を先に test で固定する。sanitization が本作業の scope を超える場合、少なくとも design renderer の style stateへ侵入しないよう、外部 output の前後で必ず reset する。

## prompt system

### 共通入口

project 選択と `init` の言語選択が直接 `dialoguer::Select::new()` を呼ばないようにする。

```rust
impl PromptUi<'_> {
    pub fn select_one(
        &mut self,
        heading: Msg,
        candidates: &[PromptCandidate],
    ) -> Result<usize>;

    pub fn select_many(
        &mut self,
        heading: Msg,
        candidates: &[PromptCandidate],
        require_one: bool,
    ) -> Result<Vec<usize>>;
}
```

`support::select::TerminalProjectPrompt` と `commands::init::TerminalPrompt::select_language` はこの API を呼ぶ adapter にする。最終的には prompt trait が `Ui` を受けるか、`PromptUi` を注入する。

### 描画状態

```rust
pub struct PromptRow<'a> {
    pub label: &'a str,
    pub current: bool,
    pub checked: Option<bool>,
}
```

renderer の invariant:

- single: current row に `›`、bold + cyan、localized current label
- multi: `›` は current だけ、`[x]`/`[ ]` は checked だけ
- current + checked の同時状態を失わない
- selected count は zero を含め常時表示
- 操作 key と action を localized text で表示
- zero selection で Enter の場合、warning を表示し current index を維持
- Esc/Ctrl-C は exit code 130 の既存契約を維持
- color off でも marker、checkbox、label、count が残る
- 狭い端末では marker/state を残し label の末尾を省略する

dialoguer `0.12` の custom theme で、current suffix、selected count、zero-selection warning、現在位置維持のすべてを実現できるかを最初の spike で確認する。theme API だけで不足する場合は次のどちらかを選ぶ。

1. dialoguer の key handling を利用し、描画を custom `Term` loop に置き換える
2. `console::Term` を直接使い、single/multi 共通の小さな state machine を実装する

見た目を library default に合わせて要件を弱めない。key handling を自作する場合は、terminal clear/redraw、上下端の wrap 方針、interrupt、候補数が terminal 高さを超える場合の viewport を testable state machine と端末 adapter に分離する。

### text input と confirm

`Input`、`Confirm`、sandbox 名の完全入力は list prompt theme の対象外だが、heading、操作説明、cancel、color policy は同じ `PromptUi` から得る。dialoguer を残す場合も共通 theme と stderr policy を明示的に渡し、default style を混在させない。

## terminal style の実装手段

既に `dialoguer` が `console 0.16` を使用しているため、まず `console` を direct dependency として宣言し、style と terminal operation を同じ version で利用する案を検証する。transitive dependency の re-export へ design system を依存させない。

採用前に小さな spike test で次を確認する。

- stream ごとの force/disable color
- bold、underline、foreground、reset
- foreground が標準ANSI named colorであり、RGB/256色 sequenceへ自動変換されない
- 組み込み glyph が emoji presentation、ZWJ sequence、regional indicator、keycap sequence を含まない
- Windows を含む supported platform
- captured writer に対する deterministic output
- `NO_COLOR` 等の判定を library 任せにせず `OutputPolicy` から制御可能
- Unicode/ASCII glyph を renderer 側で選択可能

満たせない場合は `anstyle`/`anstream` を比較する。どの crate を選んでも public API は `Role` と `VisualState` のままとし、crate の style type を `ui` 外へ出さない。

## application への注入

### startup

`main::run` は locale と interactivity を解決した後に `Ui` を一度だけ構築する。

```rust
let policy = OutputPolicy::detect(color_mode, &environment, &terminals);
let mut ui = Ui::terminal(display_locale, policy);
```

config location discovery のように locale 決定前に起きる error には、source locale と同じ policy で一時 `Ui` を構築する。

`Context` は `display_locale` の代わりに、またはそれと併せて `&mut Ui` を command dispatch へ渡す。Rust の borrow を単純に保つため、次のどちらかへ統一する。

- `dispatch(command, context, &mut ui)`
- `Context { ui: &mut Ui, ... }`

後者で workflow object にまで `Context` を広げない。推奨は明示引数の前者である。

### progress dependency

global `progress::install/step` は削除する。深い workflow へ concrete `Ui` を渡し続ける代わりに、狭い trait を注入する。

```rust
pub trait ProgressSink {
    fn step(&mut self, message: Msg);
}
```

production は `Ui`、test は no-op または recorder を実装する。`support::image`、`sandbox`、`template`、`repository`、`inventory` と `add::host_clone` の計11 call site に `&mut dyn ProgressSink` を渡す。

これにより locale/color の global state をなくし、parallel test と将来の library 利用でも出力が混線しない。workflow test は「何を報告したか」、renderer test は「どう描画したか」を別々に検証する。

### command printer

command printer の署名は raw `Catalog` ではなく `Ui` または `Document` builder を受ける形へ統一する。

推奨:

```rust
pub fn document(output: &AddOutput) -> Document;
```

exec layer:

```rust
let document = print::document(&output);
ui.stdout(&document);
```

localized `Msg` を `Document` に保持し、render 時に `Ui` の catalog で format する。printer が locale と terminal を知らないため、presentation structure を unit test できる。

warning/error は stdout document と混ぜず、exec layer が `ui.warning()`、`ui.error()` で stderr へ渡す。

## command ごとの移行

共通 module の invariant test が通った後、次の順に移す。各段階で旧 printer と新 renderer を混ぜず、一 command の全出力経路をまとめて移す。

### 1. `status` と `ls`

最初に read-only command を移し、section、table、legend、semantic state を検証する。

- `status --global`
- project `status`
- `ls`
- status diagnostic の stderr

ここで `Cell::State` と文脈別 `VisualState` mapping を完成させる。

### 2. `init` と `add`

summary、field、guidance、独立 command block を移す。

- `init-next-step` を説明と exact command に分割
- `add` の token 説明、`sbx secret`、`sbxm prepare` を別 block 化
- command 行の前後空行 invariant を end-to-end test
- `init` の言語選択を common prompt へ移す

### 3. `prepare`、`apply`、`rebuild`

長時間 progress、warning、notes、複数 table、legend を移す。

- global progress を `ProgressSink` へ置換
- `Note` を `Guidance` model へ変換
- file secret hint を独立 note block 化
- warning 内の follow-up command を構造化

### 4. `open` と `stop`

single/multi project prompt と短い結果を移す。

- common prompt の single/multi state
- selected count と empty confirmation warning
- `open` の案内 command/進捗を既定 stream policy に統一
- `stop` の結果 state と failure diagnostic

### 5. `destroy`

最後に破壊操作を移す。確認前 plan と実行後結果を別 document として作る。

- removes、keeps、recovery を独立 section 化
- re-register command を独立 command block 化
- force notice を warning model 化
- deletion plan で green を使わない
- sandbox 名完全入力は既存 safety contract を維持し、heading だけ common style にする
- external `sbx rm` confirmation と sbxm 自身の confirmation を混同しない

### 6. parser help/version/error

clap help と parser diagnostic を最後に統一する。

- help text の style が `OutputPolicy.stdout` に従う
- parser error は通常の `DiagnosticView` を通る
- version は plain one-line result のまま
- clap が生成する ANSI と design system の ANSI を二重にしない

## command 埋め込みの全件移行

移行中に漏れを防ぐため、次を repository-level test または lint script として追加する。

- `locales/*.ftl` の action/remediation message に `sbxm `、`sbx `、`git ` の literal がない
- command 用ではない message が `$command` placeholder を持たない
- Rust の利用者向け出力に `println!`、`print!`、`eprintln!` がない
- `\n` で始まる利用者向け format string がない
- ANSI escape literal が `src/ui/renderer.rs` 以外にない
- `dialoguer::Select`、`dialoguer::MultiSelect` の construction が `src/ui/prompt.rs` 以外にない
- terminal style crate の import が `src/ui/` 外にない

文字列検索だけでは `docker` のような単語と command の区別を完全にはできない。lint は禁止 API と明確な prefix を検出し、FTL review test では action message ID の allowlist を持たず、typed remediation へ移す。

## 恒久READMEとmodule doc

`src/ui/README.md` を規約の単一正本とし、`src/ui/mod.rs` は次のようにそのままRustdocへ取り込む。

```rust
#![doc = include_str!("README.md")]
```

同じ規約をREADMEとRust commentへ複製しない。READMEは少なくとも次を説明する。

1. 正常結果は stdout、progress/prompt/warning/error は stderr
2. 色は意味 role に対応し、色なしでも同じ情報を持つ
3. 標準 palette は terminal theme が定義する ANSI named color であり、固定RGBではない
4. block 間は空行一行、block 内は詰める
5. command は専用型で一行、前後一空行
6. current と checked は別状態
7. bold は情報階層に利用可、italic は表現不能
8. underline と罫線は補助であり、除いても意味を失わない
9. emoji は禁止し、marker と罫線には単色の text symbol だけを使う
10. ANSI/terminal crate を module 外へ漏らさない
11. localized text に style code と command を埋め込まない
12. table 幅は plain text で計算する

各型の doc comment は「何をするか」だけでなく、「なぜ caller が直接文字列を作らないか」を一文で示す。

良い例:

```rust
/// 利用者がそのままshellへ入力する一行。
///
/// 説明文との混在を型で防ぎ、rendererが前後一空行を保証する。
pub struct CommandLine { ... }
```

悪い例:

```rust
/// commandを表示する。
pub struct CommandLine { ... }
```

## テスト戦略

### pure model test

- `CommandLine` が空、LF、CR を拒否
- `Document` が block の順序を保持
- semantic state mapping の全 variant
- prompt state machine の移動、toggle、confirm、cancel、viewport
- `OutputPolicy` の全優先順位

### renderer golden test

同じ `Document` を次の matrix で描画する。

| locale | color | character set | stream TTY |
| --- | --- | --- | --- |
| en | never | Unicode | false |
| ja | never | Unicode | false |
| en | always | Unicode | false |
| ja | always | Unicode | false |
| en | never | ASCII | false |
| en | auto | Unicode | true/false |

全組み合わせを全 command で snapshot にせず、design system の代表 document で matrix を網羅する。command ごとは plain output と重要な styled case に限定し、snapshot 爆発を避ける。

golden assertion に加え、構造 assertion を持つ。

- ANSI off で ESC byte がない
- 標準 theme が truecolor または256色の escape sequenceを生成しない
- italic SGR がない
- 組み込み marker がvariation selectorや複数code pointのemoji sequenceを含まない
- command の直前直後が空行一行
- 先頭空行、三連続 newline がない
- color on/off で visible width が同じ
- external output 後に style reset

### domain/presentation test

各 command の `print::document()` は `Document` の構造を検査する。

- section heading ID
- row と cell semantic state
- guidance と command の分離
- 空 collection の section を出すか省くか
- warning と summary が別 stream model

これにより翻訳文の微修正で構造 test が壊れず、構造変更が snapshot の目視だけに埋もれない。

### integration test

- stdout pipe + stderr TTY
- stdout/stderr redirect
- `NO_COLOR` が存在する場合
- `CLICOLOR_FORCE=1`
- `TERM=dumb`
- 日本語 locale の table alignment
- single/multi prompt の captured terminal transcript
- Ctrl-C/Esc の exit code 130
- zero selection warning と current 維持

environment を変更する test は process-global race を避け、subprocess test または environment snapshot 注入で行う。

## 段階ごとの完了条件

### Phase 0: dependency spike

- style/terminal crate を決定
- dialoguer custom theme の限界を確認
- decision を `src/ui` の doc と dependency comment に反映
- spike code を production に残さない

### Phase 1: design system core

- `src/ui` の全中核型、policy、renderer、READMEを取り込むmodule doc
- plain/color/ASCII の invariant test
- `CommandLine` と block spacing
- production ではまだ旧 printer を利用してよい

### Phase 2: structured diagnostics and progress

- `Remediation`、`Warning`、`ProgressSink`
- FTL command 埋め込みを構造化
- global progress state の削除
- error/warning/progress が新 renderer のみを使用

### Phase 3: command output migration

- 全 command printer を `Document` producer 化
- direct print macro を利用者向け出力から除去
- table/state/legend/note/guidance を共通 component 化

### Phase 4: prompt migration

- single/multi/common operation guide
- current/checked/count/empty warning
- init language selection と project selection の統一
- input/confirm の共通 policy

### Phase 5: help and cleanup

- help/version/parser error の policy 統一
- 旧 `Reporter`、`display`、`progress` の削除
- compatibility wrapper と dead FTL message の削除
- repository-level architecture test

### Phase 6: documentation deletion readiness

次を満たした時点で `plans/cli-output-visual-design.md` と本書を削除できる。

- `src/ui/README.md` と、それを取り込んだ `src/ui` のRustdocだけで原則とcomponentの入口が分かる
- public type doc から command、block、style、prompt の使い分けが分かる
- test 名から禁止事項と fallback contract が分かる
- 全 command が design system API だけを使用する
- architecture test が direct output/style/prompt construction の再流入を防ぐ
- repository rootのREADMEには利用者向けの`--color`/`NO_COLOR`の説明だけを追加し、内部規約は`src/ui/README.md`を正本とする

## 実装時に避けること

- 各 printer に `if color { ... }` を置く
- ANSI code を FTL や domain value に含める
- emoji、emoji variation selector、複数色pictographをmarkerやheadingへ使う
- 翻訳済み文字列から command、path、ID を substring 検索する
- `println!("\n...")` で block spacing を調整する
- status string から色を推測する global map
- global mutable UI や test 間で共有される terminal state
- command 表示のために説明文を改行文字で分割する
- dialoguer default と custom prompt を command ごとに混在させる
- すべての output を box や罫線で囲む
- snapshot だけで design invariant を守る
- 移行完了後も旧/new renderer を併存させる

## 推奨するPR分割

各 PR は buildable かつ testable に保ち、同じ file を長期間二方式で描画しない。

1. `Add CLI design system core`
2. `Structure CLI diagnostics and progress`
3. `Render status and listings through UI documents`
4. `Render setup commands through UI documents`
5. `Render operational commands through UI documents`
6. `Unify interactive selection prompts`
7. `Apply UI policy to help and remove legacy output`
8. `Make CLI design invariants self-documenting`

最後の PR は新機能を足すためではなく、恒久READMEを取り込むmodule doc、architecture test、命名、dead helper の除去をreviewするために独立させる。ここを完了させて初めて、`plans/` を削除しても設計が失われない。
