# sbxm MVP 実装計画

## 1. 目的

`sbxm` は、Codex・Claude Code向けDocker Sandboxの案件別セットアップと日常操作を自動化するRust製CLIである。

初版では汎用的なSandbox管理ツールを目指さず、既存の運用手順を安全かつ再現可能に実行できることに集中する。実際の案件で使い、操作感を確認した後に設定項目や対応環境を拡張する。

## 2. MVPの前提

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
- 既存Sandboxを暗黙に削除または上書きしない
- `sbx`が保持する稼働状態を`sbxm`側へ複製しない

## 3. 設定と生成物

### 3.1 マシングローバル設定

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

`sbxm`は設定ファイルと親ディレクトリを必要時に作成し、ディレクトリのpermissionを`0700`、設定ファイルを`0600`とする。

### 3.2 案件メタデータ

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

Sandbox名、Template image名、project root、Dockerfile path、cache path、中立Workspace、Sandbox内clone先は、上記メタデータとglobal configから毎回導出する。導出可能な値を保存して不整合を作らない。

### 3.3 案件ディレクトリ

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

Dockerfileは利用者が確認・編集できる案件別ファイルとして生成する。MVPでは組み込みテンプレートから初回だけ作り、既存ファイルを暗黙に上書きしない。

## 4. コマンド設計

MVPで提供するコマンドを次に絞る。

### 4.1 `sbxm machine setup`

Macで最初の1回だけ必要な準備を案内・確認する。

実行内容:

1. macOS versionとCPU architectureを確認する
2. `brew`、Docker Client・Server、`gh`、`sbx`の存在とversionを確認する
3. Docker Engineへ接続できることを確認する
4. `sbx`が未導入の場合は、公式のHomebrew installコマンドを表示する
5. `sbx login`が必要な場合は対話commandを起動する
6. network policyを表示し、未設定の場合は`Balanced`を選ぶよう案内する
7. `sbx setup ssh`を実行する
8. `sbxm config init`相当のglobal config作成を行う

Homebrew packageのinstallはマシングローバルな変更となるため自動実行せず、正確なコマンドを表示して終了する。利用者がinstall後に再実行すると、残りの確認から続行する。

このcommandは再実行可能とする。完了済みの項目は成功として扱い、既存global configは上書きしない。

### 4.2 `sbxm config init`

global configを対話的に作成する。

- `base_path`
- 既定のGit user name
- 既定のGit user email

既存configがある場合は上書きせず終了する。変更用コマンドはMVPに含めず、ファイルを直接編集してもらう。

### 4.3 `sbxm init <github-owner>/<repository-name>`

案件のホスト側構成を初期化する。

実行内容:

1. global configを読む
2. ownerとrepository名を検証する
3. project directory、`.sbx/exports`、`.sbx/.cache`を作成する
4. ホスト側repositoryをSSH URLでcloneする
5. `sbxm.toml`を作成する
6. 標準Dockerfileを作成する

既存ファイルや既存cloneを発見した場合は、処理済みの範囲と衝突箇所を示して停止する。MVPでは不完全なdirectoryを推測して引き継がない。

### 4.4 `sbxm create`

案件directory内で実行し、Sandboxを作成する。

実行内容:

1. 親directoryを上へ辿って`.sbx/sbxm.toml`を見つける
2. 必須コマンドとversionを確認する
3. 同名Sandboxが存在しないことを確認する
4. 中立Workspaceを作成する
5. 案件別Dockerfileをbuildする
6. imageを`.sbx/.cache/template.tar`へ保存する
7. TemplateをDocker Sandboxes runtimeへloadする
8. SSH Agentを外した環境でSandboxを作成する
9. ホストの`~/.claude/settings.json`が存在する場合だけ、安全なpermissionでコピーする
10. 作成結果と次に必要な操作を表示する

daemonがすでにSSH Agent付きで起動している可能性は、`sbx create`の子processから環境変数を外すだけでは解消できない。そのため`create`前にdaemonを停止し、`SSH_AUTH_SOCK`なしで再起動する処理を含める。

daemon停止は他のSandbox操作へ影響し得るため、実行前に何をするかを表示して確認を求める。非対話実行への対応はMVP利用後に検討する。

