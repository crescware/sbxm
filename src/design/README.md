# sbxm CLIデザインシステム

## この文書の役割

このdirectoryは、`sbxm`の利用者向けinterfaceを構成するデザインシステムを持つ。
この文書はCLI出力の恒久的な設計規約であり、新しい出力、prompt、diagnosticを追加するときの正本である。

実装はこの文書をmodule documentationとして取り込む。

```rust
#![doc = include_str!("README.md")]
```

規約は文書だけで守らない。次の三層を揃える。

- このREADMEは、意思決定とその理由を説明する
- 型とpublic APIは、正しい構造を作りやすくし、不正な構造を表現しにくくする
- invariant testとarchitecture testは、規約からの逸脱を検出する

command固有のprinterは、表示する情報と順序だけを宣言する。色、ANSI sequence、prefix、罫線、空行、terminal判定を組み立ててはならない。

## 目標

長い処理でも、利用者が次を短時間で判別できるCLIにする。

- 何を実行中か
- 何が完了したか
- 何に注意が必要か
- 何が失敗したか
- どの情報が同じsectionに属するか
- 次に何を入力するか
- promptの現在位置と選択済み項目はどれか

現代的な見た目を、派手な配色や装飾の量で作らない。少数の一貫した意味色、文字属性、記号、罫線、空白によって情報階層を作る。

## streamの責務

正常結果はstdoutへ出す。

- summary
- fields
- table
- list
- legend
- 正常終了後のguidance

実行中のfeedbackと異常はstderrへ出す。

- progress
- prompt
- warning
- error
- remediation
- 外部commandの失敗情報

色や見栄えを理由にstreamを変更しない。stdoutとstderrの責務はCLIのcontractであり、redirect、pipe、scriptからの利用を支える。

stdoutとstderrは別々にTTYを判定する。stdoutだけをpipeした場合、stdoutはplain text、terminalへ残るstderrは色付きになり得る。二つのstreamを統合したときの表示順をcorrectnessの前提にしない。

## 色の基本方針

### 色は意味に割り当てる

callerは`red`、`green`、ANSI codeを指定しない。`Role`または`VisualState`を指定し、具体的なstyleはデザインシステムだけが決める。

| 意味 | 基本表示 | 主な用途 |
| --- | --- | --- |
| 見出し | bold | section、table header、guidance、外部出力heading |
| 情報・進行中 | cyan | progress marker、現在位置 |
| 成功・健全 | green | success marker、肯定的な状態値 |
| 注意・未確定 | yellow | warning、確認や行動を要する状態値 |
| 失敗 | red | error、否定的な状態値 |
| 補助 | dim | 操作説明、凡例、時間の見込み、invocation metadata |

行全体や段落全体を着色しない。識別に必要な短いmarker、label、状態値へ限定する。通常本文はterminalの既定前景色を使う。

### ANSI標準色を使う

標準paletteにはANSIの標準16色を使う。ここでANSIとはescape sequenceという出力方式だけでなく、利用者のterminal themeが定義するnamed colorを使うという意思決定を含む。

固定したtruecolor RGBや256色paletteを標準にしない。製品固有のRGBは一見洗練された色味を作りやすいが、利用者が選んだdark/light theme、背景色、contrast設定と独立し、読みにくい組み合わせを作るためである。

terminal themeのcyan、green、yellow、redを意味色として使えば、利用者が自身の環境向けに調整したcontrastを尊重できる。truecolor対応terminalでも固定RGBへ自動昇格しない。

将来、明示的なtheme optionを設ける場合に限り、利用者が選べる追加paletteとしてtruecolorを検討できる。その場合も次を維持する。

- ANSI named colorの標準theme
- 色なし出力
- dark/light双方のcontrast test
- `Role`と`VisualState`を変えず、具体色をprivateに保つ

### 色を出す条件

color modeは`Auto`、`Always`、`Never`の三値とする。

優先順位は次のとおり。

1. 明示的な`--color=always|never|auto`
2. `NO_COLOR`が存在すれば`Never`
3. `CLICOLOR_FORCE`が`0`以外なら`Always`
4. `TERM=dumb`なら`Never`
5. `Auto``は対象streamがTTYのときだけ有効`

`NO_COLOR`は値を問わず、空文字でもopt-outとして扱う。`Always``は利用者が明示した場合だけredirect先へANSI` sequenceを出す。

CIかどうかを独自に推測しない。TTY、標準的な環境変数、明示optionに従う。

### 色へ依存しない

