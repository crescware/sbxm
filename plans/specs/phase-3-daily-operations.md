# Phase 3 実装仕様: `open`、`stop`、`ls`、`status`

## 1. 目的

Phase 3は、登録済み案件の日常的な起動、接続、停止、一覧、project scopeのread-only診断を実装する。Docker Sandboxesのruntime状態は複製せず、各command実行時にstructured outputから取得する。

## 2. 本Phaseで追加する共通基盤

次はPhase 3が最初の呼び出し側となるため、本Phaseで実装する。実装は利用するworkflowと同じPRへ入れる。

- 外部command runnerの`inherit`
  - `open`がterminalをSSHへ引き渡す
  - 既存のterminal動作をそのまま保つ
- timeout classのinteractive
  - 対話中の接続へtimeoutを課さない
- `sbx ls`のstructured output parser
  - `ls`が全案件のstateを読む
  - stateの3値正規化は本Phaseの規則に従う
- `status --global`への行の追加
  - Remote SSH対応状況
  - 必要なsetup commandは実機で確認したものだけを表示する

## 3. 共通規則

- project指定とTTY規則は方向性文書に従う
- mutation commandはproject lockを取得する
- 対象解決と全validationをmutation前に完了する
- `sbx` stateはstructured outputのparserで扱う
- 1案件を対象とするcommandは、その案件のSandbox名との完全一致だけで状態を決める。無関係な案件の破損で、対象案件の状態が読めなくならないようにする
- nameは完全一致とし、substring、prefix、表示textの`grep`を使わない
- 未対応stateまたは重複nameはraw valueを示してexit code `1`
- rebuild intentが存在する案件では`open`と`stop`を実行せず、同じtargetの`sbxm rebuild <owner>/<repository>`再実行を案内してexit code `1`

Sandboxのstart・stopなど人間向け進捗を出すmutationはPhase 2で追加した`passthrough`を使用する。state判定に使うstructured outputは`capture`する。sbxm独自のprogress表示は追加しない。

### 3.1 本実装が前提としている外部commandと出力

Phase 2仕様の同名の節に加えて、本Phaseは次を前提とする。この一覧は対象Mac上での確認対象であり、実出力が異なる場合は実装とこの節を同時に直す。

| 用途 | command | 読む値 |
|---|---|---|
| Sandboxの起動 | `sbx exec <name> -- /bin/true` | exit statusのみ |
| Sandboxの停止 | `sbx stop <name>` | exit statusのみ |
| SSH接続 | `ssh <name>.sbx` | exit statusのみ。stdin、stdout、stderrを継承する |
| Remote SSH設定の確認 | `ssh -G <name>.sbx` | `proxycommand`行の有無 |
| bare repositoryの確認 | `git --git-dir <dir> rev-parse --is-bare-repository` | `true`、および`128` |
| SSH Agent socketの確認 | `printenv SSH_AUTH_SOCK` | `0`と出力、および`1` |
| SSH Agent接続の確認 | `ssh-add -L` | `0`、`1`、`2`の別。公開鍵本文は読まない |

起動と停止の完了は、いずれも`sbx ls --json`のstateを読み直して判定する。commandの戻り値だけでは判定しない。

Sandbox内のread-only検査は、内側のcommandが返したexit statusと、実行そのものが成立しなかったことを区別する。`125`から`127`、およびsignalによる終了はexec側の失敗として扱い、内側のcommandが答えた結果として読まない。上表に挙げた以外のexit statusは判定不能とし、`missing`や`not-exposed`のような肯定的な値へ丸めない。

## 4. Sandbox stateの正規化

`sbx`が返すraw stateを次へ写像する。

| sbxm state | 意味 |
|---|---|
| `not-created` | metadataはあるが対応Sandboxがない |
| `running` | Sandboxが存在し起動中 |
| `stopped` | Sandboxが存在し停止中 |

raw stateを3値へ安全に写像できない場合は、対象nameとraw stateを示してerrorにする。`unknown`へ丸めない。

metadataやworkspaceとの対応が矛盾する場合、projectの管理状態は`inconsistent`となり、上記stateを正常結果として返さない。

