# sbxm MVP 実装計画

## 1. 目的

`sbxm`は、Codex・Claude Code向けDocker Sandboxの案件別セットアップと日常操作を自動化するRust製CLIである。

初版では汎用的なSandbox管理基盤を目指さず、既存の運用手順を安全かつ再現可能に実行できることへ集中する。ただし、手順書の章や個別コマンドをそのまま公開CLIへ移植しない。利用者が達成したい目的を公開コマンドとし、Docker imageのbuild、Templateのload、Sandbox内Git設定などは内部工程として隠す。

実際の案件でMVPを使い、操作感を確認した後に設定項目や対応環境を拡張する。

## 2. CLIの操作モデル

### 2.1 設計原則

公開コマンドは次の利用者目的に対応させる。

- このMacで`sbxm`を使い始める
- 新しい案件を追加する
- 案件で作業を始める
- 案件の利用を一時停止する
- 管理案件の状態を確認する
- 案件のSandboxを破棄する

MVPの公開コマンドは次の6つに限定する。

```text
sbxm init
sbxm add <owner>/<repository>
sbxm open [project]
sbxm stop [project...]
sbxm status [project]
sbxm rm [project]
```

`create`、`setup`、`start`、`shell`、`destroy`など、実装工程や下位ツールの語彙は公開コマンドにしない。

### 2.2 案件の指定方法

案件を指定するコマンドは、次の順で対象を解決する。

1. command argumentで指定された案件
2. current directoryを親方向へ辿って見つけた`.sbx/sbxm.toml`

argumentとcurrent directoryの両方から案件を特定できない場合は、管理案件の候補を表示して終了する。MVPでは対話的な案件選択UIや`switch`コマンドを作らない。

`add`ではGitHubの`<owner>/<repository>`を指定する。それ以外のコマンドでは、案件メタデータから導出した一意な`<owner>/<repository>`またはSandbox名を受け付ける。曖昧なrepository名だけの指定は受け付けない。

## 3. MVPの前提

MVPでは次を固定する。

- 対象ホストはmacOS Sonoma 14以降のApple silicon Mac
- Docker Desktop、Docker Sandboxes 0.37.0以降、GitHub CLI、Remote SSH対応エディタを前提とする
- Git hostingはGitHubのみ
- 1 GitHub repositoryにつき、1 project directory、1 Docker Sandbox、1 Templateを使用する
- ホスト側とSandbox側のrepositoryは独立してcloneする
- Sandbox名は`<github-owner>-<repository-name>`とする
- ホスト側project directoryは`<base-path>/<github-owner>/<repository-name>.project`とする
- 中立Workspaceは`/tmp/docker-sandboxes/<sandbox-name>`とする
- Sandbox内のclone先は`/home/agent/work/<repository-name>`とする
- Sandbox imageは`docker/sandbox-templates:shell-docker`を基にする
- SandboxへホストのSSH Agent、SSH秘密鍵、Docker socketを渡さない
- GitHub認証には案件単位のDocker Sandboxes secretを使用する
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
base_path = "/Users/example/Projects"