色は、prefix、見出し、字下げ、状態label、checkbox、空行で既に表現された意味を補強する。色を取り除いても同じ情報が残らなければならない。

- warningとerrorは色だけで区別しない
- successとfailureは色だけで区別しない
- promptの現在位置を色だけで示さない
- underlineや罫線を取り除いても意味を失わない
- 非TTYの既定出力にはANSI sequenceを含めない

## 文字属性

### bold

boldは色とは独立した情報階層として使う。見出しだけに限定しない。

使用してよい例:

- section、guidance、diagnosticのheading
- promptの現在位置
- 利用者が次に入力するcommand
- error ID、project ID、sandbox名など照合の基準となる短い値
- summaryの結論
- warningのlabel

使用しない例:

- 長い説明文全体
- tableの全cell
- 外部stderr全体
- 一画面の過半

同じblock内で何を最初に読ませるかを決め、すべてを同じ強さにしない。

### italic

italicは一律に禁止する。

アルファベットでは機能しても、日本語を含む文字体系では傾斜によって字形が崩れ、視認性とlocale間の一貫性を損なう。style modelにitalic fieldを設けず、表現不能にする。

### underline

underlineは一律禁止にしない。参照または操作可能であることを補助する場合に使ってよい。

- terminal hyperlinkとして実際に開けるURL
- promptでkeyboard操作の対象を示す短いkey label
- interactive stateで一時的な照合対象を示す場合

単なる強調、severity、path、commandへ慣例的に付けない。terminal hyperlinkでない文字列をlinkに見せない。

現在の出力はどれにも当たらないため、style modelはunderlineを持たない。実際に開けるURLを出す出力ができた時点で、`Role`と写像へ同時に足す。使い道のない語彙を先に置くと、あとから慣例的な強調として使われる。

### 背景色など

背景色、点滅は使わない。terminal themeとの衝突が大きく、情報量に対して視覚的な負荷が高いためである。

## emojiと記号

emojiは一律に禁止する。

ここでいうemojiには次を含む。

- terminalやOSによって二色以上のpictographとして描画され得る文字
- emoji variation selector
- ZWJ sequence
- regional indicatorによる国旗
- keycap sequence
- text presentationとemoji presentationが環境によって揺れる文字

現在のterminalで単色に見えることを許可理由にしない。別のOS、font、terminalでは複数色になる可能性があるためである。

一つの前景色で描画するtext symbolは使用してよい。

```text
→  ✓  ×  !  ›  │  ├  └  ─
```

symbol自身は色を持たず、rendererがsemantic foregroundを一色だけ適用する。markerと罫線はprivateな`GlyphSet`へ集約し、commandや翻訳resourceが独自の記号を追加しない。

`Unicodeを安全に表示できない環境向けにASCII` fallbackを持つ。

| 意味 | Unicode | ASCII |
| --- | --- | --- |
| progress | `→` | `>` |
| success | `✓` | `+` |
| error | `×` | `x` |
| current | `›` | `>` |
| move keys | `↑` / `↓` | `^` / `v` |

罫線は現在どの出力も使っていない。外部outputとsbxm自身の診断は四空白の字下げで分けている。所属関係が字下げと空行だけでは曖昧になる出力ができた時点で、`GlyphSet`へASCII fallbackとともに足す。

Unicode/ASCIIのどちらでも意味を変えない。表示幅は実際に選んだglyphで計算する。

## prefix

人向けの非表形式messageには、左端へ意味prefixを付ける。

```text
→ Building sandbox image
! Warning: The Dockerfile changed during the build.
× error: docker-unreachable
✓ Sandbox is ready
```

- progress markerはcyan
- warning markerとlocalized warning labelはyellow
- error markerと`error:`はred + bold
- success markerはgreen
- prefixの後は半角空白一個
- prefixやseverity labelを翻訳文へ埋め込まない

error IDは翻訳せず、既存の安定した英語IDを維持する。

## blockと改行

出力は次のblockとして構成する。

1. Progress
2. Summary
3. Section
4. Guidance
5. Command
6. Diagnostic

block間は空行一行、block内は詰める。rendererがblock境界を管理する。

- 出力の先頭に空行を置かない
- 通常は末尾を改行一個で閉じる
- callerが文字列先頭の`\n`で余白を作らない
- callerが`println!("\n...")`を使わない
- section headingと内容の間には空行を置かない
- 別sectionの前には空行一行を置く
- 空sectionは原則表示しない

### Progress