### 4.5 `sbxm project setup`

作成済みSandbox内の初期設定を行う。

実行内容:

1. Git user nameとemailをSandbox内global configへ設定する
2. GitHub CLIのGit protocolをHTTPSへ設定する
3. `gh auth status`で案件用secretの利用可否を確認する
4. repositoryを`/home/agent/work/<repository-name>`へcloneする
5. `mise.toml`、`.mise.toml`、`.tool-versions`の有無を報告する

`mise trust`と`mise install`はrepository由来コードの実行につながるため、自動実行しない。必要なコマンドを案内する。

GitHub fine-grained personal access tokenの発行はブラウザ操作が必要であり、`sbxm`は自動化しない。`project setup`の前に実行する正確な`sbx secret set`コマンドを表示する。secret未登録の場合はcloneを開始せず、登録手順を示して終了する。

### 4.6 `sbxm status`

案件の構成と状態を読み取り専用で表示する。

- project root
- Sandbox名
- Sandboxの稼働状態
- 中立Workspace
- ホスト側cloneの有無
- Sandbox内cloneの有無
- Dockerfileと案件メタデータの有無
- SSH Agent露出チェックの結果

機密値は表示しない。

### 4.7 `sbxm start`

対象Sandboxを日常作業のために起動する。

実行内容:

1. Docker Engineへ接続できることを確認する
2. Docker Sandboxes daemonがSSH Agentを引き継がない状態を保証する
3. 対象Sandboxの現在状態を確認する
4. `stopped`の場合は`sbx exec <sandbox-name> /bin/true`で端末を占有せずに起動する
5. すでに起動中の場合は成功として扱う
6. Sandbox名、状態、SSH hostname、repository pathを表示する

daemonがホストの再起動後などに新しく起動する日の最初の実行では、`sbx daemon stop`後に`SSH_AUTH_SOCK`を除外した`sbx ls`で起動する。すでに安全なdaemonで別案件を利用中の場合は、案件切り替えのたびにdaemonを再起動しない。

Docker Sandboxesがdaemonの起動環境を判定できる機械可読な手段を提供しない場合、MVPでは`sbxm`が当日そのdaemonを安全に起動したことをprocess外のruntime markerで記録する。markerは再生成可能なruntime情報であり、global configには保存しない。MacまたはDocker Desktop再起動後に古いmarkerを信用しない判定方法は実装前に検証する。

### 4.8 `sbxm shell`

`sbxm start`と同じ安全確認と起動処理を行った後、`ssh <sandbox-name>.sbx`へ接続する。

通常の開始位置はDockerfileのshell設定により`/home/agent/work`とする。repository clone済みの場合でも、MVPではSSH commandを複雑化せず、接続後に移動先を表示する。

### 4.9 `sbxm stop`

対象Sandboxを停止する。内部Git repository、設定、package、Docker imageは保持される。

複数案件をまとめて停止するため、案件directory外ではSandbox名を複数指定できるようにする。

```text
sbxm stop <sandbox-name-1> <sandbox-name-2> <sandbox-name-3>
```

引数なしで案件directory内から実行した場合は、その案件だけを停止する。MVPでは全Sandboxを暗黙に停止するoptionを設けない。

### 4.10 `sbxm destroy`

Sandboxを削除する。

削除前に次を表示し、明示確認を必須とする。

- 削除対象のSandbox名
- Sandbox内Git status
- 現在のbranch
- 直近のcommit
- Git管理外ファイルが残っている可能性

確認後に`sbx rm`を実行する。ホスト側project directory、中立Workspace、Template archive、Docker imageは削除しない。

## 5. 日常利用の流れ

### 5.1 その日の最初

Docker Desktopを起動した後、最初に使う案件directoryで次を実行する。

```text
sbxm start
```

`sbxm start`はDocker Engineを確認し、Docker Sandboxes daemonをSSH Agentなしで安全に起動したうえで、対象Sandboxを起動する。初回だけdaemon再起動が必要な場合は、他の稼働中Sandboxへの影響を表示して確認を求める。

接続も同時に行う場合は次だけでよい。