対応の判定に使うTemplateの世代は、方向性文書 §7.2に従う。rebuild intentがあるあいだはtarget世代とprevious世代の両方を正本とし、そのどちらから作られたSandboxも同じ案件のものとして扱う。片方だけを期待して、切替の途中で中断した案件を別案件のSandboxとして扱わない。

## 5. `sbxm open [project]`

### 5.1 状態別動作

| 状態 | 動作 |
|---|---|
| `unmanaged` | exit `1`、`add`を案内 |
| `registered` / `not-created` | exit `1`、同じ目標構成の`add`再実行を案内 |
| `stopped` | daemonを安全に再起動し、Sandboxを非対話で起動してSSH接続 |
| `running` | daemonを安全に再起動してSSH接続 |
| `inconsistent` | exit `1`、`status`を案内 |

`open`はSandboxを新規作成しない。

### 5.2 処理順

1. 対象を引数または単一選択promptで解決
2. project lockを取得
3. Docker Engineへ接続確認
4. `sbx ls --json`相当を1回実行
5. Sandbox identity、workspace、stateを検証
7. stoppedなら実機で確認した非対話commandで起動
8. runningになるまで2秒間隔、最大60秒poll
9. managed worktree一覧をmetadataとGitから検証
10. 接続先とworktree一覧をstderrへ表示
11. `ssh <sandbox-name>.sbx`へterminalを引き渡す

対象の解決はSandboxの状態を読む前に終える。引数で完全指定された場合は導出したpathのmetadataだけを読み、案件選択promptを出す場合だけmetadata探索で候補を作る。lock取得前に読んだmetadataは判定に使わず、lock取得後に読み直した内容でrebuild intentとstateを判定する。

7の起動要否は、daemon再起動の前ではなく再起動後に読み直したstateで決める。再起動によってSandboxが停止した場合も、同じ工程で起動する。

project lockは9までのmutationを覆う区間で保持し、terminalを引き渡す前に解放する。SSH session自体はsbxmのmutationではなく、接続しているあいだ同じ案件の`stop`を待たせない。

引数なしのTTY実行で管理案件が0件の場合は、方向性文書の共通規則に従い、promptを表示せず`no-managed-projects`でexit code `1`とする。

手動手順で使用していた起動commandの候補は次であり、exact formは実装PRで実機に対して確定する。

```text
sbx exec <sandbox-name> /bin/true
```

SSH childにはstdin、stdout、stderrを継承する。SSHのexit statusが0なら`sbxm`も0、非ゼロは理由を推測せず外部command失敗としてexit code `1`に写像し、原値を表示する。sbxm自身がCtrl-Cを受けた場合は共通契約どおりexit code `130`とする。

通常開始directoryはDockerfileにより`/home/agent/work`とする。MVPではSSH commandへ自動`cd`を組み込まない。

### 5.3 Credential isolation

`open`はdaemonを操作しない。Phase 2と同じく、hostのSSH AgentがSandboxへ届かないことを、daemonの起動条件からではなくSandboxの中から確認する。

active sessionの検査は行わない。対象versionの`sbx ls --json`はsession数を示さないためである。

## 6. `sbxm stop [project...]`

### 6.1 対象

- 引数あり: 重複を除いた全projectをcanonical ID昇順に処理
- 引数なし: 全管理案件から1件以上を複数選択
- 未選択のまま確定する操作は受け付けず、EscまたはCtrl-Cでexit `130`
- 管理案件が0件の場合はpromptを表示せず、方向性文書の`no-managed-projects`でexit `1`

### 6.2 Validationとatomicity

1. 全対象metadataを解決
2. 1回のSandbox一覧取得で全stateを解決
3. `inconsistent`または未対応stateが1件でもあれば、何も停止せずerror
4. 全project lockをcanonical ID昇順に取得
5. stateを再取得してpreconditionを再確認
6. runningだけを停止

完全なtransaction rollbackは行わない。途中で外部commandが失敗した場合は後続対象を停止せず、対象ごとの`stopped`、`unchanged`、`failed`を表示してexit code `1`。`unchanged`は「この実行では停止していない」ことを示し、既に停止していた対象、Sandboxが無い対象、先行する失敗のあとそのままにした対象を含む。