一工程一行で、連続工程の間に空行を置かない。

```text
→ Cloning repository to host
→ Building sandbox image (this may take a few minutes)
→ Creating sandbox
```

「何をしているか」を先に置き、時間の注記を同じ行の末尾へdimで表示する。工程ごとにsuccess行を追加してログ量を倍増させない。command全体または利用者に意味のある成果だけをsummaryとして示す。

progressは処理開始前にstderrへ書き、直ちにflushする。

### Summary

成功結果を可能な限り一行で示す。

```text
✓ Prepared Example-Org/Example-Repo in sbxm-example-org-example-repo-99a40327a69b
```

既存の完了文が同じ内容を示す場合はsummaryを重複させない。詳細がある場合だけ、空行を一行置いてsectionへ続ける。

### Section

```text
PROJECT
Project   Example-Org/Example-Repo
Sandbox   sbxm-example-org-example-repo-99a40327a69b

WORKTREES
Path       Mode      State
workspace  attached  running
```

headingはbold、table headerはbold + dim、field labelはdim、通常値は既定色とする。section headingの大文字化はrendererではなくlocale resourceの責務とする。

### Guidance

補足と次の行動は本文から空行一行で分ける。順序がある説明は番号付き、順序のない説明はbulletを使う。

commandは説明行へ混ぜず、専用のCommand blockにする。

### Diagnostic

一つのdiagnosticは次の内部構造を持つ。

1. error ID
2. localized description
3. 事実の行
4. remediation explanation
5. 実行を求めるcommand
6. 外部output

error headingとdescriptionの間は詰める。事実の行もdescriptionへ続けて詰める。remediationとexternal outputは別の小blockとして一行空ける。複数diagnosticの間にも空行一行を置く。

外部stderrはsbxm自身のdiagnosticと区別できるよう、四空白indentまたは罫線でまとめる。外部stderr全体を着色しない。外部outputの前後ではstyleをresetし、外部byte列がrendererのstyle stateへ侵入しないようにする。

#### 事実の行

診断が示す変数は、説明文へ埋め込まず、項目名を伴う行として並べる。

```text
× error: external-output-unparseable
  The output could not be interpreted
  Command: sbx ls
  Cause: EOF while parsing an object at line 1 column 1
```

同じ色の一文へ変数を連結すると、読み手はまず「どこが変数か」「どこで区切れるか」を探すことになる。項目名を左へ置いて行を分ければ、色を1つも足さずに境界が決まる。翻訳messageは値の連結から解放され、翻訳者はcolonの前後を組み立てずに済む。

- 項目名は`Msg`として翻訳する。末尾のcolonはlocale resourceが持つ
- 項目名はdim、値は`Inline`のvariantが決めた装飾とする
- 項目名の幅は揃えない。診断ごとに項目が変わるうえ数も少なく、揃えるほど値の左端が遠ざかる
- 値が1行に収まらない場合は、項目名だけの行に続けて四空白indentで並べる。改行を1行へ潰さない
- 複数行の値は外部が書いた原文であり、着色せず、行ごとに前後でstyleをresetする

`sbxm`自身が書いた英語の文を`detail`として値へ渡さない。値は外部が示した原文か、翻訳対象のmessageのどちらかである。

## command表示

利用者へ実行を求めるcommandは、本文、見出し、箇条書き、warning、remediationの文章中へ埋め込まない。

すべて次の不変条件に従う。

- commandは一行を占有する
- command行にはshellへ入力する文字列以外を置かない
- command行の直前と直後に空行を一行ずつ置く
- 説明文はcommandより前で完結させる
- colonでcommand行へ連結しない
- commandの後に説明が続く場合も、空行を一行置く
- 複数commandを一行に連結しない
- 一commandを一blockとする
- 番号やbulletをcommand行へ付けない
- `$` prompt、backtick、引用符、枠線をcommand行へ付けない

```text
Next
  1. Register the secret required to pass environment variables.

sbx secret set ...

  2. Prepare the sandbox.

sbxm prepare Example-Org/Example-Repo

```

command行はbold + cyanとする。色なしでも前後の空行によって本文と区別できなければならない。

実装では専用の`CommandLine`型を使う。constructorは空文字、LF、CRを拒否し、複数行commandや説明との混在を表現できないようにする。

この規則は次のすべてに適用する。

- `sbx secret`のようにsbxmが代行できない操作
- `sbxm prepare`などの後続操作
- error remediation
- 再登録、復旧、再実行

