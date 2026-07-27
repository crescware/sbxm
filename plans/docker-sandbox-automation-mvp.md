# sbxm MVP 実装計画

## 1. 目的

`sbxm`は、Codex・Claude Code向けDocker Sandboxの案件別セットアップと日常操作を自動化するRust製CLIである。

初版では汎用的なSandbox管理基盤を目指さず、既存の運用手順を安全かつ再現可能に実行できることへ集中する。ただし、手順書の章や個別コマンドをそのまま公開CLIへ移植しない。利用者が達成したい目的を公開コマンドとし、Docker imageのbuild、Templateのload、Sandbox内Git設定などは内部工程として隠す。

実際の案件でMVPを使い、操作感を確認した後に設定項目や対応環境を拡張する。

### コードオーナーの設計思想

`sbxm`は、初心者向けの分かりやすさとプロフェッショナル向けの正確さを対立させない。同じ操作と出力が、経験の異なる利用者、人間とscript、日本語話者と英語話者の双方に役立つよう設計する。

- 初心者には、日本語による状態説明、危険性、具体的な対処方法を提供する
- ベテランや英語話者との共有には、英語を併記した診断ラベルと安定した英語のenum値を提供する
- 人間が対象を省略した場合は、安全な対話選択を提供する
- scriptや反復作業で対象を明示した場合は、案件選択promptを出さず決定的に実行する
- 調査時には、選択言語による説明とともに外部commandのstderr原文を保持する
- 日常利用には、全管理案件を短く確認できる`ls`を提供する
- 同じrepositoryのagentとworktreeは隔離せず、未commitの実装も相互参照できる共同作業単位として扱う
- オーナーが用意したmanaged worktreeとAgentが作る一時worktreeを区別し、管理上の数へ混在させない

便利さのために暗黙の文脈へ依存せず、画面に表示された情報だけで安全な判断、再現、他者への共有ができるCLIを目指す。新しい機能や省略記法を検討するときも、この原則に照らして初心者とプロのどちらか一方へ不要な危険や摩擦を押し付けないかを判断する。

## 2. CLIの操作モデル

### 2.1 設計原則

公開コマンドは次の利用者目的に対応させる。

- このMacで`sbxm`を使い始める
- 新しい案件を追加する
- 案件で作業を始める
- 案件の利用を一時停止する
- 全管理案件とSandboxの稼働状態を一覧する
- 1案件の構築状態と隔離を診断する
- 案件のSandboxを破棄する

MVPの公開コマンドは次の7つに限定する。

```text
sbxm init
sbxm add <owner>/<repository>
sbxm open [project]
sbxm stop [project...]
sbxm ls
sbxm status [project]
sbxm rm [project]
```

`create`、`setup`、`start`、`shell`、`destroy`など、実装工程や下位ツールの語彙は公開コマンドにしない。

### 2.2 案件の指定方法

案件を対象とする`open`、`stop`、`status`、`rm`は、引数の有無だけで動作を分ける。

- 引数あり: 指定された`<owner>/<repository>`を対象とし、案件選択promptを出さずに実行する
- 引数なし: 案件メタデータから選択肢を作り、必ず対話promptを表示する
- 引数なし、かつstdinまたはstderrがTTYでない: 対象を推測せずusage errorで終了する

カレントディレクトリから案件を推測しない。同じcommand文字列が実行directoryによって別案件へ作用することを防ぎ、shell履歴から復元したcommandの対象を予測可能にする。

明示指定する案件識別子は常に`<owner>/<repository>`とする。Sandbox名やrepository名だけの別表記は受け付けない。

対話promptには既定選択を設けない。Enterだけでは確定せず、EscまたはCtrl-Cでは何も変更せずに終了する。

- `open`、`status`、`rm`: 単一案件を選択する
- `stop`: 0件から複数案件を選択する

`rm`の削除確認は案件選択promptとは別の安全確認であるため、引数指定時にも省略しない。それ以外のコマンドは、引数指定時にpromptを表示しない。

### 2.3 表示言語

セキュリティ上の状態、危険性、対処方法を利用者が理解できることをMVPの要件とする。日本語対応を付加的な翻訳機能ではなく、初学者を含む日本語話者が安全性を判断するための中核機能として扱う。

MVPでは日本語と英語を組み込み、すべての利用者向け表示を翻訳辞書から生成する。

- helpとusage
- 対話prompt
- 状態表示
- warningとerror
- errorの原因説明と対処方法
- 破壊操作の確認
- SSH Agent露出などのsecurity診断

初回の`sbxm init`ではmacOSの優先言語を確認する。

- 優先言語の先頭が`ja`または`ja-*`: Japanese / Englishの選択promptを表示する
- それ以外: promptを出さず`en`に確定する
- macOSの優先言語を取得できない: shell localeへfallbackし、それも判定できなければ`en`にする

決定した言語はglobal configへ保存する。通常実行ではconfigを使用し、global optionの`--lang <locale>`が指定された場合だけ一時的に上書きする。

```text
sbxm --lang ja status owner/foo
sbxm --lang en status owner/foo
```

locale識別子はBCP 47に沿う形を使用する。MVPは`ja`と`en`だけを組み込むが、将来`ko`、`zh-CN`、`zh-TW`などを翻訳辞書の追加で組み込める構造にする。

日本語モードでは、診断結果を異なる言語の利用者同士で共有できるように、`ls`、`status`、error詳細、security診断などの技術的なラベルを`日本語 (English)`で表示する。

```text
案件 (Project):                 owner/foo
Sandbox状態 (Sandbox state):    stopped
ホスト側clone (Host clone):     ready
SSH Agent露出 (SSH agent):      not-exposed
```

英語モードでは英語ラベルだけを表示する。説明文や対処手順まで常に日英併記すると可読性が下がるため、二言語併記は診断・共有に必要なラベルへ限定する。

