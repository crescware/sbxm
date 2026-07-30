# CLI出力の視覚設計

## 目的

`sbxm` の出力を、長い処理でも現在地、結果、異常、次に行う操作を短時間で判別できるものにする。
対象は人が端末で読む通常出力であり、色と改行を情報構造の補助として使う。

この設計で「垢抜けた」とは、多色で派手なことではない。少数の一貫した視覚規則により、初見でも読む順序が分かり、繰り返し使う利用者は必要な箇所だけを拾える状態を指す。

## 現状と課題

現状には次の長所がある。

- 正常結果は stdout、進捗、prompt、warning、error は stderr という責務が定義されている
- table、field、凡例といった出力部品が `Reporter` に集約されている
- 長時間処理は開始前に説明され、無反応に見える時間を減らしている
- section 間には一部空行があり、外部 command の stderr は原文の block として分離される

一方、人が見ると次の情報が同じ強さで並ぶ。

- 進捗と警告がどちらも無印の文章である
- error ID、説明、対処方法、外部出力の境界が色でも字下げでも十分に区別されない
- section heading、table header、通常値、状態値が同じ文字色とウェイトである
- path、project ID、sandbox 名、実行すべき command が文章に埋もれる
- 複数の進捗行が続くと、工程の列ではなく文章の段落に見える
- 空行の入れ方が各 command の `println!("\n...")` に委ねられ、同じ意味の block でも間隔が揃わない

問題は色不足だけではない。情報の種類を表す語彙と block 境界が不足しており、そこへ単純に色を足すと、色数が増えただけの出力になる。

## 設計原則

### 色は意味に割り当てる

同じ意味はすべての command と locale で同じ色にする。command 固有の色は作らない。
一つの行を丸ごと着色せず、識別に必要な短い token だけを着色する。

### 色がなくても構造を失わない

色は prefix、見出し、字下げ、空行で既に表現された意味を補強する。色だけで warning と error、成功と失敗を区別しない。
色を取り除いた出力も完全な情報を持ち、テスト、ログ保存、copy & paste に耐えるものとする。

### 強調に階層を持たせる

通常本文は端末の既定色のままとする。bold は見出しに限定せず、現在位置、重要な値、実行する command、診断の要点など、読み順や操作対象を明確にできる場面で使用してよい。
ただし、一画面で bold の箇所が過半を占める場合、その強調は機能していないとみなす。同じ block 内では何を最初に読ませるかを決め、すべてを同じ強さにしない。

italic は一律に禁止する。アルファベットでは機能しても、日本語を含む文字体系では傾斜によって字形が崩れ、視認性や locale 間の一貫性を損なうためである。

underline と罫線文字は一律禁止にしない。情報の境界や操作対象を、色や空行だけより明確にできる場合に使用してよい。ただし underline は severity の表現には使わず、罫線は装飾目的で画面全体を囲まない。

### 改行は block の階層を表す

空行は意味の異なる block の間に一行だけ置く。同じ block 内の連続項目には空行を置かない。
先頭に空行を出さず、通常は末尾を改行一個で閉じる。

### stdout の意味を維持する

stdout と stderr の現在の責務は変更しない。色の導入を理由に stream を移動しない。
stdout が非TTYの場合、正常結果を downstream で扱えるよう ANSI escape sequence を一切含めない。

## 視覚語彙

基本 palette は次の五つに限定する。具体的な ANSI の 256 色番号や RGB 値ではなく、端末 theme が解釈できる標準色と属性を使う。

| 意味 | 表示 | 用途 |
| --- | --- | --- |
| 見出し | bold | section heading、table header、`Next`、外部出力 heading |
| 情報・進行中 | cyan | `→` と進捗の短い動詞部分 |
| 成功・健全 | green | `✓`、`ok`、`running`、`placed` など肯定的状態値 |
| 注意・未確定 | yellow | `!`、warning、`stopped`、`missing`、`unknown` など確認や行動を要する状態値 |
| 失敗 | red | `×`、`error`、`failed`、`invalid` など失敗状態値 |

dim は補助説明、凡例、所要時間の見込み、外部 command の invocation metadata にだけ使用する。path と ID は原則として端末既定色のまま bold にする。実行を指示する command は後述する専用 block で表示する。

magenta、背景色、点滅、italic は使わない。背景色は端末 theme との衝突が大きく、italic はアルファベット以外の字形で視認性を落とす。絵文字も使わず、幅が安定した ASCII/Unicode 記号に限定する。