```text
sbxm shell
```

### 5.2 案件を切り替える

次の案件のhost側repositoryまたは`.project`配下へ移動し、起動または接続する。

```text
sbxm shell
```

短時間で戻るSandboxは起動したままでよい。当面戻らない案件は、その案件directoryで停止する。

```text
sbxm stop
```

現在の状態は任意の案件directoryで確認できる。

```text
sbxm status
```

MVPでは案件選択UIや`switch` commandを作らず、current directoryを案件選択として利用する。

### 5.3 複数案件を扱う

同時に必要な各案件で`sbxm start`を実行する。3案件程度を同時利用する場合も、daemonの安全な起動確認は最初の1回だけ行い、各Sandboxを独立して起動する。

CPUやmemoryが不足した場合は、不要な開発server、test watcher、Sandboxの順に停止する。MVPではresource tuningを自動化しない。

### 5.4 業務終了時

エディタの未保存ファイルを保存し、Codex、Claude Code、開発server、test watcherを終了してから、その日に使用したSandboxを明示的に停止する。

```text
sbxm stop <sandbox-name-1> <sandbox-name-2> <sandbox-name-3>
```

続けて`sbxm status`で停止状態を確認する。`stop`は内部ストレージを保持し、日常利用では`destroy`を使用しない。

## 6. MVPに含めないもの

初回実装では次を独立コマンドにしない。

- `rebuild`: `destroy`、`create`、`project setup`の組み合わせで検証する
- `export`: 既存の`sbx cp`コマンドを案内する
- `doctor`: `status`へ最低限の前提確認を含める
- `ports`: 既存の`sbx ports`コマンドを案内する
- global configの編集コマンド
- 複数host、GitLab、Linux、Intel Macへの対応
- Dockerfileテンプレートの選択機能
- CPU・memory設定
- secret値の保管または入力代行
- Codex・Claude Codeの対話login自動化
- `mise trust`およびrepository固有toolの自動install

## 7. Rust実装方針

### 7.1 crate構成