状態値などのenum、path、command、exit status、外部commandの出力は翻訳しない。これにより、日本語モードの出力を英語話者と共有するときも、表示言語を切り替えずに項目を特定でき、検索や再現に使う値も変化しない。

英語以外のモードでは、出力に現れたenum値を選択言語で説明する凡例を出力末尾に表示する。英語モードでは凡例を表示しない。

```text
案件 (Project)  Sandbox状態 (Sandbox state)
owner/foo        running
owner/bar        stopped
owner/baz        not-created

凡例 (Legend):
  running      起動中
  stopped      停止中
  not-created  未作成
```

凡例には、その出力に実際に現れた値だけを重複なく表示する。enum値自体と並び順はlocaleによって変更しない。将来追加する言語でも、辞書にenum値の説明を加えることで同じ仕組みを利用できるようにする。

## 3. MVPの前提

MVPでは次を固定する。

- 対象ホストはmacOS Sonoma 14以降のApple silicon Mac
- Docker Desktop、Docker Sandboxes 0.37.0以降、GitHub CLI、Remote SSH対応エディタを前提とする
- Git hostingはGitHubのみ
- 1 GitHub repositoryにつき、1 project directory、1 Docker Sandbox、1 Templateを使用する
- 1 Sandbox内では、1 bare Git repositoryを複数worktreeと全agentで共有する
- securityの隔離境界はworktreeやagentではなくrepositoryとする
- ホスト側とSandbox側のrepositoryは独立してcloneする
- Sandbox名は`<github-owner>-<repository-name>`とする
- ホスト側project directoryは`<base-path>/<github-owner>/<repository-name>.project`とする
- 中立Workspaceは`/tmp/docker-sandboxes/<sandbox-name>`とする
- Sandbox内のclone先は`/home/agent/work/<repository-name>`とする
- Sandbox imageは`docker/sandbox-templates:shell-docker`を基にする
- SandboxへホストのSSH Agent、SSH秘密鍵、Docker socketを渡さない
- GitHub認証には案件単位のDocker Sandboxes secretを使用する
- 組み込み表示言語は日本語と英語とする
- 既存Sandboxや既存ファイルを暗黙に削除または上書きしない
- `sbx`が保持する稼働状態を`sbxm`側へ複製しない

## 4. 設定と生成物

### 4.1 マシングローバル設定

保存先は次に固定する。

```text
~/.sbxm/config.toml
```

初期形式:

```toml
version = 1
language = "ja"
base_path = "/Users/example/Projects"

[git]
user_name = "Example User"
user_email = "user@example.com"
```

責務:

- `base_path`は全案件のホスト側配置の基準とする
- `language`は通常実行時の表示言語とする
- Git identityはSandbox内で使用する既定値とする
- secret、token、Sandboxの稼働状態は保存しない
- 将来の形式変更に備えて`version`を必須とする

`sbxm init`が設定ファイルと親directoryを作成する。directoryのpermissionは`0700`、設定ファイルは`0600`とする。

### 4.2 案件メタデータ

保存先:

```text
<base-path>/<github-owner>/<repository-name>.project/.sbx/sbxm.toml
```

初期形式:

```toml
version = 1
github_owner = "example-org"
repository_name = "example-repo"
```

Git identityを案件ごとに変更する必要が生じるまでは、案件側の上書き項目を設けない。

Sandbox名、Template image名、project root、Dockerfile path、cache path、中立Workspace、Sandbox内bare repository path、worktree rootは、案件メタデータとglobal configから毎回導出する。導出可能な値を保存して不整合を作らない。

案件追加の途中状態を独自のstatus値として保存しない。各工程の成果物と外部状態を検査し、安全に完了済みか、再実行可能か、利用者の判断が必要かを判定する。

Sandbox内の現在branch、HEAD、dirty状態はGitから取得し、案件メタデータへ複製しない。一方、オーナーが`sbxm add`で明示的に用意したworktreeと、Agentがsub-agent用に一時作成したworktreeを区別するため、managed worktreeのrelative pathは案件メタデータへ記録する。

```toml
[[worktrees.managed]]
path = "example-repo.tree-0"

[[worktrees.managed]]
path = "example-repo.tree-1"
```

この一覧は動的なGit状態のcacheではなく、オーナーが永続的な作業場所として管理対象にしたworktreeの宣言である。

### 4.3 案件ディレクトリ

ホスト側で`sbxm`が管理する構成:

```text
<base-path>/
└── <github-owner>/
    └── <repository-name>.project/
        ├── <repository-name>/
        │   └── .git/
        └── .sbx/
            ├── sbxm.toml
            ├── Dockerfile
            ├── exports/
            └── .cache/
                └── template.tar
```

既存手順の`create` shell scriptは生成しない。build、Template load、Sandbox作成は`sbxm`自身が実行する。

Dockerfileは利用者が確認・編集できる案件別ファイルとして生成する。MVPでは組み込みtemplateから初回だけ作り、既存ファイルを暗黙に上書きしない。

Sandbox内のGit repositoryは、worktreeが1つの場合も必ずbare repositoryとworktreeに分ける。

```text
/home/agent/work/
└── <repository-name>/
    ├── .git/                              # bare repository
    ├── <repository-name>.tree-0/
    ├── <repository-name>.tree-1/
    └── <repository-name>.tree-2/
```

`/home/agent/work/<repository-name>`自体は作業treeではない。開発、commit、agentの起動は必ず`<repository-name>.tree-<index>`内で行う。同じSandboxのagentはbare repository、Git object、worktree、inner Docker Engineを共有し、隣のworktreeの未commitファイルも参照できる。

この共有は同一repository内の生産性を優先する意図的な設計である。異なるrepositoryは従来どおり別Sandboxへ隔離する。未信頼コードを同一repository内でも別Sandboxへ隔離するinstance機能はMVPに含めない。

