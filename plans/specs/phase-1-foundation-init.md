# Phase 1 実装仕様: 共通基盤、`init`、global `status`

## 1. 目的と完了境界

Phase 1は、後続Phaseが判断を追加せず利用できる共通型、永続化、外部command実行、翻訳、対象解決、Docker Sandboxes互換性probeを実装し、`sbxm init`と`sbxm status --global`を完成させる。

Phase 1のcommand自体はprojectやSandboxを作成しない。後続Phaseの調査やlocal実装はPhase 1 PRのreviewと並行できるが、後続PRはreview結果を取り込む。各mutation commandを実機で成功扱いする前に、そのcommandが依存する互換性fixtureとsecurity probeをtestへ固定する。

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
├── en.ftl
└── ja.ftl
tests/
└── fixtures/
    └── sbx/<validated-version>/
```

主なdependencyは`clap`、`serde`、`toml`、`thiserror`、`dirs`、`fluent-bundle`、`unic-langid`、`dialoguer`、`sha2`、`serde_json`、`tempfile`とする。dependency versionは実装PRでlock fileとともに固定する。

## 3. 共通domain型

stringを直接workflowへ渡さず、validation済みの型を使う。

```text
ProjectId {
  owner_display,
  repository_display,
  canonical_id
}

SandboxName(String)
AbsoluteBasePath(PathBuf)
Locale { En, Ja }
ManagedWorktreePath(PathBuf)
HostFileSource(PathBuf)
SandboxHomeRelativePath(PathBuf)
```

`ProjectId::parse`、Sandbox名導出、host path導出は方向性文書の規則を唯一の実装とする。

## 4. CLI parse

Phase 1で9 commandと全optionをparserへ登録する。Phase 1では`init`と`status --global`を実装し、`status <project>`を含む未実装処理はparse後にlocalizedな`not implemented in this build`を返してexit code `1`とする。これによりhelpとusageの翻訳・snapshotをPhase 1で固定する。

validation順:

1. syntaxとoption関係
2. `--lang`
3. command固有の引数
4. config load
5. project解決
6. 外部command
7. mutation

helpとusageを構築する前に、argvから`--lang`だけを副作用なく先読みする。CLI parser libraryの自動help・自動終了へlocale決定を委ねず、選択したlocaleでhelp、usage、parse errorを生成する。

`add --worktrees 0`、`add --worktrees >= 2`かつ`--detach`なしはconfigやfilesystemを読む前にexit code `1`とする。

`init`は次の2 modeとする。

- 対話mode: `--lang`、`--base-path`、`--git-user-name`、`--git-user-email`を1つも指定しない
- option mode: 4 optionをすべて指定する

4 optionの一部だけを指定した場合は、TTYかどうかやconfigの有無にかかわらず、不足optionを表示してconfigやfilesystemを読む前にexit code `1`とする。option modeではpromptを表示しない。

## 5. Locale決定

優先順位:

1. 有効な`--lang ja|en`
2. 有効なglobal configの`language`
3. `init`実行時だけmacOS優先言語
4. shell locale
5. `en`

`init`と`status --global`以外のcommandでconfigが存在しない場合は、`sbxm init`を案内してexit code `1`とする。error表示はbootstrap localeを使う。

helpとusageのlocaleは次の順で決定する。

1. argvから先読みした有効な`--lang ja|en`
2. read-onlyかつbest-effortで読み込めた有効なglobal configの`language`
3. shell locale
4. `en`

- `--lang`が不正な場合はconfigを読まず、shell localeまたは`en`でparse errorを表示してexit `1`
- configが不在の場合はshell localeへfallbackする
- configが構文不正、未知version、permission不正、symlink、またはread失敗の場合もshell localeへfallbackし、help表示自体は妨げない
- `--help`とcommand別helpは、config不正だけを理由に失敗させずexit `0`
- help以外の通常commandは、parse成功後のconfig loadで同じconfig不正を診断してexit `1`
- argv先読みはlocale選択だけに使用し、ほかのargument validationやcommand実行を行わない

macOS優先言語は`defaults read -g AppleLanguages`の出力をparseする。先頭が`ja`または`ja-*`なら、TTY上でJapanese / Englishを選択させる。その他はpromptなしで`en`とする。command失敗またはparse失敗時だけ`LC_ALL`、`LC_MESSAGES`、`LANG`の順にfallbackする。

新規作成へ進む対話modeの`init`はstdinとstderrの両方がTTYであることを必須とする。どちらかがTTYでなければ何も作成せずexit code `1`とする。既に有効なconfigがある場合はTTYかどうかに関係なくno-op成功とする。option modeはTTYかどうかに関係なく実行できる。

## 6. FTL契約

- message IDは意味と用途を表すkebab-case
- 英語と日本語のID集合およびplaceholder集合を完全一致させる
- help、usage、prompt、正常出力、warning、errorをFTLから生成する
- format失敗は対象message IDとlocaleを示してexit code `1`
- 外部stderrをFTL placeholderへ埋め込まず、localized説明とは別blockで出す
- security messageは`title`、`description`、`remediation`の3 IDを必須とする

testではFTL parse、ID一致、placeholder一致、全command help snapshot、代表的なerror snapshotを検証する。

## 7. Atomic file write

configとmetadataは次の手順で書く。

1. 同一directoryに`create_new`で一時fileを作る
2. 必要permissionを設定する
3. 全内容を書いて`sync_all`する
4. 既存targetがないことを再確認する
5. renameする
6. 親directoryを`sync_all`する

更新時は既存fileのpermissionとidentityを検証し、同一directoryの一時fileからatomic renameする。symlinkは拒否する。秘密情報を一時fileへ書かない。

processが中断した一時fileは次回起動時に自動削除せず、pathと安全な削除方法を表示してexit code `1`とする。

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

## 9. Project metadata探索

- `base_path`直下のowner directoryと、その直下の`*.project/.sbxm/project.toml`だけを読む
- directory entryとmetadata fileのsymlinkは追跡しない
- すべてのmetadataをparseしてから結果を返す
- canonical ID重複、導出path不一致、Sandbox名衝突は一覧化してexit code `1`
- 1件の破損を無視して部分的な案件一覧を返さない
- 並び順はcanonical IDのbyte昇順

metadataと外部状態のvalidationは、作成元や作成履歴を条件にしない共通処理として実装する。`status`などのread-only commandとmutation commandは同じvalidation規則を使用する。手作業または別toolで作成されたmetadataと成果物も、全規則を満たす場合はsbxmが作成したものと区別せず受け入れる。

## 10. 外部command runner

runner input:

```text
program
args[]
cwd
environment policy
stdin policy
stdout policy
stderr policy
timeout class
```

規則:

- shellを介さない
- defaultで現在processのenvironmentを継承する
- security-sensitiveな`sbx`起動では`SSH_AUTH_SOCK`を必ず除外する
- secret値をargumentやdebug表示へ渡さない
- stdoutとstderrを別々にbyte列として保持し、lossy変換した場合はその事実を診断する
- timeout時はchildを終了し、command名とtimeoutを表示してexit code `1`
- testではfake executableをPATH先頭へ置き、program、args、cwd、environment、streamを記録する

timeout既定値:

| Class | Timeout |
|---|---:|
| probe | 10秒 |
| local filesystem/Git | 60秒 |
| image build/save | 30分 |
| Sandbox create/start/stop/rm | 10分 |
| interactive | timeoutなし |

## 11. Docker Sandboxes互換性契約

Docker Sandboxes CLIはEarly Accessである。Phase 1ではversion検出、互換性manifest、fixture loader、structured output parserの基盤を実装する。各外部commandのfixtureは、そのcommandを最初に使用するworkflowと同時に対象Macで採取してcommitする。

Phase 1で採取するもの:

- `sbx version`または同等command
- `sbx --help`
- `sbx ls --json`の0件、running、stopped fixture
- `sbx daemon status`のrunning、stopped fixture

各workflowの実装時に採取するもの:

- 使用するsubcommandの`--help`
- そのworkflowが読む`sbx inspect`などのstructured output
- secret存在確認に使うread-only出力
- create、exec、stop、rm、Template操作の正常・代表的失敗exit status
- `sbx rm`の通常・force modeについて、running、stopped、active sessionありのcommand形とexit status
- image、archive、Templateを新世代としてbuild・loadした後も既存Sandboxを維持できること
- Sandbox削除後に、検証済みの新Templateから同名Sandboxを再作成できること

後続workflowのfixtureがPhase 1完了時に揃っていることは要求しない。ただしfixtureなしの外部出力parser、代表的失敗を検証していないmutation、parse不能出力を成功扱いする実装は完了としない。

互換性manifest:

```toml
schema_version = 1
validated_cli_versions = ["<exact-version>"]
ls_json_fixture_version = 1
```

runtimeではexact versionを検出する。

- 0.37.0未満: exit code `1`
- fixtureと一致するversion: 続行
- patch versionだけ異なる: read-only commandはwarning付きで許可、mutationはexit code `1`
- minor/majorまたはparse不能: exit code `1`

新version対応はfixture、parser test、manifestを更新するPRで行う。

## 12. Daemon安全性probe

MVPではdaemonの安全性を永続markerやinstance IDから推測しない。`add`、`open`、およびSandboxを再作成する`rebuild`の各操作前に、全Sandboxのactive sessionがないことをstructured outputから確認し、daemonを停止してから`SSH_AUTH_SOCK`をunsetした環境で起動し直す。

これらのcommandを実機で成功扱いする前に、次を証明して結果を`tests/fixtures/sbx/<version>/daemon-security.md`へ記録する。probe未完了でもdaemonに依存しないcodeの実装は進めてよいが、Sandbox mutationを安全と判定する受入testは未完了のままにする。

1. `SSH_AUTH_SOCK`ありで起動したdaemonがSandboxへagentを転送すること
2. `SSH_AUTH_SOCK`をunsetして`sbx daemon start --detach`したdaemonでは転送されないこと
3. 対象exact versionのstructured outputから、全Sandboxのactive session不在を判定できること
4. active sessionあり、0件、command失敗、timeout、parse不能を区別できること
5. daemon停止・起動後にSandboxを再利用または作成できること

daemon操作全体では`~/.sbxm/runtime/daemon.lock`をexclusive取得する。directoryは`0700`、lock fileは`0600`とし、lock fileはworkflow終了後も削除しない。

- active sessionを1件でも検出した場合はdaemonを変更せず、対象sessionと終了方法を表示してexit code `1`
- structuredなsession検査が存在しない、またはsession不在を証明できない場合はdaemonを変更せずexit code `1`
- session検査commandの失敗、timeout、parse不能は外部状態を観測できないためexit code `1`
- session不在を確認できた場合だけ、`sbx daemon stop`と、`SSH_AUTH_SOCK`をunsetした`sbx daemon start --detach`を実行する

毎回のdaemon再起動による所要時間はMVPで受け入れる。安全性を保ったまま再起動を省略する最適化は、MVP利用後の非機能要件として検討する。

## 13. `sbxm init`

### 13.1 事前状態

configがない場合だけ新規作成する。既存の有効configは再利用し、無効configは停止する。

### 13.2 排他

configをread-onlyで事前確認し、新規作成へ進む場合だけ`~/.sbxm/init.lock`を開いてexclusiveなOS file lockを取得する。

- lock待機は10秒
- timeoutはlock pathを表示してexit code `1`
- lockはworkflow終了まで保持する
- `init.lock`はworkflow終了後も削除しない
- lock取得後にconfigの有無と妥当性を再確認する
- lock fileの存在自体は処理中を意味しない。OS file lockの取得結果を使う

同時に実行された`init`はlockにより直列化される。後からlockを取得したprocessはconfigを改めて確認し、先行processが初期化を完了していれば初期化済みとして扱う。

### 13.3 処理順

1. bootstrap localeを決定する
2. `init` optionの組み合わせを検証する
3. configをread-onlyで事前確認する
4. 有効なconfigがあれば、初期化済みとして何も変更せず終了する
5. configが無効なら自動修復せず終了する
6. 対話modeならstdinとstderrがTTYであることを確認する
7. `~/.sbxm`を検証または作成し、`init.lock`を取得する
8. lock取得後にconfigの有無と妥当性を再確認する
9. 先行processが有効なconfigを作成済みなら、初期化済みとして何も変更せず終了する
10. 対話modeではlanguage、base path、Git name、Git emailをpromptで取得・検証する
11. option modeでは完全指定された値をpromptなしで検証する
12. configをatomic writeする
13. 初期化結果と、host環境を診断する`sbxm status --global`を表示する

Git identityの既定候補はhostの`git config --global user.name`と`user.email`。候補を表示して明示確定させ、空文字と改行を拒否する。

### 13.4 再実行

- config作成前の失敗: hostに作った`~/.sbxm`と`init.lock`以外を変更しない
- config作成後の再実行: 初期化済みであることとconfig pathを表示し、何も変更せずexit code `0`
- config変更: MVPでは直接編集し、次回load時にvalidationする

## 14. `sbxm status --global`

### 14.1 性質

hostとglobal環境をread-onlyで診断する。login、setup、file更新、daemon起動・停止を行わない。問題がある場合は、利用者が直接実行する外部commandを表示する。

`-g`を`--global`の短縮形とする。`--global`とprojectの同時指定、またはどちらも指定しない場合はexit code `1`とする。

検査対象は、sbxm自身がhost上で直接使用する設定、platform、command、serviceに限定する。利用者が実務で使用する可能性があっても、sbxmが直接使用しないpackage managerやtoolの有無は環境の正常性へ含めない。

### 14.2 検査順と項目

取得できた項目は、後続検査失敗時にも表示する。

1. global configとbase path
2. `sw_vers`と`uname -m`によるmacOS 14以上、arm64
3. host上でsbxmが直接実行する`git`、`ssh`、`docker`、`sbx`の存在
4. Docker Client/Server疎通
5. Docker Sandboxes CLI versionとcompatibility manifest
6. Docker Sandboxes login状態
7. network policy状態
8. Remote SSH対応状況
9. daemon状態と、active session検査機能の対応状況

未loginの場合は`sbx login`を、Remote SSH setupが必要な場合はfixtureで固定した公式commandを表示する。commandを自動実行しない。

### 14.3 出力

global scopeはhostとglobal環境だけを診断するため、正常結果は`GLOBAL` sectionだけをstdoutへ表示する。projectの情報を混在させない。英語modeの列は`ITEM`と`STATUS`で固定し、14.2の検査順に並べる。

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
Login                ready
Network policy       ready
Remote SSH           ready
Daemon               running
Session inspection   ready
```