### bold、underline、罫線

bold は色とは独立した情報階層として扱う。次の用途では積極的に使用してよい。

- section、guidance、diagnostic の heading
- prompt の現在位置
- 利用者が次に入力する command
- error ID、project ID、sandbox 名など照合の基準になる短い値
- summary の結論や warning の label

長い説明文、table の全 cell、外部 stderr 全体は bold にしない。

underline は次のように「参照または操作可能であること」を補助する場合に使用してよい。

- terminal hyperlink として実際に開ける URL
- prompt 内で keyboard 操作の対象を示す短い key label
- 同色の値が密集し、照合対象を一時的に示す必要がある interactive state

単なる強調、warning、error、path、command へ慣例的に underline を付けない。terminal hyperlink でない文字列を link に見せないためである。

罫線文字は block の所属関係が空行と字下げだけでは曖昧になる場合に使用してよい。候補は `│`、`├`、`└`、`─` とし、次の用途を想定する。

- 外部 command の出力を sbxm 自身の diagnostic から分離する
- 複数行にわたる remediation や note を一つの block として示す
- interactive prompt で heading、操作説明、候補一覧の境界を保つ

table の全 cell を格子で囲む、summary 一行を box に入れる、section ごとに長い水平線を引く、といった常用はしない。狭い terminal では罫線より内容を優先し、Unicode を安全に表示できない環境向けには `|`、`+-`、`-` へ置き換えられる構造にする。罫線を含む行も Unicode display width で計算する。

### prefix

人向けの非表形式メッセージには、左端に固定した意味 prefix を付ける。

```text
→ Building sandbox image
! Dockerfile changed during the build
× error: SBXM-E...
✓ Sandbox is ready
```

- `→` は cyan
- `!` は yellow
- `×` は red
- `✓` は green
- prefix の後は半角空白一個
- 翻訳文中に記号や severity 名を埋め込まず、renderer が付与する

warning には記号だけでなく、localized label を付けてもよい。採用形は `! Warning: <message>` / `! 警告: <message>` とし、warning の意味を記号や色だけに依存させない。

error ID は安定した英語のまま `× error: <ID>` とする。既存の interface を維持し、`× error:` のみ red/bold、ID は bold とする。

### 状態値

状態値は文字列全体を semantic category に写像して着色する。列全体や行全体は着色しない。
category は文言から推測せず、各 enum が明示的に返す。

同じ値でも文脈で意味が変わる場合がある。たとえば `stopped` は `stop` の完了結果なら成功、稼働要件の status なら注意である。このため `value -> color` の global map は作らず、`VisualState::{Positive, Attention, Negative, Neutral}` を出力 model が指定する。

`unknown`、`unobservable` は yellow とし、検査自体が失敗して command の失敗理由となる場合だけ red にする。neutral 値は無着色とする。

### table と field

- section heading は bold。英字を大文字化するかは locale 文言の責務とし、renderer は変換しない
- table header は bold + dim
- field label は dim、値は既定色
- semantic state の cell だけを状態色にする
- zebra stripe と背景色は使わない。罫線は table の全 cell を囲まず、section や diagnostic の境界を補助するときだけ使う
- table の直前と直後に余分な空行を入れず、section 間の規則に従う

大量の行がある table は色を増やすより、列 alignment と見出しで読ませる。path や hash を行ごとに bold にするとノイズになるため、通常値のままとする。

### path と識別子

文章中で利用者が照合する値は bold にする。

- project ID と sandbox 名
- error の主対象となる path
- error ID

翻訳 layer が ANSI sequence を持つことは禁止する。message の変数を typed fragment として renderer に渡せない箇所では、無理に部分着色せず文章全体を既定色にする。翻訳済み文字列の substring 検索による着色はしない。

### 実行を指示する command

利用者に実行を求める command は、本文、見出し、箇条書き、warning、remediation の文章中へ埋め込まない。色や bold だけでは本文との境界にならないため、すべて次の不変条件に従う。

- command は一行を占有し、その行には shell へ入力する文字列以外を置かない
- command 行の直前と直後に空行を一行ずつ置く
- 説明文は command より前の行で完結させ、colon で command 行へ連結しない
- command の後に追加説明がある場合も、空行を一行置いてから本文を再開する
- 複数の command は一行に連結せず、一 command 一 block とする
- 順序がある場合、番号は command 行ではなく、その直前の説明行へ付ける
- backtick、`$` prompt、引用符、枠線など、copy 対象に含まれない装飾文字を command 行へ加えない