### 4.4 Worktree作成規則

Sandbox内のcloneは次の順序で行う。

1. `/home/agent/work/<repository-name>`を作成する
2. HTTPS remoteから`.git`へbare cloneする
3. `remote.origin.fetch`へ`+refs/heads/*:refs/remotes/origin/*`を設定する
4. `origin`をfetchする
5. remote default branchまたは`--detach`で指定したbranchを検証する
6. fetch完了後にworktreeを作成する

worktree pathは`<repository-name>.tree-<index>`に固定し、未使用の0以上の最小indexから割り当てる。

`add`のoption規則:

```text
sbxm add <owner>/<repository>
         [--worktrees <N>]
         [--detach <BRANCH>]
```

| 指定 | 作成結果 |
|---|---|
| 指定なし | remote default branchをcheckoutしたattached worktreeを1つ |
| `--worktrees 1` | remote default branchをcheckoutしたattached worktreeを1つ |
| `--detach develop` | `origin/develop`起点のdetached worktreeを1つ |
| `--worktrees 1 --detach develop` | `origin/develop`起点のdetached worktreeを1つ |
| `--worktrees 3 --detach develop` | `origin/develop`起点のdetached worktreeを3つ |
| `--worktrees 2`以上、`--detach`なし | mutation前にusage error |

`--worktrees`の既定値は`1`とし、1以上の整数だけを受け付ける。1 treeで`--detach`がない場合はremote default branchをattachedでcheckoutし、branch名を常に確認できる状態にする。

`--worktrees <N>`の`N`は、オーナーが最初から用意するmanaged worktree数を意味する。Gitが現在認識しているworktree総数や、Agentが実行中に作成する一時worktree数を意味しない。

2 tree以上では同一branchを複数worktreeへcheckoutできないためdetachedを使用する。ただし、起点branchを暗黙にdefault branchへ決めると、利用者が意図と異なるbranchから複数agentの作業を始める危険がある。そのため`--worktrees 2`以上では`--detach <BRANCH>`を必須とする。

`--worktrees 2`以上かつ`--detach`なしの組み合わせは、directory作成、clone、secret確認などを始める前に拒否する。`--detach`のbranchが`origin/<branch>`として存在しない場合もworktreeを作成せずerror終了する。

作成結果ではcommit hashだけでなく、mode、起点branch、managed worktree数を明示する。

```text
作成モード (Creation mode):    detached
起点branch (Start branch):      origin/develop
Managed worktree数 (Managed worktree count): 3

WORKTREE               CREATED FROM    HEAD
repository.tree-0      origin/develop  a1b2c3d
repository.tree-1      origin/develop  a1b2c3d
repository.tree-2      origin/develop  a1b2c3d
```

### 4.5 Managed worktreeと一時worktree

`sbxm`はworktreeを次の2種類に分類する。

- managed worktree: オーナーが`sbxm add`の`--worktrees`で作成を指示し、案件メタデータに記録された永続的な作業場所
- unmanaged worktree: Agentやsub-agentが実行中に独自作成し、案件メタデータに記録されていないworktree

`unmanaged`はsecurity上の異常を意味しない。Agentが並列作業のために作成する一時worktreeを含む分類名である。通常のworktree数、接続後の作業場所案内、オーナー向けの主要表示にはmanaged worktreeだけを使用する。

`<repository-name>.tree-<index>`の名前空間はmanaged worktree用に予約する。Agentが一時worktreeを作る場合は別名または別directoryを使用するよう案内する。MVPではAgentによる一時worktreeの作成・削除自体を管理しない。

`status`ではmanaged worktreeとunmanaged worktreeを別sectionに表示する。Gitが返す全worktree pathを案件メタデータのmanaged pathと照合し、managed数へunmanaged worktreeを加算しない。

`rm`は例外として、Sandbox削除による作業消失を防ぐためにmanagedとunmanagedの両方を検査する。unmanaged worktreeにdirtyまたはuntrackedな変更がある場合も、pathを明示して警告する。

### 4.6 翻訳辞書

翻訳辞書はFluent Translation List形式のresource fileとしてrepository内に置き、binaryへ組み込む。

```text
locales/
├── en.ftl
└── ja.ftl
```

英語辞書をmessage IDの正本とする。message IDは表示文そのものではなく、意味と用途を表す安定した名前にする。

```text
sandbox-list-failed
project-select-prompt
project-remove-confirmation
security-ssh-agent-exposed-title
security-ssh-agent-exposed-description
security-ssh-agent-exposed-remediation
```

翻訳済みの短い断片をRust側で連結して文章を作らない。path、command、exit statusなどの動的な値はplaceholderとして辞書へ渡し、全言語でplaceholder名を一致させる。

外部commandのstdoutとstderrは翻訳せず原文を保持する。その前に`sbxm`が選択言語で失敗の意味、考えられる原因、対処方法を表示し、利用者が理解できる説明と検索可能な原文を両方残す。

診断ラベルは通常の文章messageと分け、安定したmessage IDを持たせる。日本語辞書の診断ラベルには対応する英語表記を含める。

```text
label-project = 案件 (Project)
label-sandbox-state = Sandbox状態 (Sandbox state)
label-command = 実行コマンド (Command)
label-exit-status = 終了状態 (Exit status)
label-legend = 凡例 (Legend)
enum-sandbox-state-running = 起動中
enum-sandbox-state-stopped = 停止中
enum-sandbox-state-not-created = 未作成
```

組み込みの日本語・英語では翻訳欠落をtest failureとする。将来追加されるlocaleでmessageが欠落した場合は英語へfallbackできる設計にするが、fallbackの発生を検出可能にする。

## 5. コマンド設計