翻訳messageへ`$command`を渡し、文章内に埋め込まない。説明は翻訳resource、commandはtyped modelから渡す。

この規則は**利用者に実行を求めるcommand**だけのものである。既に実行して失敗したcommandは実行指示ではないため、独立blockにも前後の空行にもしない。診断のなかで`Command:`の事実の行として示す。行を占有させる理由は「そのまま貼り付けて実行する」ことにあり、実行済みの起動にはその理由がない。二つを同じ形で描くと、読み手はどちらを実行すべきかを区別できなくなる。

## tableと状態値

tableの幅はANSIを含まないplain cellのUnicode display widthから計算する。paddingを確定した後でcellを装飾する。

- 色のon/offで列開始位置を変えない
- 日本語の全角幅を正しく扱う
- table全体や全cellを罫線で囲まない
- zebra stripeと背景色を使わない
- pathやhashを行ごとにboldにしない
- semantic stateを持つcellだけ状態色を付ける

状態値から色を文字列で推測しない。presentation adapterが`VisualState`を明示する。

```rust
pub enum VisualState {
    Positive,
    Attention,
    Negative,
    Neutral,
}
```

同じ値でも文脈で意味が変わり得る。たとえば`stopped`は停止commandの完了結果ならpositive、稼働要件のstatusならattentionである。globalな`value -> color` mapを作らない。

## path、ID、値

文章中で照合の基準になる短い値はboldにしてよい。

- project ID
- sandbox名
- errorの主対象となるpath
- error ID

翻訳済み文字列をsubstring検索して部分装飾しない。typed fragmentとして渡せない場合は、無理に部分着色せず文章全体を既定色にする。

部分装飾が必要に見えたときは、まず値を文中から追い出せないかを検討する。値をtyped fragmentとして渡せる場所は、`Field`、`Cell`、そして診断の`Fact`である。いずれも項目名と値が構造として分かれているため、文中を検索せずに装飾を決められる。地の文へ値を混ぜたまま色で救おうとするより、行を分けるほうが色を使わずに解決する。

path、ID、利用者データへANSI sequenceを埋め込まない。

## 選択prompt

選択promptでは「候補」「現在位置」「選択済み」を異なる状態として描画する。現在位置をcursor一文字だけで示さない。

### 単一選択

現在位置の行は次の三要素を同時に使う。

- 左端の`›`
- label全体のbold + cyan
- 行末のlocalizedな`(current)` / `（現在位置）`

```text
Which project do you want to open?

  ↑/↓ Move   Enter Confirm   Esc Cancel

  owner/alpha
› owner/bravo  (current)
  owner/charlie
```

現在位置でない候補は既定色とし、dimにしない。候補の並び順は移動によって変えない。

`current`は確定済みを意味しない。Enterを押すまではfocusがあるだけである。日本語では「選択済み」ではなく「現在位置」と表記する。

確定後は選んだ値を一行の結果として残す。

```text
✓ Selected owner/bravo
```

### 複数選択

cursorとcheckboxに異なる責務を持たせる。

- `›`はkeyboard focus、つまり現在位置だけを表す
- `[x]`は選択済み、`[ ]`は未選択だけを表す
- 現在位置のlabelはbold + cyan
- 行末へlocalizedなcurrent labelを付ける
- 選択済みの`[x]`はgreen
- 未選択の`[ ]`は既定色
- currentかつcheckedなら両方の状態を同時に表示する

```text
Which projects do you want to stop?

  ↑/↓ Move   Space Toggle   Enter Confirm   Esc Cancel
  Selected: 2

  [x] owner/alpha
› [ ] owner/bravo  (current)
  [x] owner/charlie
```

選択数はzeroを含め常時表示し、toggleと同期する。画面外の候補を選択した場合も選択が残っていることを把握できる。

未選択でEnterを押した場合、説明なく同じpromptを再描画しない。

```text
! Select at least one project, or press Esc to cancel.
```

warningを表示し、現在位置を維持して候補一覧を再描画する。EscとCtrl-Cは何も変更せずexit code 130で終了する。

### prompt共通規則

- 使用できるkeyと動作を必ず対で表示する
- 操作説明をlocaleごとに翻訳する
- 色なしでもmarker、checkbox、current label、選択数で全状態を識別できる
- 狭いterminalでは状態markerを残してlabel末尾を省略する
- 利用者データへANSI sequenceを埋め込まない
- 候補が一件でも状態表現を省略しない
- 初期状態で暗黙に一件を選択済みにしない
- 単一選択の先頭に現在位置があってもEnterまでは未確定とする
- project選択と言語選択で同じthemeを使う
- yes/no、自由入力、sandbox名の完全入力をlist promptとして扱わない

