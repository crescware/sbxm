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
| 3 | secret未登録で`sbxm add <owner>/<repo>` | 1 | `github-secret-missing`で停止し、登録commandを示す |
| 4 | secret登録後に同じ`sbxm add` | 0 | Sandboxを再利用して続きから進む |
| 5 | `sbxm add <owner>/<repo>` | 0 | remote default branchをtrackingするattached worktreeが1つ |
| 6 | `sbxm add <owner>/<repo2> --worktrees 3 --detach develop` | 0 | `origin/develop`の同じcommitから3つ |
| 7 | Sandbox内で手動のworktreeを追加 | - | 以降のunmanaged caseの前提 |
| 8 | Sandboxのworkspaceを確認 | - | 案件pathもuser homeも見えない |
| 9 | Sandbox内で`ssh-add -L`と`docker info` | 非ゼロ | agentの鍵もhostのDocker socketもない |
| 10 | Sandbox内でCodex、Claude Code、`gh auth status` | 0 | 必要なnetworkへ到達する |
| 11 | stoppedとrunningから`sbxm open <owner>/<repo>` | 0 | どちらも接続でき、stoppedは起動してから接続する |
| 12 | session接続中に別案件を`sbxm open` | 1 | `daemon-session-active`となり、daemonを変更しない |
| 13 | `sbxm stop <a> <b>`を2回 | 0 | 1回目は両方停止、2回目はno-op |
| 14 | `sbxm ls` | 0 | running、stopped、not-createdと、管理外Sandboxが分かれて出る |
| 15 | `sbxm status <owner>/<repo>` | 0または1 | managed/unmanaged、dirty、SSH Agentが診断される |
| 16 | `sbxm sync-files <owner>/<repo>` | 0 | 宣言fileだけが変わる |
| 17 | Dockerfile変更なしで`sbxm rebuild` | 0 | 適用対象がないことを表示する |
| 18 | Dockerfileを壊してから`sbxm rebuild` | 1 | buildが失敗し、既存Sandboxはそのまま動く |
| 19 | session、dirty、unpushedのある状態で`sbxm rebuild` | 1 | `daemon-session-active`または`unsaved-work` |
| 20 | case 7のunmanaged worktreeがある状態で`sbxm rebuild` | 1 | 再現できないworktreeを示す`unsaved-work` |
| 21 | stopped Sandboxへ`sbxm rebuild` | 1 | `sbxm open`を案内する |
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

Phase 2仕様のdaemon安全性probeもここへ記録する。`SSH_AUTH_SOCK`の有無でagentが
Sandboxへ転送されるか、session検査がactiveと不在を区別できるか、daemon再起動後に
Sandboxを再利用できるかの4点である。