### 5.1 `sbxm init`

このMacで`sbxm`を利用可能にするため、原則として最初の1回だけ実行する。

実行内容:

1. macOSの優先言語を確認し、日本語環境だけJapanese / Englishの選択promptを表示する
2. macOS versionとCPU architectureを確認する
3. `brew`、Docker Client・Server、`gh`、`sbx`の存在とversionを確認する
4. Docker Engineへ接続できることを確認する
5. `sbx`が未導入の場合は公式のHomebrew installコマンドを表示する
6. `sbx login`が必要な場合は対話commandを起動する
7. network policyを表示し、未設定の場合は`Balanced`を選ぶよう案内する
8. `sbx setup ssh`を実行する
9. `base_path`、Git user name、Git user emailを対話的に取得する
10. 選択した`language`を含む`~/.sbxm/config.toml`を安全なpermissionで作成する

Homebrew packageのinstallはマシングローバルな変更となるため自動実行せず、正確なコマンドを表示して終了する。利用者がinstall後に`sbxm init`を再実行すると残りの確認から続行する。

`init`は再実行可能とする。完了済みの項目は成功として扱い、既存global configは上書きしない。設定変更用コマンドはMVPに含めず、設定ファイルを直接編集してもらう。

### 5.2 `sbxm add <owner>/<repository> [--worktrees <N>] [--detach <BRANCH>]`

新しい案件を管理対象へ追加し、ホスト側とSandbox側の両方で作業可能な状態まで構築する。

利用者から見た操作は一つだが、内部では次の小さな工程を順番に実行する。

1. global configを読み、owner、repository名、`--worktrees`、`--detach`の組み合わせを検証する
2. project directory、`.sbx/exports`、`.sbx/.cache`を作成する
3. ホスト側repositoryをSSH URLでcloneする
4. `sbxm.toml`と標準Dockerfileを作成する
5. 中立Workspaceを作成する
6. Dockerfileから案件専用imageをbuildする
7. imageを`.sbx/.cache/template.tar`へ保存する
8. TemplateをDocker Sandboxes runtimeへloadする
9. Docker Sandboxes daemonをSSH Agentなしで安全に起動する
10. 案件専用Sandboxを作成する
11. ホストの`~/.claude/settings.json`が存在する場合だけ、安全なpermissionでコピーする
12. Sandbox内のGit user nameとemailを設定する
13. GitHub CLIのGit protocolをHTTPSへ設定する
14. 案件専用GitHub secretを確認する
15. repositoryを`/home/agent/work/<repository-name>/.git`へbare cloneする
16. remote-tracking設定を追加してfetchする
17. remote default branchまたは明示されたdetach branchを検証する
18. 規則に従ってattachedまたはdetached worktreeを作成する
19. 作成したworktree pathをmanaged worktreeとして案件メタデータへ記録する
20. 作成mode、起点branch、managed worktree path、HEADを表示する
21. 各managed worktreeの`mise.toml`、`.mise.toml`、`.tool-versions`の有無と次の操作を報告する

GitHub fine-grained personal access tokenの発行とsecret入力は利用者の操作を必要とする。secretが未登録の場合、`add`は正確な`sbx secret set`コマンドを表示して安全に中断する。登録後に同じ`sbxm add`を再実行すると、完了済みの工程を検証して続きから再開する。

`mise trust`と`mise install`はrepository由来コードの実行につながるため自動実行しない。必要なコマンドだけを案内する。

既存directory、bare repository、worktree、Dockerfile、Sandboxを発見した場合は、その状態が期待する案件に属することを検証する。安全に完了済みと判断できる工程は再利用し、不一致や上書きが必要な状態では対象と理由を示して停止する。

### 5.3 `sbxm open [project]`

対象案件で日常作業を始める。

実行内容:

1. 引数があれば案件選択promptなしで対象を解決し、なければ単一選択promptを表示する
2. Docker Engineへ接続できることを確認する
3. Docker Sandboxes daemonがSSH Agentを引き継がない状態を保証する
4. Sandboxが`stopped`の場合は端末を占有せずに起動する
5. Sandboxが起動済みの場合はそのまま使用する
6. `ssh <sandbox-name>.sbx`で接続する
7. 接続後に開くrepository pathを表示する

通常の開始位置はDockerfileのshell設定により`/home/agent/work`とする。MVPではSSH commandを複雑化せず、対象repositoryのmanaged worktree path一覧を接続時に表示する。bare repositoryのcontainer rootやAgentが作成した一時worktreeを通常の作業directoryとして案内しない。

daemonがホストの再起動後などに新しく起動する最初の`open`では、`sbx daemon stop`後に`SSH_AUTH_SOCK`を除外した`sbx ls`でdaemonを起動する。すでに安全なdaemonで別案件を利用中の場合は、案件切り替えのたびに再起動しない。

Docker Sandboxesがdaemonの起動環境を判定できる機械可読な手段を提供しない場合、MVPでは`sbxm`が安全に起動したdaemonを識別するruntime markerを用いる。markerは再生成可能なruntime情報とし、MacまたはDocker Desktop再起動後に古いmarkerを信用しない判定方法を実装前に検証する。

### 5.4 `sbxm stop [project...]`

当面使用しないSandboxを停止する。内部Git repository、設定、package、Docker imageは保持される。

引数がある場合は指定された全案件をpromptなしで停止する。複数案件をまとめて指定できる。

```text
sbxm stop owner/foo owner/bar
```

引数がない場合は全管理案件を選択肢とする複数選択promptを表示する。既定選択は空とし、何も選択せずに確定した場合は状態を変更せず正常終了する。

停止済みのSandboxは成功として扱う。MVPでは全Sandboxを暗黙に停止するoptionを設けない。

### 5.5 `sbxm ls`