5のpreconditionにはrebuild intentを含む。選択とlock取得のあいだにmetadataが変わり得るため、lock取得後に読み直したmetadataで判定し直す。

### 6.3 状態別動作

- `running`: `sbx stop <sandbox-name>`
- `stopped`: no-op成功
- `not-created`: no-op成功
- `inconsistent`: mutation前error

停止後は最大60秒pollし、stoppedを確認する。内部filesystem、Git、package、Docker imageを削除しない。

## 7. `sbxm ls`

### 7.1 処理

1. configからbase pathを読む
2. 全metadataを探索・検証する
3. `sbx ls --json`相当を1回実行する
4. metadataとSandboxをname完全一致で突き合わせる
5. 管理案件と未管理Sandboxを別tableで表示する

0件でもheaderを表示してexit `0`とする。

### 7.2 出力

```text
PROJECT          SANDBOX                         STATE
owner/foo        sbxm-owner-foo-0123456789ab     running
owner/bar        sbxm-owner-bar-abcdef012345     stopped
owner/baz        sbxm-owner-baz-fedcba987654     not-created
```

並び順:

- managed projects: canonical ID byte昇順
- unmanaged Sandboxes: Sandbox name byte昇順

未管理Sandboxは`UNMANAGED SANDBOXES`へname、raw state、workspaceを表示する。取り込みや削除は行わない。raw stateはsbxmの管理状態ではないため、3値へ写像せずruntimeが示したまま表示し、enum凡例にも含めない。

### 7.3 Failure

- `sbx ls`失敗: 一覧を一切出さずexit `1`
- metadataが1件でも不正: 一覧を一切出さずexit `1`
- 未対応raw state: 一覧を一切出さずexit `1`
- 同名Sandbox複数、workspace不一致: 一覧を一切出さずexit `1`

部分的に正しそうな一覧を出さない。

## 8. `sbxm status <project>`

### 8.1 性質

`status`は指定scopeをread-onlyで診断するcommandである。global scopeの`sbxm status --global`は各Phaseが自身の検査を追加し、本文書ではproject scopeとRemote SSHの行を実装する。

`sbxm status <project>`は1案件の構築状態、作業可能性、credential隔離をread-onlyで診断する。repair、起動、停止、file更新を行わない。Sandboxがstoppedで、検査が暗黙に起動する場合は実行せず`not-observed-stopped`とする。runningなど本来観測可能な状態でread-only検査commandが失敗した場合は、値を推測せずcommand失敗として扱う。

診断は現在のmetadata、filesystem、Git、`sbx`の状態だけに基づき、作成元やsbxm独自のmarkerを検査しない。同じvalidation規則をmutation commandも使用し、手作業または別toolで作成されたvalidな状態を同じ結果として扱う。

projectを省略した案件選択promptは設けない。`--global`とprojectの同時指定、またはどちらも指定しない場合はexit code `1`とする。global環境の問題でproject検査を継続できない場合は、観測不能な項目と原因を表示し、`sbxm status --global`を案内する。

### 8.2 検査順と項目

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

8と9はworktree一覧の観測結果を1項目として`PROJECT`へ示し、内訳を`WORKTREES` sectionへ並べる。

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

### 8.3 出力

project scopeは指定案件だけを診断し、正常結果を`PROJECT`と`WORKTREES`の2 sectionとしてstdoutへ表示する。global環境の検査結果を`GLOBAL` sectionとして混在させない。global環境の問題で観測できない項目がある場合は、8.1のとおり原因を診断し、別commandの`sbxm status --global`を案内する。

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
Worktrees            ready
SSH Agent            not-exposed