[git]
user_name = "Example User"
user_email = "user@example.com"
```

責務:

- `base_path`は全案件のホスト側配置の基準とする
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

Sandbox名、Template image名、project root、Dockerfile path、cache path、中立Workspace、Sandbox内clone先は、案件メタデータとglobal configから毎回導出する。導出可能な値を保存して不整合を作らない。

案件追加の途中状態を独自のstatus値として保存しない。各工程の成果物と外部状態を検査し、安全に完了済みか、再実行可能か、利用者の判断が必要かを判定する。

### 4.3 案件ディレクトリ

`sbxm`が管理する構成:

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

## 5. コマンド設計

### 5.1 `sbxm init`

このMacで`sbxm`を利用可能にするため、原則として最初の1回だけ実行する。

実行内容:

1. macOS versionとCPU architectureを確認する
2. `brew`、Docker Client・Server、`gh`、`sbx`の存在とversionを確認する
3. Docker Engineへ接続できることを確認する
4. `sbx`が未導入の場合は公式のHomebrew installコマンドを表示する
5. `sbx login`が必要な場合は対話commandを起動する
6. network policyを表示し、未設定の場合は`Balanced`を選ぶよう案内する
7. `sbx setup ssh`を実行する
8. `base_path`、Git user name、Git user emailを対話的に取得する
9. `~/.sbxm/config.toml`を安全なpermissionで作成する

Homebrew packageのinstallはマシングローバルな変更となるため自動実行せず、正確なコマンドを表示して終了する。利用者がinstall後に`sbxm init`を再実行すると残りの確認から続行する。

`init`は再実行可能とする。完了済みの項目は成功として扱い、既存global configは上書きしない。設定変更用コマンドはMVPに含めず、設定ファイルを直接編集してもらう。

### 5.2 `sbxm add <owner>/<repository>`

新しい案件を管理対象へ追加し、ホスト側とSandbox側の両方で作業可能な状態まで構築する。

利用者から見た操作は一つだが、内部では次の小さな工程を順番に実行する。

1. global configを読み、ownerとrepository名を検証する
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
15. repositoryを`/home/agent/work/<repository-name>`へcloneする
16. `mise.toml`、`.mise.toml`、`.tool-versions`の有無と次の操作を報告する

GitHub fine-grained personal access tokenの発行とsecret入力は利用者の操作を必要とする。secretが未登録の場合、`add`は正確な`sbx secret set`コマンドを表示して安全に中断する。登録後に同じ`sbxm add`を再実行すると、完了済みの工程を検証して続きから再開する。

`mise trust`と`mise install`はrepository由来コードの実行につながるため自動実行しない。必要なコマンドだけを案内する。

既存directory、clone、Dockerfile、Sandboxを発見した場合は、その状態が期待する案件に属することを検証する。安全に完了済みと判断できる工程は再利用し、不一致や上書きが必要な状態では対象と理由を示して停止する。

### 5.3 `sbxm open [project]`

対象案件で日常作業を始める。

実行内容:

1. 対象案件を解決する
2. Docker Engineへ接続できることを確認する
3. Docker Sandboxes daemonがSSH Agentを引き継がない状態を保証する
4. Sandboxが`stopped`の場合は端末を占有せずに起動する
5. Sandboxが起動済みの場合はそのまま使用する
6. `ssh <sandbox-name>.sbx`で接続する
7. 接続後に開くrepository pathを表示する

通常の開始位置はDockerfileのshell設定により`/home/agent/work`とする。repository clone済みでも、MVPではSSH commandを複雑化せず、`/home/agent/work/<repository-name>`を接続時に表示する。

daemonがホストの再起動後などに新しく起動する最初の`open`では、`sbx daemon stop`後に`SSH_AUTH_SOCK`を除外した`sbx ls`でdaemonを起動する。すでに安全なdaemonで別案件を利用中の場合は、案件切り替えのたびに再起動しない。

Docker Sandboxesがdaemonの起動環境を判定できる機械可読な手段を提供しない場合、MVPでは`sbxm`が安全に起動したdaemonを識別するruntime markerを用いる。markerは再生成可能なruntime情報とし、MacまたはDocker Desktop再起動後に古いmarkerを信用しない判定方法を実装前に検証する。

### 5.4 `sbxm stop [project...]`

当面使用しないSandboxを停止する。内部Git repository、設定、package、Docker imageは保持される。

引数なしで案件directory内から実行した場合は、その案件だけを停止する。複数案件をまとめて停止する場合は対象を明示する。

```text
sbxm stop owner/foo owner/bar
```

停止済みのSandboxは成功として扱う。MVPでは全Sandboxを暗黙に停止するoptionを設けない。

### 5.5 `sbxm status [project]`

案件の構成と状態を読み取り専用で表示する。

引数またはcurrent directoryから案件を特定できた場合は、次の詳細を表示する。

- project root
- Sandbox名と稼働状態
- 中立Workspace
- ホスト側cloneの有無
- Sandbox内cloneの有無
- Dockerfileと案件メタデータの有無
- GitHub secretの登録有無
- SSH Agent露出チェックの結果

案件を指定せず、current directoryからも特定できない場合は、`base_path`以下の案件メタデータを探索し、管理案件と稼働状態の一覧を表示する。

機密値は表示しない。修復や状態変更は行わない。

### 5.6 `sbxm rm [project]`

対象Sandboxの内部状態を破棄する。

削除前に次を表示し、明示確認を必須とする。

- 削除対象の案件とSandbox名
- Sandbox内のGit status
- 現在のbranch
- 直近のcommit
- Git管理外ファイルが残っている可能性
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

案件のhost側repository内にいる場合は引数を省略できる。

```text
sbxm open
```

`open`がDocker Engine、daemon、Sandboxの状態を確認し、必要なものだけを起動してSSH接続する。

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

任意のdirectoryで引数なしに実行すると、全管理案件の一覧を表示する。

```text
sbxm status
```

案件を指定するか案件directory内で実行すると、詳細を表示する。

```text
sbxm status owner/foo
```

### 6.4 業務終了時

エディタの未保存ファイルを保存し、Codex、Claude Code、開発server、test watcherを終了してから、その日に使用したSandboxを明示的に停止する。

```text
sbxm stop owner/foo owner/bar owner/baz
sbxm status
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
- `doctor`: `status`へ最低限の前提確認を含める
- `ports`: 既存の`sbx ports`コマンドを案内する
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
    └── rm.rs