`sbxm`の案件メタデータを正本として、全管理案件とSandboxの稼働状態を一覧する。`sbx ls`の透過的な別名にはしない。

実行内容:

1. global configから`base_path`を読む
2. `<base-path>/*/*.project/.sbx/sbxm.toml`を探索する
3. 各メタデータから案件名と期待するSandbox名を導出する
4. `sbx ls`を一度だけ実行する
5. メタデータと実際のSandbox一覧を突き合わせる
6. 案件名、Sandbox名、状態を簡潔なtableで表示する

```text
PROJECT          SANDBOX          STATE
owner/foo        owner-foo        running
owner/bar        owner-bar        stopped
owner/baz        owner-baz        not-created
```

状態は次の3つに限定する。

- `not-created`: メタデータは存在するが、対応するSandboxは存在しない
- `running`: 対応するSandboxが存在し、起動中
- `stopped`: 対応するSandboxが存在し、停止中

`sbx ls`が失敗した場合は一覧を出力せず、実行したcommand、exit status、安全なstderrを示して非ゼロ終了する。メタデータだけからSandbox状態を推測せず、`unknown`や`not-observed`へ丸めない。

`sbx ls`が対応外のstateを返した場合も、対象Sandboxと生のstate値を示して非ゼロ終了する。壊れた案件メタデータを発見した場合は、そのpathとparse errorを示して非ゼロ終了する。

対応する案件メタデータがないSandboxは、管理案件と混ぜず`UNMANAGED SANDBOXES`として別tableに表示する。`ls`は読み取り専用とし、取り込みや削除は行わない。

### 5.6 `sbxm status [project]`

1案件について、構築工程、作業可能性、credential隔離を詳細に診断する。全案件の一覧は表示しない。

引数がある場合は案件選択promptなしで対象を診断する。引数がない場合は全管理案件から単一選択するpromptを表示する。

表示項目:

- project root
- Sandbox名と稼働状態
- 中立Workspace
- ホスト側clone
- Dockerfile
- Template archive
- GitHub secret
- Sandbox内bare repository
- managed worktree数とpath
- 各managed worktreeのHEAD、branchまたはdetached、dirty状態
- unmanaged worktreeのpath、HEAD、branchまたはdetached、dirty状態
- SSH Agent露出

Sandboxが存在しない場合、Sandbox内部でのみ検査可能な項目は`not-applicable`とする。これは不明状態ではなく、検査対象が存在しないという確定結果である。

診断に必要な外部commandが失敗した場合は、取得済みの診断結果を表示した後、失敗したcommand、exit status、安全なstderrを示して非ゼロ終了する。確認不能な項目を`unknown`へ丸めない。

機密値は表示しない。修復や状態変更は行わない。

`ls`と`status`の責務は次に固定する。

```text
ls      メタデータを起点に全管理案件とSandboxの稼働状態を一覧する
status  1案件の構築状態、作業可能性、credential隔離を診断する
```

### 5.7 `sbxm rm [project]`

対象Sandboxの内部状態を破棄する。

引数がある場合は案件選択promptなしで対象を解決する。引数がない場合は全管理案件から単一選択するpromptを表示する。

削除前に次を表示し、明示確認を必須とする。

- 削除対象の案件とSandbox名
- managedとunmanagedを含む全worktreeのGit status
- 各worktreeの分類、現在branchまたはdetached状態
- 各worktreeの直近commit
- 各worktreeにGit管理外ファイルが残っている可能性
- 削除するものと残すもの

確認後に`sbx rm`を実行する。MVPでは次を残す。

- ホスト側clone
- `.sbx/sbxm.toml`
- 案件別Dockerfile
- `.sbx/exports`
- Template archive
- Docker image
- 中立Workspace

これにより`rm`はSandbox内部状態の破棄に限定する。案件メタデータを含むホスト側project全体の削除はMVPで自動化しない。

## 6. 日常利用

### 6.1 その日の最初

Docker Desktopを起動し、作業する案件を指定して接続する。

```text
sbxm open owner/foo
```

案件を明示せず人間が一覧から選ぶ場合は、引数なしで起動する。

```text
sbxm open
```

`open`は案件選択promptを表示した後、Docker Engine、daemon、Sandboxの状態を確認し、必要なものだけを起動してSSH接続する。実行directoryから対象を推測しない。

### 6.2 案件を切り替える

次の案件を開く。

```text
sbxm open owner/bar
```

短時間で戻るSandboxは起動したままでよい。当面戻らない案件だけ停止する。

```text
sbxm stop owner/foo
```

専用の`switch`コマンドは設けない。案件を開くことと、以前の案件を停止することを独立させ、複数案件の同時利用を妨げない。

### 6.3 状態を確認する

任意のdirectoryで全管理案件の一覧を表示する。

```text
sbxm ls
```

問題のある案件を詳細に診断する。

```text
sbxm status owner/foo
```

### 6.4 業務終了時

エディタの未保存ファイルを保存し、Codex、Claude Code、開発server、test watcherを終了してから、その日に使用したSandboxを明示的に停止する。

```text
sbxm stop owner/foo owner/bar owner/baz
sbxm ls
```

`stop`は内部状態を保持する。日常利用では`rm`を使用しない。

## 7. MVPに含めないもの

初回実装では次を独立コマンドにしない。

- `config`: global configは`init`で作成し、必要なら直接編集する
- `create`、`setup`: `add`の内部工程とする
- `start`、`shell`: `open`の内部工程とする
- `remove`、`destroy`: 破棄操作は`rm`に統一する
- `switch`: `open`と必要に応じた`stop`を使用する
- `rebuild`: Dockerfile変更後の再構築はMVP利用後に設計する
- `export`: 既存の`sbx cp`コマンドを案内する
- `doctor`: 1案件の診断は`status`へ含める
- `ports`: 既存の`sbx ports`コマンドを案内する
- worktreeの追加・削除専用command: MVPではSandbox内の`git worktree`を使用する
- 同一repositoryを複数Sandboxへ分離するinstance機能
- host側project全体の削除
- 複数host、GitLab、Linux、Intel Macへの対応
- Dockerfile templateの選択機能
- CPU・memory設定
- secret値の保管または入力代行
- Codex・Claude Codeの対話login自動化
- `mise trust`およびrepository固有toolの自動install

