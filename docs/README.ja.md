# sbxm

`sbxm`は、GitHubプロジェクトごとに専用のDocker Sandboxと、構成の予測できる
Git worktree一式を用意します。ホスト側のclone、Sandbox image、repositoryのセットアップ、
日常的な接続、診断、再構築、破棄までを扱います。

Sandboxには、指定したGit identityと設定ファイルだけが渡されます。ホスト側の
プロジェクトディレクトリ、Docker socket、SSH agentは渡されません。GitHubの認証情報は
Sandboxへコピーせず、Docker Sandboxesのsecret proxyを通じて提供します。

English: [README.md](../README.md)

## 必要要件

- macOS 14以降を搭載したApple silicon Mac
- Docker Engineが起動しているDocker Desktop
- **[Docker Sandboxes CLI 0.37.0以降](https://docs.docker.com/ai/sandboxes/get-started/)**
- GitとSSH
- 管理対象のrepositoryごとに発行したGitHub personal access token

初期設定後に`sbxm status --global`を実行すると、これらの要件とDocker Sandboxes環境を
確認できます。

## インストール

> [!WARNING]
> インストール手段はまだ提供していません。この節は、予定しているHomebrewでの
> インストール方法を示すドラフトです。

```sh
brew install crescware/tap/sbxm
```

## クイックスタート

### 1. sbxmを初期設定する

```sh
sbxm init
```

対話形式のセットアップによって`~/.sbxm/config.toml`が作成され、次の項目を尋ねられます。

- ホスト側のプロジェクトcloneを置くディレクトリ
- Sandbox内で使うGitの名前とメールアドレス

表示言語はシステムのlocaleから選ばれ、必要な場合は選択内容の確認を求められます。

非対話形式でセットアップする場合は、3つの設定値をすべて指定します。

```sh
sbxm init \
  --lang ja \
  --base-path "$HOME/Projects" \
  --git-user-name "Your Name" \
  --git-user-email "you@example.com"
```

続いて、ホスト環境を検証します。

```sh
sbxm status --global
```

### 2. プロジェクトを登録する

プロジェクトは`owner/repository`形式で指定します。

```sh
sbxm add owner/repository
```

このコマンドはプロジェクトを登録し、ホスト側のcloneとDockerfileを作成したうえで、
Sandbox名と次に実行する正確なコマンドを表示します。この時点ではまだSandboxを
構築しません。

デフォルトでは、repositoryのdefault branch上にworktreeを1つ作成します。独立した
worktreeを複数用意する場合は、起点となるbranchとdetached modeを指定します。

```sh
sbxm add owner/repository --detach main --worktrees 3
```

複数のagentやタスクで作業ディレクトリを分離したい場合に、detached worktreeが役立ちます。
指定できる個数は1〜32です。

### 3. GitHubの認証情報を登録する

repositoryを読み書きできるpersonal access tokenを発行します。

- fine-grained tokenには**Contents: read and write**と**Metadata: read**が必要です。
- classic tokenには`repo` scopeが必要です。

`sbxm add`は、プロジェクト専用の`sbx secret set-custom`コマンドを表示します。
プロジェクトをprepareする前に、そのコマンドへtokenを渡して実行してください。
表示されるコマンドは次のような形です。

```sh
sbx secret set-custom <sandbox> \
  --host github.com \
  --host '**.github.com' \
  --host '**.githubusercontent.com' \
  --host ghcr.io \
  --env GH_TOKEN \
  --value <token>
```

secret proxyにより、本物のtokenはSandboxの外側に保たれます。Sandboxから見えるのは
placeholderだけであり、登録済みのhostへのrequestに限ってproxyが本物のtokenへ
置き換えます。

### 4. Sandboxを構築して接続する

```sh
sbxm prepare owner/repository
sbxm open owner/repository
```

`prepare`はプロジェクトのimageをbuildし、Sandboxを作成して、その中へrepositoryを
cloneし、managed worktreeを作成します。`open`は必要に応じて停止中のSandboxを起動し、
SSHで接続します。

Sandbox内のworktreeは次の場所にあります。

```text
/home/agent/work/<repository>/<repository>.tree-1
/home/agent/work/<repository>/<repository>.tree-2
...
```

## 日常的な操作

```sh
# 管理対象の全プロジェクトとSandboxの状態を表示する
sbxm ls

# 変更を加えずに1つのプロジェクトを検査する
sbxm status owner/repository

# プロジェクトへ接続する
sbxm open owner/repository

# 1つ以上のプロジェクトを削除せずに停止する
sbxm stop owner/repository
sbxm stop owner/repository another/project
```

対話端末で実行した場合、`open`、`stop`、`destroy`はプロジェクト引数を省略すると
対象を選択するpromptを表示できます。

## プロジェクトをカスタマイズする

### Sandbox imageを編集する

`sbxm add`はプロジェクトのホスト側ディレクトリにDockerfileを作成します。ツールや
system dependencyを追加するにはこのファイルを編集し、変更を適用します。

```sh
sbxm rebuild owner/repository
```

rebuildはSandboxを作り直します。作業内容を保護するため、dirty file、pushしていない
commit、またはunmanaged worktreeがある場合、sbxmは通常のrebuildを拒否します。

### managed worktreeを追加する

構築済みのプロジェクトには、rebuildせずにmanaged worktreeを追加できます。

```sh
sbxm apply owner/repository --worktrees 4
```

worktree数は増やすことだけができます。デフォルトのattached modeで登録したプロジェクト
では、最初のworktreeはtracking branch上に残り、追加のworktreeはdetachedになります。

### 設定ファイルを配置する

ホスト側のファイルを`~/.sbxm/config.toml`に宣言します。

```toml
[[files]]
source = "/Users/you/.config/example/config.toml"
destination = ".config/example/config.toml"

[[files]]
source = "/Users/you/.config/another-tool/settings.toml"
destination = ".config/another-tool/settings.toml"
```

配置先はSandbox userのhome directoryからの相対pathです。宣言したファイルは
`prepare`の実行時に配置されます。あとから加えた変更は明示的に適用します。

```sh
sbxm apply owner/repository --files
```

`--files`は宣言された配置先を上書きします。token、private keyなどの認証情報は
これらのファイルに含めず、Docker Sandboxesのsecretを使用してください。

2つのapply対象は同時に指定できます。

```sh
sbxm apply owner/repository --files --worktrees 4
```

## プロジェクトを破棄する

```sh
sbxm destroy owner/repository
```

sbxmは何かを削除する前に、削除するものと残すものを表示します。通常のdestroyでは、
dirty worktree、pushしていないcommit、active sessionを検査します。対話端末では、
続いてSandbox名の入力を求めます。

Sandboxとsbxmのプロジェクトmetadataは削除されます。ホスト側のclone、プロジェクトの
Dockerfile、build済みimage、load済みtemplate、登録済みsecretは残るため、あとから
プロジェクトを再登録できます。

データ保護とactive sessionの検査を意図的に省略する必要がある場合は、次を実行します。

```sh
sbxm destroy --force owner/repository
```

Sandbox内に残すべきものがないと別途確認できた場合に限って、`--force`を使用してください。

## コマンド一覧

| コマンド | 用途 |
|---|---|
| `sbxm init` | global configを作成する |
| `sbxm add owner/repository` | GitHubプロジェクトを登録し、ホスト側の成果物を作成する |
| `sbxm prepare owner/repository` | プロジェクトのSandboxをbuildして構築する |
| `sbxm open [owner/repository]` | 必要に応じてSandboxを起動し、SSHで接続する |
| `sbxm stop [owner/repository ...]` | 1つ以上のSandboxを停止する |
| `sbxm ls` | 管理対象のプロジェクトとSandboxの状態を一覧表示する |
| `sbxm status --global` | ホストとDocker Sandboxes環境を診断する |
| `sbxm status owner/repository` | 変更を加えずにプロジェクトを診断する |
| `sbxm apply owner/repository ...` | 宣言済みファイルを配置するか、managed worktreeを追加する |
| `sbxm rebuild owner/repository` | 編集したDockerfileからSandboxを作り直す |
| `sbxm destroy [owner/repository]` | Sandboxを削除し、プロジェクトの管理を終了する |

完全なCLI referenceは、`sbxm --help`または`sbxm <command> --help`で確認できます。