command 行は bold + cyan とする。ただし色なしでも空行によって本文と識別できることを必須とする。実際の入力に改行を含む複合手順は、一つの shell snippet として表示せず、複数の一行 command block または文書への案内へ分解する。

この規則は `sbx secret` のように `sbxm` が代行できない環境変数の受け渡し、`sbxm prepare` のような後続操作、error の remediation、再登録や復旧の command のすべてに適用する。実行済み外部 command の記録や table の値として command 名を表示する場合は「実行指示」ではないが、診断内では同じ視認性を得るため独立行にする。

### 選択 prompt

選択 prompt では「候補」「現在位置」「選択済み」を別の状態として描画する。現在位置を示す cursor 一文字だけに識別を依存させない。

#### 単一選択

単一選択には候補と現在位置の二状態がある。現在位置の行は、次の三つを同時に使って区別する。

- 左端の `›` marker
- label 全体の bold + cyan
- 行末の localized な `(current)` / `（現在位置）`

現在位置でない候補は、marker と状態 label を付けず端末の既定色で表示する。候補を dim にすると一覧そのものが読みにくくなるため、dim にはしない。

```text
Which project do you want to open?

  ↑/↓ Move   Enter Confirm   Esc Cancel

  owner/alpha
› owner/bravo  (current)
  owner/charlie
```

見出しは bold、操作説明は dim とする。見出し、操作説明、候補一覧の間に空行一行を置く。現在位置を移動しても候補の並び順は変えない。

`current` は「確定済み」を意味しない。Enter を押すまでは候補に focus があるだけなので、日本語では「選択済み」ではなく「現在位置」と表記する。確定後の transcript では、dialoguer の既定のように prompt と選んだ値を一行へ潰さず、次のように結果を一行残す。

```text
✓ Selected owner/bravo
```

#### 複数選択

複数選択には候補、現在位置、選択済みの三状態がある。cursor と checkbox に異なる責務を持たせる。

- `›` は keyboard focus、つまり現在位置だけを表す
- `[x]` は選択済み、`[ ]` は未選択だけを表す
- 現在位置の label は bold + cyan にし、行末へ localized な `(current)` / `（現在位置）` を付ける
- 選択済みの `[x]` は green、未選択の `[ ]` は既定色にする
- 現在位置かつ選択済みなら `›` と label は cyan、`[x]` は green のままにし、二つの状態を同時に見せる

```text
Which projects do you want to stop?

  ↑/↓ Move   Space Toggle   Enter Confirm   Esc Cancel
  Selected: 2

  [x] owner/alpha
› [ ] owner/bravo  (current)
  [x] owner/charlie
```

選択数は候補一覧の上に常時表示し、toggle のたびに更新する。これにより画面外の候補を選択した場合も、選択が残っていることを把握できる。`Selected: 0` も省略しない。

未選択のまま Enter を押した場合、現在のように説明なく同じ prompt を再描画してはならない。一覧の直上へ localized warning を一行表示する。

```text
! Select at least one project, or press Esc to cancel.
```

warning の後に候補一覧を再描画し、現在位置は維持する。Esc と Ctrl-C は引き続き何も変更せず終了する。

#### prompt 共通規則

- 操作説明は使用できる key と動作を必ず対で示し、locale ごとに翻訳する
- 色なしでも marker、checkbox、`current` label、選択数で全状態を識別できる
- terminal 幅に収まらない label は状態 marker を残したまま末尾を省略し、横折り返しによって次候補に見えないようにする
- project ID のような利用者データに ANSI sequence を埋めず、theme が行単位で装飾する
- 候補が一件でも prompt の状態表現を省略しない
- 初期状態で暗黙に一件を「選択済み」にしない。単一選択の先頭行に現在位置があっても、Enter までは未確定である
- `init` の言語選択など、project 選択以外の `Select` にも同じ単一選択 theme を適用する
- yes/no、自由入力、sandbox 名の完全入力は選択一覧ではないため、この marker/checkbox 規則の対象外とする

実装時は dialoguer の既定 theme に任せず、単一選択と複数選択で共有する custom theme または prompt renderer を用いる。theme が現在位置と checkbox を別々に style できない場合、表示要件を library の既定表現へ合わせて弱めず、renderer の差し替えを選ぶ。

## 改行と block

出力を次の block type として扱う。