## 8. Rust実装方針

### 8.1 crate構成

最初は単一binary crateとし、公開コマンドと内部workflowを分離する。

```text
src/
├── main.rs
├── cli.rs
├── config.rs
├── i18n.rs
├── project.rs
├── paths.rs
├── command.rs
├── sandbox.rs
├── git.rs
├── templates.rs
└── workflow/
    ├── init.rs
    ├── add.rs
    ├── open.rs
    ├── list.rs
    ├── status.rs
    └── rm.rs
```

- `cli`: 7つの公開コマンドとargumentの定義
- `config`: TOMLの読み書き、version検証
- `i18n`: locale決定、辞書load、message format、fallback検出
- `project`: 案件探索、対象解決、案件メタデータ
- `paths`: 全導出pathの一元管理
- `command`: 外部process実行、環境変数制御、error整形
- `sandbox`: `sbx`、`docker`を使う再利用可能な内部操作
- `git`: host clone、Sandbox内bare clone、remote-tracking、worktree検査と作成
- `templates`: 組み込みDockerfile
- `workflow`: 利用者目的ごとの内部工程と再開判定

### 8.2 主なdependency

- `clap`: CLI parser
- `dialoguer`: 既定選択のない案件選択promptと削除確認
- `fluent-bundle`: FTL翻訳辞書のloadとmessage format
- `unic-langid`: BCP 47 locale識別子のparseと照合
- `serde`と`toml`: 設定形式
- `thiserror`: 利用者向けに文脈を付けたerror
- `dirs`: home directoryの解決

外部コマンドはRust libraryで再実装せず、引数配列を使って`git`、`docker`、`sbx`、`ssh`を呼び出す。shell文字列を組み立てないことで、owner名やpathのshell injectionを避ける。

### 8.3 翻訳の実装規則

- 利用者向け文字列をRust sourceへ直接埋め込まない
- message IDを通して辞書から表示する
- help、usage、prompt、正常出力、warning、errorを同じ仕組みで扱う
- 英語をmessage IDとfallbackの正本にする
- 組み込みの日本語と英語は全message IDを必須とする
- 全言語でplaceholder名と必須placeholderを一致させる
- security messageは少なくともtitle、description、remediationを持つ
- 日本語の診断ラベルは対応する英語表記を括弧内に含める
- 二言語併記は診断ラベルに限定し、説明文と対処手順は選択言語だけで表示する
- enum値、path、command、exit statusはlocaleによって変更しない
- 英語以外のlocaleでは、出力に現れたenum値の説明を選択言語の凡例として表示する
- 英語localeではenum値の凡例を表示しない
- 外部commandの出力は原文を保持し、選択言語による説明と分離して表示する
- 翻訳format自体に失敗した場合は、そのmessage IDとlocaleを示して安全にerror終了する

### 8.4 workflowの再開

`add`の各内部工程は次のinterfaceを持つ単位として設計する。

1. 現在状態を検査する
2. 未完了なら実行する
3. 完了状態を検証する
4. 次の工程へ進む

途中で利用者操作や外部command失敗が発生しても、同じ`add`を再実行できるようにする。

独自の進捗flagだけを根拠に工程をskipしない。ファイル、Git remote、Docker image、Template、Sandbox、secret、bare repository、worktreeなど、実際の成果物を確認する。

不完全な成果物を自動削除してやり直さない。安全に再利用できない状態では、対象と手動復旧方法を表示して停止する。

### 8.5 安全性の共通規則

- ownerは`[A-Za-z0-9-]+`、repository名は`[A-Za-z0-9._-]+`で検証する
- `base_path`は絶対pathかつ末尾のslashを除いた形で保持する
- pathを文字列連結せず`PathBuf`で構築する
- secret値をcommand argument、log、configへ書かない
- 外部commandの失敗時はstatusと安全なstderrを示す
- security warningには危険性と具体的な対処方法を選択言語で表示する
- current directoryから操作対象を推測しない
- 引数指定時は案件選択promptを出さず、引数省略時だけTTY上でpromptを出す
- 非TTYで対象引数を省略した場合はusage errorで終了する
- 破壊的操作は対象を完全な名前で表示し、明示確認を要求する
- 既存ファイルを既定で上書きしない
- 同じrepositoryのworktree間は相互アクセス可能であり、security境界ではないことを表示と文書で明示する
- `rm`前は一部ではなく全worktreeの保存状態を検査する
- `SSH_AUTH_SOCK`を除外すべきprocessを一箇所に集約してtestする
- Sandboxの存在判定は部分一致を使わず、可能なら`sbx`の機械可読出力を利用する
- 外部状態を取得できない場合は、曖昧な代替状態を生成せず具体的なerrorで終了する
- 複数案件操作では、全対象を解決・検証してから状態変更を始める

## 9. 実装順

### Phase 1: `init`と共通基盤

- Cargo projectを作成する
- command parserと共通error表示を実装する
- `en.ftl`と`ja.ftl`を追加し、翻訳辞書のloadとformatを実装する
- macOSの優先言語、shell locale、`--lang`、configによるlocale決定を実装する
- help、usage、prompt、errorの翻訳経路を実装する
- 前提commandとversionの検出を実装する
- global configのschema、permission、読み書きを実装する
- 案件metadata、全案件探索、明示的な案件識別子、導出pathを実装する
- TTY判定と単一・複数選択promptを実装する
- `sbxm init`を実装する
- config、入力値、path導出のunit testを追加する