prompt libraryの既定themeへ要件を合わせない。custom themeで不足する場合はrendererまたはterminal state machineを実装する。

## 罫線

罫線は一律禁止にしない。空行と字下げだけでは所属関係が曖昧になる場合に使ってよい。

使用してよい例:

- 外部command outputをsbxm自身のdiagnosticから分離する
- 複数行remediationやnoteを一blockとして示す
- interactive promptでheading、操作説明、候補一覧の境界を保つ

使用しない例:

- tableの全cellを格子で囲む
- summary一行をboxに入れる
- sectionごとに長い水平線を引く
- 画面全体を装飾目的で囲む

狭いterminalでは罫線より内容を優先する。Unicode罫線を安全に表示できない環境ではASCIIへfallbackする。

## localization

翻訳resourceは意味を持つplain textを管理する。次を含めない。

- ANSI sequence
- style属性
- prefix記号
- severity marker
- 罫線
- emoji
- 利用者が実行するcommand
- block間隔を作る先頭・末尾の空行

ただし`Warning`、`Try`、`Next`、`Selected`、`current`など、利用者が読むlabelは翻訳対象とする。

localeによってheadingの長さや語順が変わっても、block構造、semantic role、streamの責務を変えない。

## component model

デザインシステムは少なくとも次の意味型を持つ。

```rust
pub enum Role {
    Heading,
    TableHeader,
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

pub enum Block {
    Progress(Msg),
    Summary(Msg),
    Section(Section),
    Guidance(Guidance),
    Warning(Msg),
    Note(Msg),
    Command(CommandLine),
    Diagnostic(Box<Diagnostic>),
    Verbatim(String),
    Rule,
}

pub enum Fact {
    OneLine { label: Msg, value: Inline },
    ManyLines { label: Msg, lines: Vec<String> },
}
```

`Warning`と`Note`は、markerとlocalized labelを伴う一行として同じ規則で描く。severityを色だけに委ねないため、labelは翻訳対象とする。

`Fact`は診断が示す事実1件であり、翻訳する項目名と翻訳しない値を型で分ける。1行に収まるかどうかは`Fact::new`が値から決め、rendererは判断しない。rendererに判断させると、同じ値が呼び出し側ごとに違う形へ落ちる。

`Verbatim`はhelpとversionだけが使う。既に組み立てられた本文をそのまま置き、末尾の改行だけをrendererが揃える。

sectionの中身は`Fields`、`Table`、`Lines`、`Legend`、`Empty`のいずれかとする。tableとlistのcellは`Cell`とし、翻訳する項目名と翻訳しない値を型で分ける。混ぜると、状態値まで訳す経路と項目名を原文のまま出す経路の両方が作れてしまう。

具体的なterminal crateの型を`design` module外へ公開しない。callerは意味型だけを使う。

`StyleSpec`はbold、dim、underline、foregroundを表現できるが、italic、背景色、点滅、RGB、256色indexを持たない。dimは補助情報にだけ使い、`Muted`と`TableHeader`以外のroleへは付かない。

promptは`Ui::prompt()`が返す`PromptUi`だけが描く。`PromptUi`はlocaleと描画条件の写しを持ち、`Ui`を借りたままにしない。同じworkflowが進捗の報告とpromptの両方を必要としても借用が衝突しないようにするためである。

## 新しい出力を追加する手順

1. 出力先がstdoutかstderrかを決める
2. 既存のBlockまたはcomponentで表現できるか確認する
3. localized textを`Msg`として追加する
4. path、ID、stateをtyped fragmentとして渡す
5. 診断の変数は説明文へ連結せず、`Fact`の行として渡す
6. 実行commandがあれば`CommandLine`として別blockにする
7. statusには文脈に応じた`VisualState`を明示する
8. printerは`Document`を返し、直接描画しない
9. plain outputの構造testを追加する
10. 必要な場合だけstyled output testを追加する
11. 色なし、狭いterminal、別localeでも意味が維持されるか確認する

新しい色、marker、block typeをcommand固有の都合で追加しない。既存componentで不十分なら、デザインシステム全体の語彙として妥当かを先に検討する。

## 禁止事項