最初は単一binary crateとし、責務ごとにmoduleを分ける。

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
└── templates.rs
```

- `cli`: commandとargumentの定義
- `config`: TOMLの読み書き、version検証
- `project`: 案件探索と案件メタデータ
- `paths`: 全導出pathの一元管理
- `command`: 外部process実行、環境変数制御、エラー整形
- `sandbox`: `sbx`、`docker`を使う操作
- `git`: host cloneとSandbox内Git初期化
- `templates`: 組み込みDockerfile

### 7.2 主なdependency

- `clap`: CLI parser
- `serde`と`toml`: 設定形式
- `thiserror`: 利用者向けに文脈を付けたerror
- `dirs`: home directoryの解決

外部コマンドはRust libraryで再実装せず、引数配列を使って`git`、`docker`、`sbx`、`ssh`を呼び出す。shell文字列を組み立てないことで、owner名やpathのshell injectionを避ける。

### 7.3 安全性の共通規則

- ownerは`[A-Za-z0-9-]+`、repository名は`[A-Za-z0-9._-]+`で検証する
- `base_path`は絶対pathかつ末尾のslashを除いた形で保持する
- pathを文字列連結せず`PathBuf`で構築する
- secret値をcommand argument、log、configへ書かない
- 外部コマンドの失敗時はstatusと安全なstderrを示す
- 破壊的操作は対象を完全な名前で表示し、明示確認を要求する
- 既存ファイルを既定で上書きしない
- `SSH_AUTH_SOCK`を除外すべきprocessを一箇所に集約してtestする
- Sandboxの存在判定は`grep`相当の曖昧な部分一致ではなく、可能なら`sbx`の機械可読出力を利用する。利用できないversionでは行単位の正確一致を実装する

## 8. 実装順

### Phase 1: マシンセットアップと設定基盤

- Cargo projectを作成する
- command parserと共通error表示を実装する
- 前提commandとversionの検出を実装する
- global configのschema、permission、読み書きを実装する
- 案件metadataと導出pathを実装する
- `sbxm machine setup`を実装する
- `sbxm config init`を実装する
- configとpath導出のunit testを追加する

完了条件:

- 一時HOMEを用いたtestで`~/.sbxm/config.toml`を安全に作成できる
- 未導入の前提commandと未起動のDocker Engineを明確に報告できる
- `sbx setup ssh`までの初回準備を一巡できる
- 不正なowner、repository、base pathを拒否できる
- 主要な導出pathが文書どおりになる

### Phase 2: 案件初期化

- Dockerfileを組み込みtemplateとして追加する
- `sbxm init`を実装する
- host側SSH cloneを実装する
- 既存directory・file衝突時の停止を実装する
- dryな一時directoryを用いたtestを追加する

完了条件:

- 新規案件のdirectory、metadata、Dockerfileを一度で作成できる
- 再実行で既存データを上書きしない
- clone失敗時に途中状態と再開方法が分かる

### Phase 3: Sandbox lifecycle

- `sbxm create`を実装する
- Claude settingsの条件付きコピーを実装する
- `sbxm status`を実装する
- 安全なdaemon起動判定とruntime markerを実機検証する
- `sbxm start`、`sbxm shell`、`sbxm stop`を実装する
- 外部コマンド実行をfake化したintegration testを追加する

完了条件:

- build、save、Template load、Sandbox createの順序が保証される
- daemonをSSH Agentなしで起動する
- 同じ日の案件切り替えで不要なdaemon再起動を行わない
- 停止中、起動中のどちらへ`start`を実行しても安全に成功する
- 複数Sandboxを明示的にまとめて停止できる
- 既存Sandboxを上書きしない
- Claude settingsを必要な場合だけmode `0600`で投入する

### Phase 4: Sandbox内セットアップと削除

- `sbxm project setup`を実装する
- secret未設定時の案内を実装する
- Sandbox内Git identity、HTTPS cloneを実装する
- `sbxm destroy`と保存確認を実装する
- end-to-end手動検証手順をREADMEへ追加する

完了条件:

- 新規案件で`config init`からSandbox内cloneまで一巡できる
- tokenやホストcredentialを成果物へ残さない
- 未保存作業を確認せずSandboxを削除できない

## 9. 検証方針

自動testでは外部環境を変更せず、次を確認する。

- TOMLのround tripとschema version拒否
- path導出
- 入力値検証
- 外部commandへ渡すprogram、arguments、cwd、environment
- `SSH_AUTH_SOCK`の除外
- 既存fileの非上書き
- 各外部command失敗時の後続処理停止
- destructive confirmation
- daemonの安全な初回起動と同日中の再利用判定
- `start`、`shell`、`stop`の冪等性
- 複数Sandbox停止時の対象限定

実機でのみ確認できる内容は、専用のtest repositoryを使って手動検証する。

1. Docker Desktopと`sbx`の前提確認
2. `sbxm machine setup`とSSH連携
3. host側clone
4. image buildとTemplate load
5. Sandbox作成
6. 中立Workspaceの表示
7. その日の最初の`start`と安全なdaemon起動
8. 2案件目の`start`でdaemonを不要に再起動しないこと
9. SSH接続時の開始directory
10. `SSH_AUTH_SOCK`と`ssh-add -L`
11. GitHub secret経由のHTTPS clone
12. Remote SSH接続
13. 複数案件の起動、切り替え、一括停止
14. stop後の状態保持と翌日の再起動
15. destroy前の保存確認

## 10. 最初の利用後にレビューする論点

MVPを実案件またはtest repositoryで一巡した後、次だけをレビューする。

- `init`、`create`、`project setup`の分割は利用者の認知に合っているか
- daemon再起動確認が日常利用の妨げにならないか
- runtime markerによる「その日の最初」の判定は十分に堅牢か
- GitHub secret登録をどのcommandの導線へ置くべきか
- Dockerfileを利用者が編集できる生成物にした判断は適切か
- `shell`接続後にrepository rootへ自動移動する必要があるか
- `rebuild`、`export`、`ports`、`doctor`のうち、次に独立command化すべきものはどれか
- 案件単位のGit identity上書きが必要か
- 中断した`init`や`create`の安全な再実行をどこまで支援すべきか

このレビューを行うまでは、対応環境や設定項目を増やさない。