完了条件:

- 未導入の前提commandと未起動のDocker Engineを明確に報告できる
- 英語環境では選択promptなしで`en`を保存できる
- 日本語環境ではJapanese / Englishを選択して保存できる
- `--lang`で保存済み言語を一時的に上書きできる
- help、usage、prompt、errorを日本語と英語で表示できる
- 日本語の診断出力を言語変更せず英語話者と項目単位で共有できる
- `sbx setup ssh`までの初回準備を一巡できる
- 一時HOMEを用いたtestで`~/.sbxm/config.toml`を安全に作成できる
- `init`を再実行して既存設定を上書きしない
- 不正なowner、repository、base pathを拒否できる
- current directoryに依存せず同じ引数から同じ案件を解決できる
- 引数なしの非TTY実行を拒否できる

### Phase 2: `add`

- Dockerfileを組み込みtemplateとして追加する
- host側directory作成、SSH clone、案件metadata作成を実装する
- `--worktrees`と`--detach`の入力関係をmutation前に検証する
- image build、save、Template load、Sandbox createを実装する
- Claude settingsの条件付きコピーを実装する
- GitHub secret確認と再開導線を実装する
- Sandbox内Git identity、HTTPS bare cloneを実装する
- remote-tracking設定、fetch、remote default branch解決を実装する
- attached worktreeと明示branch起点のdetached worktree作成を実装する
- worktree作成結果のmode、起点branch、path、HEAD表示を実装する
- managed worktree pathの案件メタデータ記録を実装する
- 工程ごとの状態検査と再開判定を実装する
- 外部commandをfake化したintegration testを追加する

完了条件:

- `sbxm add owner/repository`だけで新規案件を作業可能な状態まで構築できる
- optionなしではremote default branchのattached worktreeを1つ作成する
- 1 treeでもbare repositoryとworktreeを分離する
- 2 tree以上では`--detach <BRANCH>`なしにmutationを開始できない
- detached worktreeの作成結果に起点branchを表示する
- `--worktrees`の数がmanaged worktreeだけを表す
- secret登録で中断した後、同じcommandで再開できる
- 完了済み工程を安全に再利用できる
- 不一致や不完全な成果物を暗黙に上書き・削除しない
- daemonをSSH Agentなしで起動する
- tokenやホストcredentialを成果物へ残さない

### Phase 3: `open`、`stop`、`ls`、`status`

- 安全なdaemon起動判定とruntime markerを実機検証する
- Sandboxの冪等な起動を実装する
- SSH接続を実装する
- 引数ありの非対話操作と引数なしの案件選択promptを実装する
- 単一・複数案件の停止を実装する
- メタデータを起点とする全管理案件の探索を実装する
- 1回の`sbx ls`との突き合わせと3状態の一覧を実装する
- `sbx ls`失敗と未対応stateの具体的なerrorを実装する
- 未管理Sandboxの分離表示を実装する
- 案件詳細、隔離状態、bare repository、managed・unmanaged worktreeの分離診断を実装する

完了条件:

- その日の最初の`open`でdaemonをSSH Agentなしにできる
- 同じ日の案件切り替えでdaemonを不要に再起動しない
- 停止中、起動中のどちらからでも`open`できる
- 複数Sandboxを明示的にまとめて停止できる
- `ls`が未作成Sandboxを`not-created`として表示できる
- `ls`が外部状態を取得できないときに一覧を推測しない
- `status`が1案件だけを詳細に診断できる
- 引数指定時に案件選択promptを出さない
- 引数省略時に既定選択なしのpromptを表示する
- promptのキャンセル時に状態を変更しない
- 状態表示で機密値を出力しない

### Phase 4: `rm`と手動検証

- managed・unmanagedを含む全worktreeの保存状態確認を実装する
- 削除対象と保持対象の表示を実装する
- 明示確認後のSandbox削除を実装する
- end-to-end手動検証手順をREADMEへ追加する

完了条件:

- 未保存作業を確認せずSandboxを削除できない
- 1つでもdirtyまたはuntrackedなworktreeがあれば、そのtreeを明示して警告できる
- `rm`がホスト側projectや成果物を削除しない
- `init`、`add`、日常利用、`rm`まで一巡できる

## 10. 検証方針

自動testでは外部環境を変更せず、次を確認する。

