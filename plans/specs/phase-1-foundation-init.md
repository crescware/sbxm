# Phase 1 実装仕様: 共通基盤、`init`、global `status`

## 1. 目的と完了境界

Phase 1は`sbxm init`と`sbxm status --global`を完成させ、この2 commandが必要とする共通型、永続化、外部command実行、翻訳、CLI契約を実装する。

Phase 1のcommand自体はprojectやSandboxを作成しない。

共通基盤は呼び出し側が現れたPhaseで作る。

- 最初に呼び出し側が現れるPhaseが実装する
- 定義できる最も早いPhaseではない
- 複数Phaseで使うものは、最初の呼び出し側のPhaseが実装する
- 後続Phaseは実装済みのものを利用する

この規則は、呼び出し側のない実装がreview対象から検証基準を奪うため置く。実際の必要から形が決まる前に型やpolicyを確定させない。

後続Phaseの調査やlocal実装はPhase 1 PRのreviewと並行できるが、後続PRはreview結果を取り込む。

## 2. 成果物

```text
Cargo.toml
src/
├── main.rs
├── cli.rs
├── config.rs
├── error.rs
├── i18n.rs
├── project.rs
├── paths.rs
├── command.rs
├── compatibility.rs
└── workflow/
    ├── mod.rs
    ├── init.rs
    └── status_global.rs
locales/
├── README.md
├── en.ftl
└── ja.ftl
tests/
└── snapshots/
    └── cli-surface.txt
```

主なdependencyは`clap`、`serde`、`toml`、`dirs`、`fluent-bundle`、`unic-langid`、`dialoguer`、`serde_json`、`tempfile`とする。dependency versionは実装PRでlock fileとともに固定する。

## 3. 共通domain型

stringを直接workflowへ渡さず、validation済みの型を使う。

```text
ProjectId(String)
AbsoluteBasePath(PathBuf)
Locale { En, Ja }
HostFileSource(PathBuf)
SandboxHomeRelativePath(PathBuf)
```

`ProjectId::parse`と`AbsoluteBasePath`の導出は方向性文書の規則を唯一の実装とする。

`ProjectId`は表記をそのまま保持する。case非依存の比較とcanonical形式は、案件を突き合わせる`add`が必要とするため、Phase 2で追加する。

Sandbox名の導出とmanaged worktree pathは、それぞれ最初の呼び出し側があるPhase 2で定義する。

## 4. CLI parse

Phase 1で9 commandと全optionをparserへ登録する。Phase 1では`init`と`status --global`を実装し、`status <project>`を含む未実装処理はparse後にlocalizedな`not implemented in this build`を返してexit code `1`とする。これによりhelpとusageの翻訳をPhase 1で固定する。

CLIの公開契約は、翻訳文を含まない記録1枚として`tests/snapshots/cli-surface.txt`へ固定する。記録はparserを内省して作り、command名、option名、short、value name、arity、必須性、並び順だけを持つ。localeに依存しないため、言語を増やしても変わらない。契約を変えるときはこの記録の差分をreviewする。

validation順:

1. syntaxとoption関係
2. `--lang`
3. command固有の引数
4. config load
5. project解決
6. 外部command
7. mutation

helpとusageを構築する前に、argvから`--lang`だけを副作用なく先読みする。CLI parser libraryの自動help・自動終了へlocale決定を委ねず、選択したlocaleでhelp、usage、parse errorを生成する。

`add --worktrees`が1以上32以下でない場合、または`add --worktrees >= 2`かつ`--detach`なしはconfigやfilesystemを読む前にexit code `1`とする。

`init`は次の2 modeとする。

- 対話mode: `--base-path`、`--git-user-name`、`--git-user-email`を1つも指定しない
- option mode: 上記3 optionをすべて指定する

3 optionの一部だけを指定した場合は、TTYかどうかやconfigの有無にかかわらず、不足optionを表示してconfigやfilesystemを読む前にexit code `1`とする。option modeではpromptを表示しない。global optionの`--lang`はmode判定へ含めず、対話modeとoption modeのどちらでも独立して表示言語を指定できる。

## 5. Locale決定

優先順位:

1. 有効な`--lang <tag>`
2. 有効なglobal configの`language`
3. `init`実行時だけmacOS優先言語
4. shell locale
5. 正本locale

`init`と`status --global`以外のcommandでconfigが存在しない場合は、`sbxm init`を案内してexit code `1`とする。error表示はbootstrap localeを使う。

helpとusageのlocaleは次の順で決定する。