1. Progress: 実行中の連続工程
2. Summary: command の結論を一行で示す
3. Section: heading と fields/table/list
4. Guidance: 注意、補足、次の行動
5. Command: 利用者が次に実行する一行
6. Diagnostic: error 一件とその対処、外部出力

block 間は空行一行、block 内は詰める。renderer が block 境界を管理し、個々の caller は文字列先頭の `\n` で余白を作らない。

### Progress

連続する工程は空行を挟まず、一工程一行にする。

```text
→ Cloning repository to host
→ Building sandbox image (this may take a few minutes)
→ Creating sandbox
```

進捗文は「何をしているか」を先にし、時間の注記は同じ行の末尾へ置いて dim にする。
完了のたびに成功行を追加してログ量を倍増させない。command 全体または利用者に意味のある成果の summary だけを `✓` で示す。

進捗から最終 summary または error へ移るときは空行一行を置く。stdout と stderr が別 stream のため、redirect や buffering によって順序が保証されない点は維持される。両者を統合した見栄えを correctness の前提にしない。

### Summary

成功 command は可能な限り最初の結果 block に一行の summary を持つ。

```text
✓ Prepared owner/repository in sandbox-name
```

その下に詳細がある場合だけ空行を一行置く。詳細のない短い command は summary 一行で終える。
既に `...done`、`...registered` などの成功文を持つ command はその文を summary とし、同じ内容を追加しない。

### Section

heading の直後に空行を置かず、内容を続ける。別 section の前には空行一行を置く。

```text
PROJECT
Project   owner/repository
Sandbox   owner-repository

WORKTREES
Path       Mode      State
workspace  attached  running
```

heading と内容を離す空行は、heading が何を指すかを弱めるため使わない。
空 section は原則表示しない。ただし「対象がゼロ」という結果自体が重要なら、localized empty-state を一行表示する。

### Guidance

補足と次の行動は本文から空行一行で分ける。説明が複数ある場合は二空白で字下げする。ただし command は字下げした説明行へ混ぜず、必ず独立した command block にする。

```text
Next
  1. Register the secret required to pass environment variables.

sbx secret set ...

  2. Prepare the sandbox.

sbxm prepare owner/repository

```

順序がある操作の説明は番号付き、順序のない説明は `-` を使う。番号や bullet は command 行に付けない。現在のような無印の字下げ行や、説明と command を一行に併記する形式は使わない。
長い security hint は `! Note:` / `! 注記:` の一 block とし、table の末尾に接着させない。

### Diagnostic

一つの diagnostic は次の内部構造にする。

```text
× error: SBXM-...
  <localized description>

  Try:
    <localized remediation>

<command to run>

  Command:

<invoked program and safe arguments>

  Directory:
    <working directory>

  Output from <program>:
    <external stderr, each line indented>
```

- error heading と説明は同じ block なので間に空行を置かない
- remediation、invocation metadata、external output は別の小 block として一行空ける
- remediation の command と、実行済み外部 command の invocation は独立行とし、前後に空行を置く
- 複数 diagnostic の間は空行一行
- 外部 stderr は各行を四空白字下げし、sbxm 自身の error と視覚的に分離する
- 外部出力の本文は着色しない。含まれていた ANSI sequence は安全方針を別途決めるまで透過しない
- external stderr が末尾改行を持たなくても、block は必ず改行で閉じる

## 色を出す条件

色 mode は `Auto`、`Always`、`Never` の三値とする。初期実装時の public option と環境変数の優先順位は次の通りとする。

1. 明示的な `--color=always|never|auto`
2. `NO_COLOR` が存在すれば `Never`
3. `CLICOLOR_FORCE` が `0` 以外なら `Always`
4. `TERM=dumb` なら `Never`
5. `Auto` は対象 stream が TTY のときだけ有効

`NO_COLOR` の値は問わず、存在を尊重する。空文字も opt-out として扱う。
色判定は stdout と stderr で別々に行う。たとえば stdout を pipe し stderr を端末に残した場合、結果は plain text、進捗と診断は colored text になる。

`Always` は redirect 先にも ANSI を出すため、利用者が明示した場合に限る。CI だから色を無効にするという独自判定は設けず、TTY と標準環境変数に従う。

初回の実装範囲で `--color` を public interface に加えない判断も許容する。その場合でも内部 model は三値にし、`Auto` と `NO_COLOR` を先に実装できる構造にする。

## 出力例

### `prepare`

