# Phase 3 実装仕様: `open`、`stop`、`ls`、`status`

## 1. 目的

Phase 3は、登録済み案件の日常的な起動、接続、停止、一覧、project scopeのread-only診断を実装する。Docker Sandboxesのruntime状態は複製せず、各command実行時にstructured outputから取得する。

## 2. 共通規則

- project指定とTTY規則は方向性文書に従う
- mutation commandはproject lockを取得する
- daemon操作はproject lock後にglobal daemon lockを取得する
- 対象解決と全validationをmutation前に完了する
- `sbx` stateはPhase 1 fixtureのJSON parserで扱う
- nameは完全一致とし、substring、prefix、表示textの`grep`を使わない
- 未対応stateまたは重複nameはraw valueを示してexit code `1`

## 3. Sandbox stateの正規化

対応versionのfixtureで得たraw stateを次へ写像する。

| sbxm state | 意味 |
|---|---|
| `not-created` | metadataはあるが対応Sandboxがない |
| `running` | Sandboxが存在し起動中 |
| `stopped` | Sandboxが存在し停止中 |

raw stateを3値へ安全に写像できない場合は、対象nameとraw stateを示してerrorにする。`unknown`へ丸めない。

metadataやworkspaceとの対応が矛盾する場合、projectの管理状態は`inconsistent`となり、上記stateを正常結果として返さない。

## 4. `sbxm open [project]`

### 4.1 状態別動作

| 状態 | 動作 |
|---|---|
| `unmanaged` | exit `1`、`add`を案内 |
| `registered` / `not-created` | exit `1`、同じ目標構成の`add`再実行を案内 |
| `stopped` | daemonを安全に再起動し、Sandboxを非対話で起動してSSH接続 |
| `running` | daemonを安全に再起動してSSH接続 |
| `inconsistent` | exit `1`、`status`を案内 |

`open`はSandboxを新規作成しない。

### 4.2 処理順

1. 対象を引数または単一選択promptで解決
2. project lockを取得
3. Docker Engineへ接続確認
4. `sbx ls --json`相当を1回実行
5. Sandbox identity、workspace、stateを検証
6. 全Sandboxのactive session不在を確認し、daemonを安全に再起動
7. stoppedならfixtureで固定した非対話commandで起動
8. runningになるまで2秒間隔、最大60秒poll
9. managed worktree一覧をmetadataとGitから検証
10. 接続先とworktree一覧をstderrへ表示
11. `ssh <sandbox-name>.sbx`へterminalを引き渡す

手動手順で使用していた起動commandの候補は次であり、exact formはfixtureで固定する。

```text
sbx exec <sandbox-name> /bin/true
```

SSH childにはstdin、stdout、stderrを継承する。SSHのexit statusが0なら`sbxm`も0、利用者による通常切断以外の非ゼロは外部command失敗としてexit code `1`に写像し、原値を表示する。

通常開始directoryはDockerfileにより`/home/agent/work`とする。MVPではSSH commandへ自動`cd`を組み込まない。

### 4.3 Safe daemon

Phase 1の共通手順を使用し、`open`のたびにglobal daemon lockを取得してdaemonを安全に再起動する。

- 全Sandboxのactive session不在をstructured outputから確認する
- active sessionを検出した場合、またはsession不在を証明できない場合はdaemonを変更せずexit code `1`
- session検査commandの失敗、timeout、parse不能はexit code `1`
- session不在を確認できた場合だけdaemonを停止し、`SSH_AUTH_SOCK`をunsetした環境で起動する

毎回の再起動による所要時間はMVPで受け入れ、再起動省略はMVP利用後の非機能要件として検討する。

## 5. `sbxm stop [project...]`

### 5.1 対象

- 引数あり: 重複を除いた全projectをcanonical ID昇順に処理
- 引数なし: 全管理案件から0件以上を複数選択
- 0件確定: exit `0`

### 5.2 Validationとatomicity

1. 全対象metadataを解決
2. 1回のSandbox一覧取得で全stateを解決
3. `inconsistent`または未対応stateが1件でもあれば、何も停止せずerror
4. 全project lockをcanonical ID昇順に取得
5. stateを再取得してpreconditionを再確認
6. runningだけを停止

完全なtransaction rollbackは行わない。途中で外部commandが失敗した場合は後続対象を停止せず、対象ごとの`stopped`、`unchanged`、`failed`を表示してexit code `1`。