1. argvから先読みした有効な`--lang <tag>`
2. read-onlyかつbest-effortで読み込めた有効なglobal configの`language`
3. shell locale
4. 正本locale

- `--lang`が不正な場合はconfigを読まず、shell localeまたは正本localeでparse errorを表示してexit `1`
- configが不在の場合はshell localeへfallbackする
- configが構文不正、未知version、permission不正、symlink、またはread失敗の場合もshell localeへfallbackし、help表示自体は妨げない
- `--help`とcommand別helpは、config不正だけを理由に失敗させずexit `0`
- help以外の通常commandは、parse成功後のconfig loadで同じconfig不正を診断してexit `1`
- argv先読みはlocale選択だけに使用し、ほかのargument validationやcommand実行を行わない

macOS優先言語は`defaults read -g AppleLanguages`の出力をparseする。先頭のlanguage tagを組み込みlocaleのtagと突き合わせ、一致した場合だけ推測を確定させる。新規作成へ進む対話modeで推測が正本locale以外なら、TTY上で言語を選択させる。選択肢は組み込みlocaleの全体とし、各言語の名称はその言語自身のresourceから取る。option modeではpromptを表示せず推測をそのまま使う。一致しない、command失敗、またはparse失敗の場合だけ`LC_ALL`、`LC_MESSAGES`、`LANG`の順にfallbackする。

新規作成へ進む対話modeの`init`はstdinとstderrの両方がTTYであることを必須とする。どちらかがTTYでなければ何も作成せずexit code `1`とする。既に有効なconfigがある場合はTTYかどうかに関係なくno-op成功とする。option modeはTTYかどうかに関係なく実行できる。

## 6. FTL契約

- message IDは意味と用途を表すkebab-case
- 正本localeを`en`とし、全localeのID集合とplaceholder集合を正本と完全一致させる
- help、usage、prompt、正常出力、warning、errorをFTLから生成する
- format失敗は対象message IDとlocaleを示してexit code `1`
- 外部stderrをFTL placeholderへ埋め込まず、localized説明とは別blockで出す
- security messageは`description`と`remediation`の2 IDを必須とする

言語ごとの内容は`locales/<tag>.ftl`だけが、言語ごとの同一性は`src/i18n.rs`の定義表だけが
持つ。実装は特定の言語を名指しで分岐しない。凡例の要否のような言語別の振る舞いは、正本
localeとの関係から導出する。resourceの規約は`locales/README.md`が1箇所で持ち、resourceへ
コメントと見出しを書かない。言語を増やすときに触るのは、resource 1枚と定義表の1行だけと
する。

testではFTL parse、ID一致、placeholder一致、resourceがコメントを持たないこと、全localeで
全commandのhelpが成功すること、全localeで利用者向けslotがresourceで埋まること、代表的な
error snapshotを検証する。検査対象のlocaleは`locales/`から決め、testへ言語名を列挙しない。

利用者向け出力の文言はresourceが正本であるため、localeごとの出力をsnapshotとして複製
しない。文言のreviewはresourceに対して行う。

## 7. Atomic file write

configは次の手順で新規作成する。

1. 同一directoryに`create_new`で一時fileを作る
2. 必要permissionを設定する
3. 全内容を書いて`sync_all`する
4. 既存targetがないことを再確認する
5. renameする
6. 親directoryを`sync_all`する

symlinkは拒否する。秘密情報を一時fileへ書かない。

processが中断した一時fileは次回起動時に自動削除せず、pathと安全な削除方法を表示してexit code `1`とする。

既存fileの置き換えはPhase 1に呼び出し側がない。configの変更は利用者が直接編集し、次回load時にvalidationする。置き換え手順は、metadataを更新するPhase 2で定義する。

## 8. Config loadとvalidation

### 8.1 不在

- `init`: 新規作成へ進む
- `status --global`: configを`missing`として診断し、`sbxm init`を案内する
- その他: `sbxm init`を案内してexit code `1`

### 8.2 有効

`init`は初期化済みであることとconfig pathを表示し、何も変更せずexit code `0`で終了する。host環境の診断は`status --global`で行う。

### 8.3 無効

構文不正、未知version、必須値欠落、permission過剰、symlink、relative base pathはpathと原因を示してexit code `1`。`init`も自動修復・上書きしない。

`base_path`はstandardizeしたabsolute pathとして保存する。存在しなければ`init`が確認後に作成する。既存ならdirectoryであり、利用者がwrite可能であることを確認する。

## 9. 外部command runner

runner input:

```text
program
args[]
environment policy
timeout class
```

規則:

- shellを介さない
- defaultで現在processのenvironmentを継承する
- security-sensitiveな`sbx`起動では`SSH_AUTH_SOCK`を必ず除外する
- secret値をargumentやdebug表示へ渡さない
- stdoutとstderrを別々にbyte列としてcaptureする
- stderrをlossy変換した場合はその事実を診断する
- 未検証の外部出力を利用者へそのままstreamしない
- timeout時はchildを終了し、command名とtimeoutを表示してexit code `1`
- testではfake executableをPATH先頭へ置き、program、args、environment、streamを記録する

timeout既定値:

| Class | Timeout |
|---|---:|
| probe | 10秒 |
| local filesystem/Git | 60秒 |

Phase 1が実行する外部commandは、すべてstructured outputまたは短いtextを読むread-only probeである。

- 人間向け進捗をそのまま転送する`passthrough`は、`docker build`を実行するPhase 2で定義する
- terminalを引き渡す`inherit`と対話用のtimeout classは、SSHを起動するPhase 3で定義する
- 作業directoryの指定は、host cloneを操作するPhase 2で定義する

## 10. Docker Sandboxes CLIの出力解釈

Docker Sandboxes CLIはEarly Accessである。Phase 1は`status --global`が読む範囲だけを実装する。

- `sbx version`からexact versionを検出する
- `0.37.0`未満はexit code `1`
- versionをparseできない場合もexit code `1`
- `sbx daemon status`の`Status:`行と、`sbx policy ls`の出力から現在値を読む

parserは推測で補完しない。

- 必須fieldを欠く出力はparse不能として扱う
- 未知のstate値を既知の値へ丸めない
- 現在のnetwork policyを一意に特定できない出力を拒否する
- parse不能はerrorとし、観測できなかった項目を成功扱いしない

採取済みfixtureをversionごとに束ねる仕組みは持たない。実機出力に対する検証は、その出力を最初に読むPhaseが自身のPRで行う。mutationを行うPhase 2以降の規則は、Phase 2仕様が定める。

## 11. `sbxm init`

### 11.1 事前状態

configがない場合だけ新規作成する。既存の有効configは再利用し、無効configは停止する。

### 11.2 排他

configをread-onlyで事前確認し、新規作成へ進む場合だけ`~/.sbxm/init.lock`を開いてexclusiveなOS file lockを取得する。

- lock待機は10秒
- timeoutはlock pathを表示してexit code `1`
- lockはworkflow終了まで保持する
- `init.lock`はworkflow終了後も削除しない
- lock取得後にconfigの有無と妥当性を再確認する
- lock fileの存在自体は処理中を意味しない。OS file lockの取得結果を使う

同時に実行された`init`はlockにより直列化される。後からlockを取得したprocessはconfigを改めて確認し、先行processが初期化を完了していれば初期化済みとして扱う。

### 11.3 処理順

1. bootstrap localeを決定する
2. `init` optionの組み合わせを検証する
3. configをread-onlyで事前確認する
4. 有効なconfigがあれば、初期化済みとして何も変更せず終了する
5. configが無効なら自動修復せず終了する
6. 対話modeならstdinとstderrがTTYであることを確認する
7. `~/.sbxm`を検証または作成し、`init.lock`を取得する
8. lock取得後にconfigの有無と妥当性を再確認する
9. 先行processが有効なconfigを作成済みなら、初期化済みとして何も変更せず終了する
10. 対話modeでは、`--lang`がなければlanguageをpromptで取得し、base path、Git name、Git emailをpromptで取得・検証する
11. option modeでは完全指定された値をpromptなしで検証する
12. configをatomic writeする
13. 初期化結果と、host環境を診断する`sbxm status --global`を表示する

Git identityの既定候補はhostの`git config --global user.name`と`user.email`。候補を表示して明示確定させ、空文字と改行を拒否する。

### 11.4 再実行

- config作成前の失敗: hostに作った`~/.sbxm`と`init.lock`以外を変更しない
- config作成後の再実行: 初期化済みであることとconfig pathを表示し、何も変更せずexit code `0`
- config変更: MVPでは直接編集し、次回load時にvalidationする

## 12. `sbxm status --global`

### 12.1 性質

hostとglobal環境をread-onlyで診断する。login、setup、file更新、daemon起動・停止を行わない。問題がある場合は、利用者が直接実行する外部commandを表示する。

`-g`を`--global`の短縮形とする。`--global`とprojectの同時指定、またはどちらも指定しない場合はexit code `1`とする。

検査対象は、sbxm自身がhost上で直接使用する設定、platform、command、serviceに限定する。利用者が実務で使用する可能性があっても、sbxmが直接使用しないpackage managerやtoolの有無は環境の正常性へ含めない。