```text
→ Cloning repository to host
→ Building sandbox image (this may take a few minutes)
→ Creating sandbox
→ Preparing repository inside the sandbox

✓ Prepared owner/repository in owner-repository

PROJECT
Project             owner/repository
Sandbox             owner-repository
Creation mode       attached
Managed worktrees   2
Sandbox state       running

WORKTREES
Path        Created from  Head     Mode
workspace   main          a1b2c3d  attached
workspace2  main          a1b2c3d  detached

! Note: Declared files carry configuration, not credentials.

Legend
  attached  Uses the primary working tree
  running   The sandbox is running
```

実際には `→` が cyan、`✓` と `running` が green、section heading が bold、`Note` が yellow、Legend は dim となる。plain text でも同じ階層を維持する。

### warning を伴う成功

```text
! Warning: The Dockerfile changed while the build was running.

✓ Prepared owner/repository in owner-repository

Next
  1. Apply the current Dockerfile.

sbxm rebuild owner/repository

```

warning を summary の直前に出すか直後に出すかは発生時点によるが、warning と結果 block の間には空行を置く。warning が成功を隠さず、成功色が warning を打ち消さないよう両方を残す。

### `status`

```text
GLOBAL
Item                Status
Platform            ok
Docker daemon       error
Sandbox login       unknown

Legend
  error    Could not be verified or does not meet requirements
  ok       Meets the requirement
  unknown  Could not be observed

× error: DOCKER_UNREACHABLE
  The Docker daemon did not answer.

  Try:
    Start Docker, then diagnose the global environment again.

sbxm status --global

```

`ok` は green、`unknown` は yellow、`error` と diagnostic heading は red になる。status table の結果と diagnostic は stdout/stderr に分かれるという既存仕様を維持する。

## command 別の適用方針

| command / 出力 | Summary | Section と改行 |
| --- | --- | --- |
| `init` | 既存の初期化完了文 | 次の一手を `Next` と独立 command block に分離 |
| `add` | 登録済み/登録完了文 | fields、`Next`、`sbx secret`、`sbxm prepare` をそれぞれ block 化 |
| `prepare` | 成果を一行に集約 | project fields、worktrees、files、notes、legend を独立 section 化 |
| `apply` | worktree と file の結果を必要に応じ二行 | notes、files、legend を独立 block 化 |
| `rebuild` | 既存の完了文 | warning と summary を空行で分離 |
| `status` | summary を追加せず table 自体を結論とする | global/project、worktrees、legend を section 化 |
| `ls` | summary を追加しない | managed、unmanaged、legend を section 化 |
| `stop` | table を結論とする | failure diagnostic を別 block 化 |
| `destroy` | 実行前は summary を付けない | identity、worktrees、removes、keeps、recovery を独立 section 化 |
| `open` | 短い成功なら一行 | 追加の section は作らない |

破壊操作の確認画面では green を使わない。削除対象 heading は red ではなく bold、保持対象は既定色とする。red を画面全体へ広げると error と混同し、確認内容を落ち着いて比較しにくいためである。force notice は warning として yellow にする。

## 実装モデル

本設計を実装する際は、各 command が ANSI code や先頭改行を直接組み立てない構造にする。

概念上、次の責務に分ける。

- `ColorMode`: 環境、CLI option、stream の TTY 状態から装飾可否を決定
- `Style`: heading、progress、success、warning、error、dim、state category を表現
- `BlockWriter`: block 間の空行を一箇所で管理し、先頭/末尾の余分な空行を防止
- `CommandBlock`: 説明と分離した一行 command を描画し、前後一空行を保証
- `PromptTheme`: 現在位置、選択済み、未選択、操作説明、選択数を一貫して描画
- `Reporter`: message と typed value を受け、stdout/stderr の適切な writer へ描画
- command printer: 表示する情報と順番だけを宣言

翻訳 catalog は plain text の意味を管理し、色、prefix、indent、空行を管理しない。ただし `Warning`、`Try`、`Next` のような可視 label は翻訳対象とする。

style 適用後の表示幅を table alignment に使ってはならない。alignment は ANSI を含まない元文字列の Unicode display width から計算し、cell 単位で padding を確定してから装飾する。

外部 crate を採用する場合は、次を満たすものに限る。

- nested style 後に確実に reset する
- stdout/stderr ごとに色可否を指定できる
- plain text path が ANSI sequence を生成しない
- Windows を含む対応 platform で標準色が動作する
- width 計算と文字列 localization を侵食しない

## 互換性と安全性