### 5.3 状態別動作

- `running`: `sbx stop <sandbox-name>`
- `stopped`: no-op成功
- `not-created`: no-op成功
- `inconsistent`: mutation前error

停止後は最大60秒pollし、stoppedを確認する。内部filesystem、Git、package、Docker imageを削除しない。

## 6. `sbxm ls`

### 6.1 処理

1. configからbase pathを読む
2. 全metadataを探索・検証する
3. `sbx ls --json`相当を1回実行する
4. metadataとSandboxをname完全一致で突き合わせる
5. 管理案件と未管理Sandboxを別tableで表示する

0件でもheaderを表示してexit `0`とする。

### 6.2 出力

```text
PROJECT          SANDBOX                         STATE
owner/foo        sbxm-owner-foo-0123456789ab     running
owner/bar        sbxm-owner-bar-abcdef012345     stopped
owner/baz        sbxm-owner-baz-fedcba987654     not-created
```

並び順:

- managed projects: canonical ID byte昇順
- unmanaged Sandboxes: Sandbox name byte昇順

未管理Sandboxは`UNMANAGED SANDBOXES`へname、raw state、workspaceを表示する。取り込みや削除は行わない。

### 6.3 Failure

- `sbx ls`失敗: 一覧を一切出さずexit `1`
- metadataが1件でも不正: 一覧を一切出さずexit `1`
- 未対応raw state: 一覧を一切出さずexit `1`
- 同名Sandbox複数、workspace不一致: 一覧を一切出さずexit `1`

部分的に正しそうな一覧を出さない。

## 7. `sbxm status <project>`

### 7.1 性質

`status`は指定scopeをread-onlyで診断するcommandである。global scopeの`sbxm status --global`はPhase 1仕様を正本とし、本文書ではproject scopeを実装する。

`sbxm status <project>`は1案件の構築状態、作業可能性、credential隔離をread-onlyで診断する。repair、起動、停止、file更新を行わない。Sandboxがstoppedで、検査が暗黙に起動する場合は実行せず`not-observed-stopped`とする。runningなど本来観測可能な状態でread-only検査commandが失敗した場合は、値を推測せずcommand失敗として扱う。

診断は現在のmetadata、filesystem、Git、`sbx`の状態だけに基づき、作成元やsbxm独自のmarkerを検査しない。同じvalidation規則をmutation commandも使用し、手作業または別toolで作成されたvalidな状態を同じ結果として扱う。

projectを省略した案件選択promptは設けない。`--global`とprojectの同時指定、またはどちらも指定しない場合はexit code `1`とする。global環境の問題でproject検査を継続できない場合は、観測不能な項目と原因を表示し、`sbxm status --global`を案内する。

### 7.2 検査順と項目

取得できた項目は、後続検査失敗時にも表示する。

1. metadataと目標構成
2. project rootとhost clone
3. 現在のDockerfile hashとmetadataに記録した適用済みhash、および一致・変更済み
4. image labelとTemplate archive
5. Sandbox name、workspace、state
6. GitHub secretの存在
7. Sandbox内bare repository
8. managed worktree
9. unmanaged worktree
10. SSH Agent露出

表示値:

```text
ready
missing
mismatch
changed
running
stopped
not-created
clean
dirty
attached
detached
not-exposed
exposed
not-applicable
not-observed-stopped
```

`unknown`は使用しない。

### 7.3 出力

project scopeは指定案件だけを診断し、正常結果を`PROJECT`と`WORKTREES`の2 sectionとしてstdoutへ表示する。global環境の検査結果を`GLOBAL` sectionとして混在させない。global環境の問題で観測できない項目がある場合は、7.1のとおり原因を診断し、別commandの`sbxm status --global`を案内する。

`PROJECT`は1案件の固定項目を縦に並べ、英語modeの列を`ITEM`と`VALUE`で固定する。`WORKTREES`は複数件を比較できるtableとし、英語modeの列を`PATH`、`KIND`、`MODE`、`STATE`で固定する。`KIND`はmetadataとの対応による`managed`または`unmanaged`、`MODE`はGit worktreeの形態を示す`attached`または`detached`、`STATE`は`clean`または`dirty`とする。