WORKTREES
PATH                    KIND        MODE        STATE
repository.tree-0       managed     attached   clean
repository.tree-1       unmanaged   detached   dirty
```

`changed`は現在のDockerfile hashがmetadataの適用済みhashと異なり、次の`rebuild`対象であることを示す。破損や観測失敗を示す`mismatch`とは区別する。

取得できた行は後続検査が失敗しても省略しない。Sandbox名、workspace、hash、観測値、外部commandの失敗、対処方法などの詳細は表の列を増やさず、安定したerror IDを持つ診断としてstderrへ出す。これにより一覧性のある正常出力と、原因を特定できる詳細なerror情報を分離する。

日本語modeではsection名、列名、項目名を翻訳し、状態値と`KIND`は翻訳しない。正常出力末尾のenum凡例は方向性文書の言語契約に従う。列間の空白幅は実装時のsnapshotで固定し、公開する英語modeの列構成と並び順は変更しない。

### 8.4 `not-applicable`

Sandboxが存在しない場合だけ、Sandbox内部でしか検査できない次を`not-applicable`とする。

- secret injectionのSandbox対応
- bare repository
- managed/unmanaged worktree
- SSH Agent露出

Docker image、archive、host cloneは引き続き検査する。

Sandboxがstoppedの場合、read-only `sbx exec`が暗黙に起動する可能性があるため実行しない。内部項目は`not-observed-stopped`とし、「停止状態を変えないため検査していない」という説明を付ける。これは観測失敗ではなく意図的な非観測なので、ほかに問題がなければexit code `0`とする。状態値を`unknown`へ丸めない。

Sandboxの状態そのものを観測できなかった場合は、Sandboxが存在しないことにせず、破損や観測失敗を示す`mismatch`とする。`not-applicable`はSandboxが無いことを確かめられた場合だけに使う。

Sandbox一覧やDocker Engineを読めないなど、global環境の問題で観測できない項目がある場合は、その原因とあわせて`sbxm status --global`を案内する。

### 8.5 Worktree検査

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

### 8.6 SSH Agent検査

running時:

```text
env lookup for SSH_AUTH_SOCK
ssh-add -L
```

- `SSH_AUTH_SOCK`未設定かつ`ssh-add -L`がagent接続不能: `not-exposed`
- socket設定、またはagentへ接続できた場合: `exposed`、exit `1`
- command不在、timeout、判定不能: `mismatch`としexit `1`

`ssh-add -L`は、鍵が1件もない場合の`1`とagentへ接続できない場合の`2`を区別する。鍵の有無にかかわらずagentへ接続できた時点で露出とみなす。判定できないexit statusを`not-exposed`へ丸めない。

公開鍵本文は出力しない。

### 8.7 Exit

- 全検査成功かつsecurity issueなし: `0`
- 1件以上のerror: `1`

複数種類のerrorがあってもexit codeは`1`とし、構成不一致、外部観測失敗、SSH Agent露出をそれぞれのerror IDと診断で表示する。

## 9. 自動test

- 各commandの引数あり・なし・非TTY・cancel
- `open`と`stop`の管理案件0件、`stop`の未選択確定拒否
- state mappingの全既知値と未知state
- `open`のnot-created拒否、stopped起動、running再利用
- rebuild intent中の`open`と`stop`拒否
- credential隔離の確認成功・確認不能・露出
- Sandbox start・stopのpassthrough、structured出力のcapture、SSHのinherit
- `stop`の事前全件validation、部分失敗report
- `ls`のmanaged/unmanaged、0件、failure時一覧非出力
- project `status`の必須対象、検査順、出力snapshot、部分結果、not-applicable、global診断の案内
- porcelain `-z` parser
- managed/unmanaged、missing managed、path逸脱
- dirty/untracked/submodule
- SSH Agent not-exposed、exposed、判定不能
- localeによらないenumと並び順

## 10. 実機受入条件

- 各`open`で、hostのSSH AgentがSandboxへ届かないことをSandboxの中から確認できる
- stopped/runningから同じ操作でSSH接続できる
- not-createdへの`open`が`add`再実行を正確に案内する
- 複数Sandboxを対象限定で停止できる
- `ls`がrunning、stopped、not-created、unmanagedを正しく分離する
- `status`がbare、managed、unmanaged、dirty、SSH Agentを診断する
- 外部状態取得失敗時に推測した値を出さない
- 呼び出し側のない型、policy、error ID、messageを追加していない