- plain text 時の文言、値、exit code、stream の意味は維持する
- ANSI sequence は利用者データではなく renderer だけが生成する
- project 名、path、外部 stderr に含まれる制御文字の扱いは、色導入とは別に sanitization 方針を定める
- `--help` は clap の style と二重管理にしない。通常出力と同じ color policy を注入できる場合だけ統一する
- prompt は dialoguer の描画を尊重し、Reporter の prefix を重ねない
- locale によって heading の長さが変わっても、空行規則と意味色は変えない

## テスト方針

色付き snapshot だけに依存せず、構造と policy を個別に検証する。

### unit test

- `Auto` は stream が TTY の場合だけ色を出す
- stdout と stderr の TTY 判定が独立している
- `NO_COLOR`、`CLICOLOR_FORCE`、`TERM=dumb` と明示 option の優先順位
- 各 `VisualState` が期待する style に写像される
- plain renderer は ANSI escape byte を一切含まない
- styled cell を含む table でも列位置が plain text と一致する
- renderer が italic attribute を生成しない
- underline と罫線の有無で message の意味や操作可否が変わらない
- Unicode 罫線を ASCII fallback へ置き換えても block の境界を識別できる
- `BlockWriter` は先頭空行、二重空行、末尾の過剰改行を作らない
- `CommandBlock` は command を一行に限定し、本文との前後に空行を一行ずつ作る
- localization message の本文中に実行指示の command が残っていない
- 複数 diagnostic と外部 stderr の block 境界
- 単一選択の現在位置に marker、style、localized label がすべて付く
- 複数選択で focus と checked state が独立して変化する
- 複数選択の選択数が toggle と同期する
- 色なしでも現在位置、選択済み、未選択を区別できる
- 未選択での確定は warning を表示し、現在位置を維持して再開する

### command 出力 test

既存 test の既定は `Never` とし、locale ごとの plain text snapshot を安定させる。
代表的な `prepare`、`status`、warning、error についてだけ `Always` の snapshot を追加し、ANSI sequence の位置を確認する。

stdout と stderr は別々に assertion する。統合順序を test の前提にしない。

### manual check

- dark theme と light theme
- 8 色相当の端末と truecolor 端末
- Unicode 罫線を表示できる端末と ASCII fallback
- `NO_COLOR=1`
- `sbxm status --global | less`、file redirect、stdout のみ pipe
- 日本語 locale での全角幅 alignment
- 80 column と狭い端末
- screen reader または ANSI を除去した transcript
- 単一選択で上下移動したときの現在位置表示
- 複数選択で画面外を含む複数項目を toggle したときの marker と選択数

## 受け入れ条件

- 色なしの transcript だけで progress、warning、error、section、次の操作を識別できる
- 通常本文の過半を着色しない
- bold は読み順または操作対象を示し、長い本文や table 全体へ無差別に適用しない
- italic を locale や内容にかかわらず使用しない
- underline と罫線は意味の補助として使用できるが、それらを除いても情報を失わない
- 一つの意味に複数の色を使わず、一つの色へ無関係な意味を過度に集約しない
- section 間は空行一行、section heading と内容の間は空行なしで全 command が統一される
- command 出力が先頭空行または連続三改行以上を含まない
- 実行を指示する command は常に独立行で、その前後に空行一行がある
- command 行には説明、番号、bullet、shell prompt が混在しない
- 選択 prompt は cursor の形や色だけに依存せず、現在位置を localized label でも示す
- 複数選択では現在位置と選択済みを別々の記号で同時に示す
- prompt の操作方法と現在の選択数を候補一覧の上で確認できる
- redirect と pipe の既定出力に ANSI sequence が含まれない
- `NO_COLOR` が機能する
- table alignment が色の有無で変化しない
- stdout/stderr、exit code、安定した error ID、非翻訳の状態値という既存 contract を維持する

## 導入順序

1. 色を使わず、block と改行規則、prefix、localized label を統一する
2. `ColorMode` と stream 別判定を導入する
3. progress、warning、error、heading に限定して色を付ける
4. typed な semantic state を table cell に導入する
5. custom prompt theme を導入し、単一選択と複数選択の状態表現を統一する
6. path と識別子の部分強調を、typed fragment を渡せる箇所だけに導入する
7. help と prompt の一貫性を確認し、可能な範囲で policy を共有する

この順序なら、各段階で色なし出力の改善を確認でき、翻訳文字列への後付け解析や全 command の一括変更を避けられる。