```

- `cli`: 6つの公開コマンドとargumentの定義
- `config`: TOMLの読み書き、version検証
- `project`: 案件探索、対象解決、案件メタデータ
- `paths`: 全導出pathの一元管理
- `command`: 外部process実行、環境変数制御、error整形
- `sandbox`: `sbx`、`docker`を使う再利用可能な内部操作
- `git`: host cloneとSandbox内Git初期化
- `templates`: 組み込みDockerfile
- `workflow`: 利用者目的ごとの内部工程と再開判定

### 8.2 主なdependency

- `clap`: CLI parser
- `serde`と`toml`: 設定形式
- `thiserror`: 利用者向けに文脈を付けたerror
- `dirs`: home directoryの解決

外部コマンドはRust libraryで再実装せず、引数配列を使って`git`、`docker`、`sbx`、`ssh`を呼び出す。shell文字列を組み立てないことで、owner名やpathのshell injectionを避ける。

### 8.3 workflowの再開

`add`の各内部工程は次のinterfaceを持つ単位として設計する。

1. 現在状態を検査する
2. 未完了なら実行する
3. 完了状態を検証する
4. 次の工程へ進む

途中で利用者操作や外部command失敗が発生しても、同じ`add`を再実行できるようにする。

独自の進捗flagだけを根拠に工程をskipしない。ファイル、Git remote、Docker image、Template、Sandbox、secret、Sandbox内cloneなど、実際の成果物を確認する。

不完全な成果物を自動削除してやり直さない。安全に再利用できない状態では、対象と手動復旧方法を表示して停止する。

### 8.4 安全性の共通規則

- ownerは`[A-Za-z0-9-]+`、repository名は`[A-Za-z0-9._-]+`で検証する
- `base_path`は絶対pathかつ末尾のslashを除いた形で保持する
- pathを文字列連結せず`PathBuf`で構築する
- secret値をcommand argument、log、configへ書かない
- 外部commandの失敗時はstatusと安全なstderrを示す
- 破壊的操作は対象を完全な名前で表示し、明示確認を要求する
- 既存ファイルを既定で上書きしない
- `SSH_AUTH_SOCK`を除外すべきprocessを一箇所に集約してtestする
- Sandboxの存在判定は部分一致を使わず、可能なら`sbx`の機械可読出力を利用する
- 複数案件操作では、全対象を解決・検証してから状態変更を始める

## 9. 実装順

### Phase 1: `init`と共通基盤

- Cargo projectを作成する
- command parserと共通error表示を実装する
- 前提commandとversionの検出を実装する
- global configのschema、permission、読み書きを実装する
- 案件metadata、案件探索、導出pathを実装する
- `sbxm init`を実装する
- config、入力値、path導出のunit testを追加する

完了条件:

- 未導入の前提commandと未起動のDocker Engineを明確に報告できる
- `sbx setup ssh`までの初回準備を一巡できる
- 一時HOMEを用いたtestで`~/.sbxm/config.toml`を安全に作成できる
- `init`を再実行して既存設定を上書きしない
- 不正なowner、repository、base pathを拒否できる

### Phase 2: `add`

- Dockerfileを組み込みtemplateとして追加する
- host側directory作成、SSH clone、案件metadata作成を実装する
- image build、save、Template load、Sandbox createを実装する
- Claude settingsの条件付きコピーを実装する
- GitHub secret確認と再開導線を実装する
- Sandbox内Git identity、HTTPS cloneを実装する
- 工程ごとの状態検査と再開判定を実装する
- 外部commandをfake化したintegration testを追加する

完了条件:

- `sbxm add owner/repository`だけで新規案件を作業可能な状態まで構築できる
- secret登録で中断した後、同じcommandで再開できる
- 完了済み工程を安全に再利用できる
- 不一致や不完全な成果物を暗黙に上書き・削除しない
- daemonをSSH Agentなしで起動する
- tokenやホストcredentialを成果物へ残さない

### Phase 3: `open`、`stop`、`status`

- 安全なdaemon起動判定とruntime markerを実機検証する
- Sandboxの冪等な起動を実装する
- SSH接続を実装する
- 単一・複数案件の停止を実装する
- 全管理案件の探索と状態一覧を実装する
- 案件詳細と隔離状態の診断を実装する

完了条件:

- その日の最初の`open`でdaemonをSSH Agentなしにできる
- 同じ日の案件切り替えでdaemonを不要に再起動しない
- 停止中、起動中のどちらからでも`open`できる
- 複数Sandboxを明示的にまとめて停止できる
- `status`を任意のdirectoryから利用できる
- 状態表示で機密値を出力しない

### Phase 4: `rm`と手動検証

- Sandbox内の保存状態確認を実装する
- 削除対象と保持対象の表示を実装する
- 明示確認後のSandbox削除を実装する
- end-to-end手動検証手順をREADMEへ追加する

完了条件:

- 未保存作業を確認せずSandboxを削除できない
- `rm`がホスト側projectや成果物を削除しない
- `init`、`add`、日常利用、`rm`まで一巡できる

## 10. 検証方針

自動testでは外部環境を変更せず、次を確認する。

- TOMLのround tripとschema version拒否
- path導出とcurrent directoryからの案件探索
- 案件argumentの一意な解決
- 入力値検証
- 外部commandへ渡すprogram、arguments、cwd、environment
- `SSH_AUTH_SOCK`の除外
- 既存fileの非上書き
- `add`の各工程のskip、実行、中断、再開
- 各外部command失敗時の後続処理停止
- `open`と`stop`の冪等性
- 複数Sandbox停止時の対象限定
- `rm`の確認と保持対象

実機でのみ確認できる内容は、専用のtest repositoryを使って手動検証する。

1. `sbxm init`による前提確認、global config、SSH連携
2. `sbxm add`によるhost側clone、image build、Template load
3. secret未登録による安全な中断
4. secret登録後の`add`再開とSandbox内clone
5. 中立Workspaceとホストpathの非露出
6. その日の最初の`open`と安全なdaemon起動
7. Sandbox内の`SSH_AUTH_SOCK`と`ssh-add -L`
8. Remote SSH接続と開始directory
9. 2案件目の`open`でdaemonを不要に再起動しないこと
10. 複数案件の起動、切り替え、一括停止
11. stop後の状態保持と翌日の再起動
12. 任意のdirectoryからの全案件`status`
13. `rm`前の保存確認と削除後の保持対象

## 11. 最初の利用後にレビューする論点

MVPを実案件またはtest repositoryで一巡した後、次をレビューする。

- `init`、`add`、`open`、`stop`、`status`、`rm`の語彙と粒度は自然か
- `add`の中断理由と再開方法が利用者に明確か
- daemon再起動確認が日常利用の妨げにならないか
- runtime markerによる安全なdaemonの判定は十分に堅牢か
- 案件argumentを毎回`owner/repository`で指定する負担は許容できるか
- `open`接続後にrepository rootへ自動移動する必要があるか
- Dockerfileを利用者が編集できる生成物にした判断は適切か
- 次に`rebuild`、`export`、`ports`、`doctor`のどれを追加すべきか
- 案件単位のGit identity上書きが必要か
- host側project全体を削除する操作が必要か

このレビューを行うまでは、公開コマンド、対応環境、設定項目を増やさない。