```text
PROJECT
ITEM                 VALUE
Project              owner/repository
Metadata             ready
Project root         ready
Host clone           ready
Dockerfile           changed
Image                ready
Template archive     ready
Sandbox              running
Workspace            ready
GitHub secret        ready
Bare repository      ready
SSH Agent            not-exposed

WORKTREES
PATH                    KIND        MODE        STATE
repository.tree-0       managed     attached   clean
repository.tree-1       unmanaged   detached   dirty
```

`changed`は現在のDockerfile hashがmetadataの適用済みhashと異なり、次の`rebuild`対象であることを示す。破損や観測失敗を示す`mismatch`とは区別する。

取得できた行は後続検査が失敗しても省略しない。Sandbox名、workspace、hash、観測値、外部commandの失敗、対処方法などの詳細は表の列を増やさず、安定したerror IDを持つ診断としてstderrへ出す。これにより一覧性のある正常出力と、原因を特定できる詳細なerror情報を分離する。

日本語modeではsection名、列名、項目名を翻訳し、状態値と`KIND`は翻訳しない。正常出力末尾のenum凡例は方向性文書の言語契約に従う。列間の空白幅は実装時のsnapshotで固定し、公開する英語modeの列構成と並び順は変更しない。

### 7.4 `not-applicable`

Sandboxが存在しない場合だけ、Sandbox内部でしか検査できない次を`not-applicable`とする。

- secret injectionのSandbox対応
- bare repository
- managed/unmanaged worktree
- SSH Agent露出

Docker image、archive、host cloneは引き続き検査する。

Sandboxがstoppedの場合、read-only `sbx exec`が暗黙に起動する可能性があるため実行しない。内部項目は`not-observed-stopped`とし、「停止状態を変えないため検査していない」という説明を付ける。これは観測失敗ではなく意図的な非観測なので、ほかに問題がなければexit code `0`とする。状態値を`unknown`へ丸めない。

### 7.5 Worktree検査

running時にSandbox内で次のporcelain出力を取得する。

```text
git --git-dir <bare-git-dir> worktree list --porcelain -z
git -C <worktree> status --porcelain=v2 -z --untracked-files=all
git -C <worktree> rev-parse HEAD
git -C <worktree> symbolic-ref --quiet --short HEAD
```

- `worktree list`のbare entryはworktree数へ含めない
- metadataのrelative pathと完全一致するものをmanagedとする
- その他をunmanagedとする
- managed entryがGitに存在しなければ`mismatch`
- pathはbare root配下へstandardizeできること。逸脱pathはsecurity error
- submodule状態も`git status`がdirtyと返す場合はdirty

### 7.6 SSH Agent検査

running時:

```text
env lookup for SSH_AUTH_SOCK
ssh-add -L
```

- `SSH_AUTH_SOCK`未設定かつ`ssh-add -L`がagent接続不能: `not-exposed`
- socket設定または公開鍵が1件以上: `exposed`、exit `1`
- command不在、timeout、判定不能: exit `1`

公開鍵本文は出力しない。

### 7.7 Exit

- 全検査成功かつsecurity issueなし: `0`
- 1件以上のerror: `1`

複数種類のerrorがあってもexit codeは`1`とし、構成不一致、外部観測失敗、SSH Agent露出をそれぞれのerror IDと診断で表示する。

## 8. 自動test

- 各commandの引数あり・なし・非TTY・cancel
- state mappingの全fixtureと未知state
- `open`のnot-created拒否、stopped起動、running再利用
- daemon再起動、active session、session不在を証明不能
- `stop`の事前全件validation、部分失敗report
- `ls`のmanaged/unmanaged、0件、failure時一覧非出力
- project `status`の必須対象、検査順、出力snapshot、部分結果、not-applicable、global診断の案内
- porcelain `-z` parser
- managed/unmanaged、missing managed、path逸脱
- dirty/untracked/submodule
- SSH Agent not-exposed、exposed、判定不能
- localeによらないenumと並び順

## 9. 実機受入条件

- 各`open`でactive session不在を確認し、daemonをSSH Agentなしで安全に再起動できる
- active sessionがある場合、またはsession不在を証明できない場合にdaemonを変更しない
- stopped/runningから同じ操作でSSH接続できる
- not-createdへの`open`が`add`再実行を正確に案内する
- 複数Sandboxを対象限定で停止できる
- `ls`がrunning、stopped、not-created、unmanagedを正しく分離する
- `status`がbare、managed、unmanaged、dirty、SSH Agentを診断する
- 外部状態取得失敗時に推測した値を出さない