- `design` module外でANSI escape sequenceを生成する
- callerが具体色を指定する
- `固定RGBや256色indexを標準themeへ追加する`
- terminal capabilityによってtruecolorへ自動昇格する
- italic、背景色、点滅をstyle modelへ追加する
- emojiや複数色pictographをmarker、heading、statusへ使う
- 翻訳済み文字列をsubstring検索して部分styleを適用する
- 翻訳文へcommandを埋め込む
- commandを説明、番号、bulletと同じ行に置く
- 実行済みのinvocationを、実行を求めるcommandと同じ独立blockで描く
- 事実として示す値を説明文にも重ねて置く
- 事実の値の改行を1行へ潰す
- `println!("\n...")`でblock間隔を作る
- status文字列から色を推測する
- commandごとにprompt themeを作る
- prompt libraryの既定themeとcustom themeを混在させる
- global mutable UI stateを作る
- snapshotだけで規約を守る
- legacy rendererとdesign systemを恒久的に併存させる

## 必須テスト

### policy

- color modeの優先順位
- stdoutとstderrの独立TTY判定
- `NO_COLOR`の存在
- `CLICOLOR_FORCE`
- `TERM=dumb`
- Auto、Always、Never

### style

- semantic roleからANSI named colorへの対応
- truecolorと256色sequenceを生成しない
- italic SGRを生成しない
- style終了時にresetする
- external output後にstyleが漏れない

### glyph

- 組み込みglyphにemoji variation selectorがない
- ZWJ sequenceがない
- regional indicatorがない
- keycap sequenceがない
- `UnicodeとASCIIで意味が同じ`
- glyphを含むdisplay widthが正しい

### document

- 先頭空行がない
- block間の空行が一行
- 三連続newlineがない
- 通常の末尾改行が一個
- section headingと内容の間に空行がない
- 空sectionの扱い

### fact

- 項目名と値が一行に並ぶ
- 1行に収まらない値が項目名の下へ字下げされる
- 末尾だけの改行が形を変えない
- 値がないときに項目名の後ろへ空白が残らない
- 項目名がdim、値が`Inline`由来の装飾を持つ
- 複数行の値の前後でstyleがresetされる
- localeとcolor modeを変えても行数が変わらない

### command

- 空文字を拒否する
- LFを拒否する
- CRを拒否する
- command行の前後に空行が一行
- command行にrenderer由来のpromptやbulletがない
- localization messageへcommandが残っていない

### table

- 色のon/offで列位置が同じ
- 日本語の全角幅
- state cellだけがsemantic styleを持つ
- ANSI offでESC byteがない

### prompt

- singleのcurrent marker、style、localized label
- focusとchecked stateの独立
- current + checkedの同時表示
- selected countとtoggleの同期
- zero selection warning
- warning後のcurrent維持
- 色なしで全状態を識別可能
- Esc/Ctrl-Cによるcancel
- viewportと狭いterminal

### architecture

- 利用者向けの`print!`、`println!`、`eprintln!`が`design`外にない
- ANSI literalがpainter外にない
- terminal style crateのimportが`design`外にない
- 選択promptの生成がprompt component外にない
- FTL action/remediation messageにliteral commandがない
- command用でないFTL messageに`$command` placeholderがない

## review checklist

`新しいCLI表示をreviewするときは次を確認する`。

- 最初に読むべき箇所が明確か
- 色を消しても意味が残るか
- 色を使いすぎていないか
- boldの優先順位があるか
- italicやemojiが混入していないか
- 説明文へ変数を連結せず、事実の行へ追い出せていないか
- 実行を求めるcommandと実行済みのinvocationを描き分けているか
- commandが独立行か
- commandの前後に空行があるか
- stdout/stderrの責務が正しいか
- table幅がstyleから独立しているか
- promptのcurrentとcheckedを混同していないか
- terminal themeを尊重しているか
- 別localeと狭いterminalでも読めるか
- callerがdesign systemの責務を再実装していないか

## 完成の定義

このデザインシステムが完成した状態では、すべての利用者向け出力が`design`の意味型とpainterを通る。

- command printerは`Document`を生成する
- warningとdiagnosticは構造化される
- remediationの説明とcommandは分離される
- progressは注入可能なsinkを通る
- promptは共通themeまたは共通state machineを使う
- helpとparser errorも同じpolicyに従う
- legacyな`Reporter`、display helper、global progress stateは残らない
- architecture testが直接描画の再流入を防ぐ

このREADME、型のdoc comment、invariant testを読めば、別の設計資料がなくてもCLIの視覚言語、理由、実装方法、禁止事項を理解できることを最終条件とする。
