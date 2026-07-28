# sbxm

案件ごとのDocker Sandboxを構築、接続、診断、破棄するRust製CLI。

## 手動検証

自動testは外部commandをすべてfakeへ差し替えるため、実機のDocker Sandboxes CLIに
ついては何も保証しない。この節は、実機に対してsbxmを実行した記録である。各caseに
実行command、期待exit code、期待結果を並べてあるので、実行時に観測結果とCLIのversion
を書き足す。

対象platformでだけ実行する。macOS 14以降のApple silicon、Docker Desktop、
Docker Sandboxes CLI 0.37.0以降。

### 実行前

- private test repositoryを使う。実案件を最初の検証対象にしない。
- 自分の設定に触れないよう、使い捨てのHOMEで実行する:
  `env HOME="$(mktemp -d)" sbxm ...`
- versionを正確に記録する: `sbx version`、`docker version`

### GitHub token

`prepare`はSandboxの中からrepositoryをcloneする。SandboxにはSSH Agentが届かないため、Docker Sandboxesのsecretとして保存したtokenで認証する。`add`がSandbox名と登録commandを表示するので、tokenは`add`と`prepare`の間で登録する。

tokenは`github` service secretではなく、custom secretとして登録する。

```
sbx secret set-custom <sandbox> --host github.com --env GH_TOKEN --value <token>
```

custom secretはSandboxへplaceholderだけを見せ、本物のtokenはproxyに留める。proxyがgithub.com宛のrequest headerでplaceholderを本物へ差し替える。tokenはSandboxへ入らず、tokenの種類も問わない。`github` service secretを使わないのは、実機でfine-grained tokenは認証できたのにclassic tokenは認証されないままだったためである。

custom secretはSandboxの作成時に結び付くため、あとから登録しても既存のSandboxには届かない。`prepare`は何かを作る前にsecretを確認し、作成後にSandboxの中を見てplaceholderが届いたことを確かめる。

対象repositoryをread/writeできるtokenを発行する。

| token | 設定 |
|---|---|
| fine-grained | Contents read/write、Metadata read |
| fine-grained、任意 | Pull requests、Issues、Actionsは作業に必要な場合だけ |
| classic | `repo` scope |

この要件は`add`も表示するので、暗記する必要はない。

### Redaction

記録する前に次を除く。

- あらゆるtokenとsecretの値
- path中のmacOS user名。`<user>`へ置き換える
- SSH公開鍵とagent socketのpath
- test repositoryが非公開の場合はrepository名

### Case