### 12.2 検査順と項目

取得できた項目は、後続検査失敗時にも表示する。

1. global configとbase path
2. `sw_vers`と`uname -m`によるmacOS 14以上、arm64
3. host上でsbxmが直接実行する`git`、`ssh`、`docker`、`sbx`の存在
4. Docker Client/Server疎通
5. Docker Sandboxes CLIの最小version
6. network policy状態
7. daemon状態

検査を実装していない項目は行に出さない。常にerrorとなる行を予約として置かない。

- login状態は、loginを前提とするPhase 2で追加する
- Remote SSH対応状況は、SSHで接続するPhase 3で追加する

network policyは`sbx policy ls`のread-only出力から現在値を取得し、`Balanced`との完全一致だけを`ready`とする。`Balanced`以外、command失敗、timeout、parse不能は、検証済みbaselineを確認できないためerrorとしてexit code `1`とする。より制限が強いpolicyも動作と安全性を推測して受け入れない。観測した値と期待値`Balanced`を表示し、policyを自動変更しない。

### 12.3 出力

global scopeはhostとglobal環境だけを診断するため、正常結果は`GLOBAL` sectionだけをstdoutへ表示する。projectの情報を混在させない。正本localeの列は`ITEM`と`STATUS`で固定し、12.2の検査順に並べる。

```text
GLOBAL
ITEM                 STATUS
Config               ready
Base path            ready
Platform             ready
Git                  ready
SSH                  ready
Docker               ready
Docker Sandboxes     ready
Network policy       ready
Daemon               running
```

後続Phaseが検査を実装した時点で、この表へ行が増える。列構成と既存行の並び順は変えない。

取得できた行は後続検査が失敗しても省略しない。path、version、観測値、外部commandの失敗、対処方法などの詳細は表の列を増やさず、安定したerror IDを持つ診断としてstderrへ出す。これにより一覧性のある正常出力と、原因を特定できる詳細なerror情報を分離する。

正本locale以外ではsection名、列名、項目名を翻訳し、状態値はどのlocaleでも翻訳しない。状態値が正本localeの語であるため、正本locale以外は正常出力末尾へenum凡例を付ける。凡例の要否を言語ごとの設定として持たず、正本localeか否かから導出する。公開する正本localeの列構成と並び順は変更しない。列は項目名の表示幅から算出し、幅そのものを出力契約としない。

### 12.4 Exit

- 全検査成功: `0`
- 1件以上のerror: `1`

複数種類のerrorがあってもexit codeは`1`とし、個々のerror IDと診断をすべて表示する。

## 13. 自動test

- Project ID validationと予約repository名の拒否
- host path導出とsymlink拒否
- configのround trip、unknown version、permission
- 宣言fileのsourceとdestination validation
- atomic writeの各中断点
- `init` lockの同時実行、待機、timeout、事前確認とlock取得後のconfig再確認
- `init`の対話mode、3入力の完全指定option mode、不完全optionのmutation前拒否、mode判定と独立した`--lang`
- 初期化済み`init`のTTY、非TTYと副作用なしのno-op
- locale優先順位、bootstrap fallback
- help・usageの`--lang`先読み、config language、config不在・不正時fallback、helpのexit `0`
- FTL完全性、locale定義表の一貫性、CLI公開契約の記録
- CLI argument関係とmutation前validation
- TTY/non-TTY、Esc、Ctrl-C
- command runnerのenvironment、`SSH_AUTH_SOCK`除外、timeout、stream capture
- `sbx`出力parserの拒否条件
- global `status`の直接依存だけを対象とする全検査、`Balanced` network policy、出力の section・列構成・項目順、partial result、remediation、複数error時の診断
- CLI parserと外部commandの非ゼロstatusを`1`へ写像し、原値を診断へ保持すること

## 14. 受入条件

- 方向性文書の識別子、path、exit codeを共通型で表現できる
- `init`を新規・再実行・失敗後再実行できる
- `init`がconfig作成以外のhost検査、login、setupを行わない
- `status --global`がhostとglobal環境を変更せず診断し、必要な外部commandを案内する
- configの不正を自動修復しない
- 全利用者向け出力が日英で生成される
- 外部commandをshellなしで実行し、secretと`SSH_AUTH_SOCK`を規則どおり扱う
- version検出と、Phase 1が読むJSON parserのtestが成功する
- 呼び出し側のない型、policy、error ID、messageを持たない
  - `allow(dead_code)`を置かない
  - `cargo build`と`cargo clippy --all-targets`が警告なし