取得できた行は後続検査が失敗しても省略しない。path、version、観測値、外部commandの失敗、対処方法などの詳細は表の列を増やさず、安定したerror IDを持つ診断としてstderrへ出す。これにより一覧性のある正常出力と、原因を特定できる詳細なerror情報を分離する。

日本語modeではsection名、列名、項目名を翻訳し、状態値は翻訳しない。正常出力末尾のenum凡例は方向性文書の言語契約に従う。列間の空白幅は実装時のsnapshotで固定し、公開する英語modeの列構成と並び順は変更しない。

### 14.4 Exit

- 全検査成功: `0`
- 1件以上のerror: `1`

複数種類のerrorがあってもexit codeは`1`とし、個々のerror IDと診断をすべて表示する。

## 15. 自動test

- Project ID validation、case正規化、Sandbox名の衝突耐性
- path導出、symlink拒否、metadata重複
- configのround trip、unknown version、permission
- 宣言fileのsourceとdestination validation
- atomic writeの各中断点
- `init` lockの同時実行、待機、timeout、事前確認とlock取得後のconfig再確認
- `init`の対話mode、完全指定option mode、不完全optionのmutation前拒否
- 初期化済み`init`のTTY、非TTYと副作用なしのno-op
- locale優先順位、bootstrap fallback
- help・usageの`--lang`先読み、config language、config不在・不正時fallback、helpのexit `0`
- FTL完全性とsnapshot
- CLI argument関係とmutation前validation
- TTY/non-TTY、Esc、Ctrl-C
- command runnerのenvironment、timeout、stream
- compatibility fixtureの全parser
- global `status`の直接依存だけを対象とする全検査、出力snapshot、partial result、remediation、複数error時の診断
- CLI parserと外部commandの非ゼロstatusを`1`へ写像し、原値を診断へ保持すること

## 16. 受入条件

- 方向性文書の識別子、path、exit codeを共通型で表現できる
- `init`を新規・再実行・失敗後再実行できる
- `init`がconfig作成以外のhost検査、login、setupを行わない
- `status --global`がhostとglobal環境を変更せず診断し、必要な外部commandを案内する
- configとmetadataの不正を自動修復しない
- 全利用者向け出力が日英で生成される
- 外部commandをshellなしで実行し、secretと`SSH_AUTH_SOCK`を規則どおり扱う
- version検出、compatibility manifest、fixture loader、Phase 1が読むJSON parserのtestが成功する
- 後続workflowが必要なfixtureを、各workflowの実装と同時に追加できる構造になっている