- TOMLのround tripとschema version拒否
- path導出と全案件metadataの探索
- `<owner>/<repository>`による案件argumentの一意な解決
- current directoryを変えても明示引数の解決結果が変わらないこと
- 引数指定時に案件選択promptを表示しないこと
- 引数省略時の単一選択・複数選択prompt
- promptに既定選択がなく、Enterだけで対象を確定しないこと
- promptのEsc・Ctrl-Cによる副作用なしの終了
- 引数省略かつ非TTYでのusage error
- locale決定の優先順位と英語環境でのprompt省略
- 日本語環境でのJapanese / English選択
- `--lang`による一時上書き
- `en.ftl`と`ja.ftl`のmessage ID完全一致
- 全locale間のplaceholder一致
- FTL syntax errorの検出
- 日本語と英語のhelp、usage、prompt、errorのsnapshot
- 日本語の全診断ラベルに対応する英語表記が含まれること
- localeを変更してもenum値、path、command、exit statusが変化しないこと
- 英語以外のlocaleで、出力に現れたenum値だけが重複なく凡例へ表示されること
- 英語localeでenum値の凡例が表示されないこと
- 全組み込みlocaleでenum値の説明が欠落していないこと
- 説明文と対処手順が不要に二言語併記されないこと
- security messageにtitle、description、remediationが存在すること
- 外部stderrが原文のまま保持され、その前に選択言語の説明が表示されること
- 組み込みlocaleでfallbackが発生しないこと
- 入力値検証
- 外部commandへ渡すprogram、arguments、cwd、environment
- `SSH_AUTH_SOCK`の除外
- 既存fileの非上書き
- `add`の各工程のskip、実行、中断、再開
- `--worktrees`の既定値が`1`であり、0を拒否すること
- optionなしと`--worktrees 1`でremote default branchのattached worktreeを作ること
- `--detach develop`で`origin/develop`起点のdetached worktreeを1つ作ること
- `--worktrees 1 --detach develop`を許可すること
- `--worktrees 2`以上では`--detach`を必須とし、違反時はmutation前にusage errorとなること
- `--worktrees 3 --detach develop`で同じcommit起点のdetached worktreeを3つ作ること
- 存在しないdetach branchをworktree作成前に拒否すること
- bare clone後にremote-trackingを設定し、fetch完了後だけworktreeを作ること
- worktreeが1つでも通常cloneを作らないこと
- worktree pathのindexが重複しないこと
- 作成したmanaged worktree pathだけが案件メタデータへ記録されること
- Agentが追加したworktreeをmanaged数へ加算しないこと
- managed metadataにないGit worktreeをunmanagedとして分離すること
- 作成結果がmode、起点branch、worktree path、HEADを含むこと
- 各外部command失敗時の後続処理停止
- `open`と`stop`の冪等性
- 複数Sandbox停止時の対象限定
- メタデータに存在し、Sandboxに存在しない案件の`not-created`判定
- `sbx ls`失敗時の非ゼロ終了と一覧非出力
- 未対応Sandbox stateを生の値とともにerror表示すること
- 案件メタデータのparse errorをpathとともに表示すること
- 未管理Sandboxの分離
- `ls`が詳細診断を実行しないこと
- `status`の単一案件診断と`not-applicable`判定
- `status`がbare repositoryとmanaged・unmanaged worktreeを分離して表示すること
- `status`が各worktreeのHEAD、branch、dirty状態を表示すること
- `status`の外部command失敗時の部分結果と非ゼロ終了
- `rm`が全worktreeを検査すること
- `rm`の確認と保持対象

実機でのみ確認できる内容は、専用のtest repositoryを使って手動検証する。

1. 英語環境の`sbxm init`でpromptなしに`en`が選ばれること
2. 日本語環境の`sbxm init`でJapanese / Englishを選択できること
3. 日本語と英語のhelp、error、security warningと対処方法
4. 日本語モードの診断ラベルが英語話者にも識別可能であること
5. 日本語モードでenum値が英語のまま表示され、日本語の凡例が付くこと
6. 英語モードではenum値の凡例が付かないこと
7. `sbxm init`による前提確認、global config、SSH連携
8. `sbxm add`によるhost側clone、image build、Template load
9. secret未登録による安全な中断
10. secret登録後の`add`再開とSandbox内bare clone
11. optionなしでdefault branchのattached worktreeが1つ作られること
12. 複数worktreeで`--detach`を必須とし、明示branchから作られること
13. 同じSandbox内のworktree間で未commitファイルを相互参照できること
14. Agentが一時worktreeを追加してもmanaged worktree数が変化しないこと
15. `status`でmanaged worktreeとAgentの一時worktreeが分離されること
16. 中立Workspaceとホストpathの非露出
17. その日の最初の`open`と安全なdaemon起動
18. Sandbox内の`SSH_AUTH_SOCK`と`ssh-add -L`
19. Remote SSH接続とmanaged worktree pathの案内
20. 2案件目の`open`でdaemonを不要に再起動しないこと
21. 複数案件の起動、切り替え、一括停止
22. stop後の状態保持と翌日の再起動
23. `ls`による`running`、`stopped`、`not-created`の一覧
24. `sbx ls`失敗時に推測した一覧を出さないこと
25. 未管理Sandboxの分離表示
26. 引数指定時のpromptなし実行
27. 引数省略時の案件選択、キャンセル、非TTY拒否
28. `status`によるbare repository、managed・unmanaged worktree、隔離状態の診断
29. `rm`によるmanaged・unmanagedを含む全worktreeの保存確認
30. `rm`の案件選択とは独立した削除確認
31. `rm`後の保持対象

## 11. 最初の利用後にレビューする論点

MVPを実案件またはtest repositoryで一巡した後、次をレビューする。

- `init`、`add`、`open`、`stop`、`ls`、`status`、`rm`の語彙と粒度は自然か
- `ls`と`status`の責務分担は日常利用で自然か
- 明示引数と対話選択の使い分けは業務利用で安全か
- 日本語のsecurity messageは初学者が危険性と対処を判断できるか
- 日本語の診断ラベルに英語を併記する範囲は過不足ないか
- 英語以外のモードに付けるenum値の凡例は理解を助け、過度に冗長ではないか
- 翻訳辞書だけで新しいlocaleを追加できる構造になっているか
- `add`の中断理由と再開方法が利用者に明確か
- daemon再起動確認が日常利用の妨げにならないか
- runtime markerによる安全なdaemonの判定は十分に堅牢か
- 引数省略時の選択promptは十分に素早く操作できるか
- `open`接続後にworktreeを選択・移動する支援が必要か
- worktreeの追加・削除専用commandを次に追加すべきか
- repository単位の共有境界はagent間の生産性とsecurityのバランスに合っているか
- managed worktreeとAgentの一時worktreeの区別は実際の並列agent運用に合っているか
- Dockerfileを利用者が編集できる生成物にした判断は適切か
- 次に`rebuild`、`export`、`ports`、`doctor`のどれを追加すべきか
- 案件単位のGit identity上書きが必要か
- host側project全体を削除する操作が必要か

このレビューを行うまでは、公開コマンド、対応環境、設定項目を増やさない。