| # | command | 期待exit | 期待結果 |
|---:|---|---:|---|
| 1 | 新しいHOMEで`sbxm init` | 0 | `~/.sbxm/config.toml`がmode 0600で作られる |
| 2 | `sbxm --lang ja init`、`sbxm --lang en init` | 0 | helpと出力が選択言語に従う |
| 3 | `sbxm add <owner>/<repo>` | 0 | 案件を登録し、host cloneを取り、Sandbox名とtoken登録commandを示す |
| 4 | secretを登録してから`sbxm prepare <owner>/<repo>` | 0 | Sandboxとrepositoryを一度の実行で構築する |
| 5 | secret未登録で`sbxm prepare <owner>/<repo>` | 1 | imageを組む前、Sandboxを作る前に`github-secret-missing`で停止し、登録commandを示す |
| 5a | Sandbox作成後にsecretを登録して`sbxm prepare` | 1 | `sandbox-secret-not-applied`で停止し、作り直すための`sbx rm`を示す |
| 5b | Sandboxの中でbare repositoryに対し`git ls-remote origin` | 0 | placeholderで認証され、usernameを尋ねられない |
| 6 | `sbxm add <owner>/<repo2> --worktrees 3 --detach develop` のあと `sbxm prepare` | 0 | `origin/develop`の同じcommitから3つ |
| 7 | Sandbox内で手動のworktreeを追加 | - | 以降のunmanaged caseの前提 |
| 8 | Sandboxのworkspaceを確認 | - | 案件pathもuser homeも見えない |
| 9 | Sandbox内で`ssh-add -L`と`docker info` | 非ゼロ | agentの鍵もhostのDocker socketもない |
| 10 | Sandbox内でCodex、Claude Code、`gh auth status` | 0 | 必要なnetworkへ到達する |
| 11 | stoppedとrunningから`sbxm open <owner>/<repo>` | 0 | どちらも接続でき、stoppedは起動してから接続する |
| 13 | `sbxm stop <a> <b>`を2回 | 0 | 1回目は両方停止、2回目はno-op |
| 14 | `sbxm ls` | 0 | running、stopped、not-createdと、管理外Sandboxが分かれて出る |
| 15 | `sbxm status <owner>/<repo>` | 0または1 | managed/unmanaged、dirty、SSH Agentが診断される |
| 16 | `sbxm sync-files <owner>/<repo>` | 0 | 宣言fileだけが変わる |
| 17 | Dockerfile変更なしで`sbxm rebuild` | 0 | 適用対象がないことを表示する |
| 18 | Dockerfileを壊してから`sbxm rebuild` | 1 | buildが失敗し、既存Sandboxはそのまま動く |
| 19 | dirty、unpushedのある状態で`sbxm rebuild` | 1 | 失われる対象を示す`unsaved-work` |
| 20 | case 7のunmanaged worktreeがある状態で`sbxm rebuild` | 1 | 対象worktreeと削除方法を示す`unmanaged-worktree-present` |
| 21 | stopped Sandboxへ`sbxm rebuild` | 0 | 保存状態を読むために起動してから再構築する |
| 22 | cleanでmanagedだけのSandboxへ`sbxm rebuild` | 0 | 新世代が適用される |
| 23 | case 22をSandbox削除直後に中断して再実行 | 0 | 記録済みの世代から継続する |
| 24 | case 22の後に`sbxm status <owner>/<repo>` | 0 | 新しいDockerfile hash、worktree、file、Git identityが揃う |
| 25 | dirty managed worktreeがある状態で`sbxm destroy` | 1 | `unsaved-work` |
| 26 | dirty unmanaged worktreeがある状態で`sbxm destroy` | 1 | `unsaved-work` |
| 27 | unpushed commitがある状態で`sbxm destroy` | 1 | `unsaved-work` |
| 28 | cleanな案件へ`sbxm destroy`し、Sandbox名を入力 | 0 | 入力一致後に削除される |
| 29 | 保存されていない作業のあるrunningへ`sbxm destroy --force` | 0 | 省略した検査を明示して削除する |
| 30 | stopped Sandboxへ`sbxm destroy --force` | 0 | 削除される |
| 31 | 非対話shellで案件を完全指定した通常modeとforce mode | 0 | どちらもpromptを出さない |
| 32 | destroy後にhostを確認 | - | host clone、Dockerfile、image、Template、workspace、secretが残る |
| 33 | destroy後に`.sbxm`を確認 | - | metadata、lock file、cacheがない |
| 34 | destroy後に`sbxm open <owner>/<repo>` | 1 | 管理対象ではないと示す |
| 35 | もう一度`sbxm add <owner>/<repo>` | 0 | 新規案件として登録される |
| 36 | 残ったDockerfileでcase 35を実行 | 0 | 初回buildが残存Dockerfileを採用する |

### 結果

| 項目 | 値 |
|---|---|
| 実施日 | 未実施 |
| macOS / arch | |
| `sbx version` | |
| `docker version` | |
| 合格case数 | |
| 不合格case数 | |

### daemon安全性probe

Phase 2仕様の項目もここへ記録する。記録の仕方はcaseと同じである。

| # | 確認する内容 |
|---:|---|
| 1 | `SSH_AUTH_SOCK`ありで起動したdaemonがSandboxへagentを転送すること |
| 2 | `SSH_AUTH_SOCK`をunsetして起動したdaemonでは転送されないこと |
| 3 | daemon停止・起動後にSandboxを再利用または作成できること |
