# Phase 4 実装仕様: `rebuild`、`destroy`とE2E検証

## 1. 目的

`sbxm rebuild`は、利用者が編集したDockerfileを新しいimageとTemplateへbuildし、保存されていない作業や再現不能なworktreeがないことを確認してから、既存Sandboxを同じ目標構成で再作成する。安全検査を省略するoptionは設けない。

`sbxm destroy`は、対象Sandboxを一意に特定したうえで、通常modeでは保存されていない作業を失わないことを確認して、force modeではデータ保護検査を省略して、Sandboxとsbxmの管理情報を破棄する。host cloneと利用者管理の成果物は保持し、案件を`unmanaged`状態へ戻す。

```text
sbxm rebuild <owner>/<repository>
sbxm destroy [<owner>/<repository>]
sbxm destroy --force <owner>/<repository>
sbxm destroy -f <owner>/<repository>
```

通常modeではdirty、untracked、検査不能なworktreeが1つでもあれば削除しない。`-f`は`--force`の短縮形とする。

`--force`は、対象特定後のactive session、worktree、保存状態の検査と対話確認を省略する。TTYかどうかにかかわらずproject引数の完全指定を必須とする。

`rebuild`はproject引数の完全指定を必須とし、対象選択promptと対話確認を行わない。安全性を証明できない場合は再構築せず、問題の解消方法を表示する。

## 2. 共通のデータ保護検査

running Sandboxを削除する通常modeの`rebuild`と`destroy`は、同じactive session、worktree、保存状態parserと判定規則を使用する。

- active sessionがないこと
- managed worktreeがmetadataと一致すること
- dirty、untracked、進行中Git操作がないこと
- attached HEADにupstreamがあり、unpushed commitがないこと
- detached HEADが`refs/remotes/origin/*`から到達可能であること
- unreadable、parse不能、path逸脱がないこと

`destroy`は上記を満たすunmanaged worktreeも削除可能とする。`rebuild`はunmanaged worktreeの配置を再現できないため、保存状態にかかわらず1件でも存在すれば拒否する。`rebuild`に`--force`は設けない。

## 3. `rebuild`

### 3.1 状態別動作

| 状態 | 動作 |
|---|---|
| `unmanaged` | exit `4`、`add`を案内 |
| `registered`、rebuild intentなし | 初回構築未完了として`add`を案内 |
| `registered`、rebuild intentあり | 新世代成果物とSandbox不在を検証し、再作成を継続 |
| `stopped` | 内部状態を観測するため`open`後の再実行を案内して拒否 |
| `running` | 共通データ保護検査後に再構築 |
| `inconsistent` | exit `4`、自動変更しない |

Dockerfile hashがmetadataの適用済みhashと同一で、rebuild intentがない場合は、変更がないことを表示して何も変更せずexit code `0`とする。

rebuild intentがある場合は通常の状態表よりintentの継続規則を優先する。Sandboxが不在なら作成工程から、同じtarget TemplateのSandboxが存在するなら構築済み工程をinspectして未完了箇所から継続する。旧TemplateのSandbox、identity不一致、対象を一意に証明できない状態では自動削除せずexit code `4`とする。

### 3.2 新世代成果物

imageとarchiveはDockerfile SHA-256 prefixを含む世代別の名前を使用する。

```text
image   = <sandbox-name>-template:<dockerfile-sha256-first-12-hex>
archive = .sbxm/.cache/template-<dockerfile-sha256-first-12-hex>.tar
```

世代名のprefixが同じ既存成果物を検出した場合はfull SHA-256 labelを比較し、一致しなければ衝突としてmutationしない。

1. 現在のDockerfileを検証してSHA-256を求める
2. project lockを取得し、stateとDockerfile hashを再確認する
3. runningなら共通データ保護検査を行う
4. 新imageをbuildし、labelとimage IDを検証する
5. 新archiveを世代別の一時pathからatomicに確定する
6. 新Templateをloadし、imageとの対応を検証する
7. 適用予定hash、旧適用済みhash、目標構成をrebuild intentとしてmetadataへatomic writeする

新世代のbuild、archive、Template検証が完了するまで既存Sandboxを停止・削除しない。これらの工程が失敗した場合、rebuild intentを作らず、既存Sandboxと適用済みhashを変更しない。

### 3.3 Sandbox切替

rebuild intentの記録後は次を行う。

1. active sessionがないことと共通データ保護条件を直前に再確認する
2. 対象Sandboxを通常modeの削除commandで削除する
3. `sbx ls --json`で不在を確認する
4. Phase 1の共通手順でdaemonを安全に再起動し、Phase 2と同じ中立Workspaceと新Templateで同名Sandboxを作成する
5. Git identity、protocol、宣言fileを配置する
6. bare repositoryをcloneし、metadataにあるmanaged worktreeだけを同じmode、start ref、indexで再作成する
7. Sandbox identity、worktree、credential隔離を検証する
8. metadataの適用済みDockerfile hashを新hashへ更新し、rebuild intentを削除する

利用者が編集したDockerfile、host clone、exports、global config、GitHub secretは保持する。旧image、旧archive、旧Templateの自動cleanupはMVP対象外とする。

Sandbox削除後に失敗した場合は、metadataとrebuild intentを保持し、exit code `5`で終了する。利用者は同じ`sbxm rebuild <owner>/<repository>`を再実行する。rebuild intentがある状態では`add`、`sync-files`、`open`、`stop`、通常の新規`rebuild`を開始せず、同じtarget hashの`rebuild`継続だけを許可する。Dockerfileがintent記録時から変わっていた場合は、内容を混在させずexit code `4`とする。

### 3.4 Confirmationとforce

`rebuild`というcommandとproject完全指定を再構築意思の表明とし、追加のtyped confirmationは要求しない。TTY、非TTYのどちらでも同じ安全検査を実行する。

- `--force`、`-f`はparserで受け付けない
- active session、unmanaged worktree、保存状態不合格、検査不能では常に拒否する
- stopped Sandboxを暗黙に起動しない
- 新世代成果物の準備前に既存Sandboxを変更しない

## 4. `destroy`の削除対象と保持対象

削除対象:

- Docker Sandboxesの対象Sandbox
- Sandbox内filesystem
- Sandbox内bare repositoryと全worktree
- Sandbox内package、設定、inner Docker Engine状態
- `.sbxm/project.toml`
- `.sbxm/project.lock`
- `.sbxm/.cache`とその内容

保持対象:

- host cloneとその全内容
- `.sbxm/Dockerfile`
- `.sbxm/exports`とその内容
- host Docker image
- loaded Template
- 中立Workspaceとownership marker
- Docker Sandboxes secret

host Docker image、loaded Template、中立Workspace、secretのcleanupはMVP対象外。Dockerfileは利用者が手修正するfile、`exports`は利用者が退避したfileの置き場であり、管理解除後も保持する。

## 5. `destroy`の状態別動作

| 状態 | 動作 |
|---|---|
| `unmanaged` | exit `4` |
| `registered` / `not-created` | Sandboxは削除済みとして、管理情報を破棄して`unmanaged` |
| `stopped` | 通常modeでは内部状態を観測できないため削除を拒否し、完全指定した`destroy --force`を案内 |
| `running` | 通常modeではsession終了を要求し、worktree検査後に削除 |
| `inconsistent` | exit `4`、自動削除しない |

`destroy`成功後はmetadataを削除するため、常に`unmanaged`となる。以後の再構築は`add`で新規登録する。

force modeでは、`registered`は管理情報を破棄し、`stopped`と`running`はデータ保護検査なしでSandboxと管理情報を削除する。`unmanaged`、`inconsistent`、対象を一意に特定できない状態はforce modeの対象にならない。

## 6. `destroy`の排他と事前確認

1. 対象を引数またはTTY上の単一選択promptで解決
2. project lockを取得
3. stateとSandbox identityを取得
4. Sandboxが存在する通常modeではactive sessionと全worktreeの保存状態を検査
5. 削除対象と保持対象を表示
6. 通常modeかつTTYでは明示確認
7. Sandboxが存在すれば削除
8. Sandboxの不在を検証
9. `.sbxm/.cache`を削除
10. metadataを削除して管理解除を確定
11. project lockを解放してlock fileを削除

削除開始前にproject lockを保持し、他の`add`、`sync-files`、`rebuild`、`open`、`stop`、`destroy`を排除する。

対象特定ではmetadata、canonical project ID、導出したSandbox名、workspace、ownershipを検証する。対象を一意に特定できない場合は通常・forceのどちらでも削除しない。

## 7. Active session

通常modeでは、Codex、Claude Code、SSH、editor、development serverなどのsession状態を`sbx` structured outputから判定する。

- active sessionを検出: 対象sessionを表示し、終了方法と`destroy --force`を案内してexit code `6`
- 対象versionがstructuredなsession検査を提供しない: session不在を証明できないことと`destroy --force`を案内してexit code `6`
- 対象versionが提供するsession検査commandの実行失敗、timeout、parse失敗: 外部状態を観測できないためexit code `5`
- inactiveを確認: worktreeと保存状態の検査へ進む

通常modeは実行回数を記録せず、「初回」と「再実行」を区別しない。session検査を提供しないversionでは、通常modeを再実行しても毎回exit code `6`とする。

force modeではactive sessionを検査せず、session終了を要求しない。

## 8. Worktree列挙

Phase 3と同じporcelain parserを再利用する。

```text
git --git-dir <bare-git-dir> worktree list --porcelain -z
```

- bare entryを除く全worktreeを対象とする
- metadata一致をmanaged、それ以外をunmanagedと表示する
- metadataにあるがGit一覧にないmanaged pathもmismatchとして削除を拒否
- bare root外のworktree pathはsecurity errorとして削除を拒否
- worktreeが0件、Git command失敗、parse不能も削除を拒否

## 9. 保存状態

各worktreeで次を取得する。

```text
git -C <path> status --porcelain=v2 -z --untracked-files=all
git -C <path> rev-parse HEAD
git -C <path> symbolic-ref --quiet --short HEAD
git -C <path> log -1 --format=%H%x00%aI%x00%s
```

削除可能なのは全worktreeが次を満たす場合だけ。

- porcelain statusが空
- HEADを取得可能
- merge、rebase、cherry-pick、revert、bisectの進行中状態がない
- unreadable fileまたはpermission errorがない

upstream未設定、unpushed commitはdirtyではないが、消失可能性があるため別途検査する。

attached branch:

- upstreamがあり、`git rev-list --count <upstream>..HEAD`が0なら`pushed`
- upstreamなし、またはahead > 0なら削除拒否

detached HEAD:

- HEADが`refs/remotes/origin/*`のいずれかから到達可能なら`reachable`
- 到達不能なら削除拒否

unmanaged worktreeにも同じ規則を適用する。通常modeで削除するには、必要なcommitをpushするか、fileを`.sbxm/exports`へ取り出してworktreeをcleanにしてから再実行する。

force modeでは本sectionのworktree列挙と保存状態検査を行わない。

## 10. 停止中Sandbox

通常modeでは停止中Sandboxを起動せず、内部のworktreeと保存状態を観測不能としてexit code `6`で削除を拒否する。完全指定した次のcommandを案内する。

```text
sbxm destroy --force <owner>/<repository>
```

## 11. 削除確認

通常modeでは全データ保護検査に合格した後、次を表示する。

- canonical project ID
- Sandbox名とstate
- managed/unmanagedの分類
- 各path、branch/detached、HEAD、remote到達状態
- 削除対象
- 保持対象
- 再登録command

force modeではworktreeと保存状態を検査しないため、managed/unmanagedの実体分類、各path、branch、HEAD、remote到達状態を表示しない。次を表示する。

- canonical project ID
- Sandbox名とstate
- データ保護検査とactive session検査を省略すること
- 削除対象
- 保持対象
- 再登録command

通常modeをTTYで実行した場合だけ、Sandbox名の完全入力を要求する。yes/noだけでは削除しない。

```text
削除を確認するため、Sandbox名を入力してください:
sbxm-owner-repository-0123456789ab
```

- 完全一致: 続行
- 空または不一致: 削除せずexit `10`
- Ctrl-C/Esc: exit `130`

projectを完全指定した非TTYの通常modeとforce modeでは対話確認を行わない。force modeでは、データ保護検査を省略して削除することをstderrへ明示する。

## 12. Sandbox削除

Sandboxが存在する通常modeではactive sessionがないことを直前に再確認する。force modeでは再確認しない。Sandboxが存在する場合だけ、対象versionのfixtureで固定した各modeのcommandを実行する。

```text
sbx rm <sandbox-name>
sbx rm --force <sandbox-name>
```

削除後、`sbx ls --json`を最大60秒pollし、nameが存在しないことを確認する。`registered`では削除commandを実行せず、一覧で不在を1回確認して管理情報のcleanupへ進む。

- command失敗: metadataやhost成果物を変更せずexit `5`
- timeout: exit `5`
- 不在確認成功: 管理情報のcleanupへ進む

Sandbox不在を確認した後、`.sbxm/.cache`を削除し、`project.toml`を削除する。metadata削除を管理解除のcommit pointとする。最後にproject lockを解放して`project.lock`を削除する。

cleanupに失敗した場合は残ったpathを表示してexit code `5`とする。metadata削除前の失敗では案件は引き続き管理対象であり、`destroy`を再実行できる。metadata削除後にlock fileだけが残った場合、案件は`unmanaged`として扱い、残存lock fileのcleanup失敗をwarningとして表示してexit code `0`とする。

## 13. 再登録command

実行前にmetadataから元の目標構成を表示用に保持し、成功後に次のcommandを案内する。

attached:

```text
sbxm add <owner>/<repository> --worktrees 1
```

detached:

```text
sbxm add <owner>/<repository> --worktrees <N> --detach <branch>
```

このcommandはmetadataを再利用せず、新規案件として登録する。保持されたDockerfileがあれば、その内容を新しいbuildへ採用する。

## 14. 自動test

- `rebuild`のproject完全指定と`--force`拒否
- Dockerfile変更なしのno-op
- 新image build、世代別archive、Template検証
- build、archive、Template失敗時に既存Sandboxを維持すること
- active session、dirty、untracked、unpushed、検査不能による`rebuild`拒否
- unmanaged worktreeによる`rebuild`拒否
- stoppedでの`rebuild`拒否と`open`案内
- rebuild intentのatomic write、Sandbox削除前後の各中断点、同じ`rebuild`による継続
- rebuild中のDockerfile再変更、旧Template Sandbox、identity不一致の拒否
- managed worktree、宣言file、Git identityの再構築
- 適用済みhash更新とrebuild intent削除
- not-createdからの管理情報破棄
- TTY/非TTYの対象指定共通規則
- managed/unmanaged全件列挙
- dirty、untracked、operation in progress
- attachedのunpushed、upstreamなし、pushed
- detached HEADのremote到達・未到達
- path逸脱、metadata missing、Git parse failure
- stoppedの通常mode拒否とforce mode削除
- active session、session検査APIなし、検査command失敗
- typed confirmation一致、不一致、cancel、非TTYでの省略
- `-f`と`--force`、project完全指定、データ保護検査の全省略
- force modeでも対象を一意に特定できなければ拒否
- 通常modeとforce modeの表示項目
- delete command失敗、poll timeout、成功
- Sandbox削除失敗時にhost成果物とmetadataが変更されないこと
- cleanupの各失敗点、metadata削除のcommit point、lock file残存
- 成功後の`unmanaged` stateと再登録command

## 15. E2E実機検証

専用のprivate test repositoryだけを使用する。実案件を最初の検証対象にしない。

1. 新規Mac user相当の一時HOMEで`init`
2. 日本語・英語のlocale決定
3. `add`のsecret未登録中断
4. secret登録後の再開
5. attached 1 worktree
6. detached 3 managed worktree
7. Agent相当のunmanaged worktree追加
8. host pathとuser homeの非露出
9. SSH AgentとDocker socketの非露出
10. Codex、Claude Code、GitHub疎通
11. `open`のstopped/running
12. 案件切替時のactive session拒否とdaemon安全再起動
13. `stop`の複数対象とno-op
14. `ls`の3 stateとunmanaged Sandbox
15. `status`のmanaged/unmanaged、dirty、security診断
16. `sync-files`による宣言file再配置と他成果物の不変
17. Dockerfile変更なしの`rebuild` no-op
18. 新世代build失敗時の既存Sandbox維持
19. active session、dirty、unpushedによる`rebuild`拒否
20. unmanaged worktreeによる`rebuild`拒否
21. stopped Sandboxの`rebuild`拒否
22. cleanなmanaged worktreeだけを持つSandboxの`rebuild`
23. Sandbox削除直後に中断した`rebuild`の再実行
24. 新Dockerfile hash、managed worktree、file、Git identityの適用確認
25. dirty managedによる`destroy`拒否
26. dirty unmanagedによる`destroy`拒否
27. unpushed commitによる`destroy`拒否
28. cleanかつremote到達済みでtyped confirmation後の`destroy`
29. dirty、unpushed、active sessionを持つrunning Sandboxの`destroy --force`
30. stopped Sandboxの`destroy --force`
31. 非TTYかつ完全指定した通常`destroy`と`destroy --force`
32. host clone、Dockerfile、exports、image、Template、workspace、secretの保持
33. metadata、project lock、cacheの削除
34. `open`がunmanagedを拒否
35. 新しい`add`による再登録
36. 保持したDockerfileを使う初回build

各caseは実行command、期待exit code、期待stdout/stderr、事後状態をREADMEの手動検証sectionへ記録する。token、path内のMac user名、公開鍵は記録前にredactする。

## 16. Phase 4受入条件

- `rebuild`はproject完全指定を必須とし、force optionを持たない
- 新世代image、archive、Templateの検証前に既存Sandboxを変更しない
- active session、保存状態不合格、検査不能、unmanaged worktreeがあれば`rebuild`できない
- `rebuild`は停止中Sandboxを暗黙に起動しない
- `rebuild`成功後にmanaged worktreeと宣言設定を復元し、適用済みDockerfile hashを更新する
- Sandbox切替中に失敗してもrebuild intentを保持し、同じ`rebuild`で継続できる
- 通常modeではdirty、untracked、進行中Git操作、unpushed commit、到達不能detached HEADを持つSandboxを削除できない
- managedとunmanagedを同じ安全基準で検査する
- 通常modeは停止中Sandboxを起動せず削除を拒否する
- TTYの通常modeはtyped confirmationなしに削除できない
- projectを完全指定した非TTYの通常modeはデータ保護検査後に対話なしで削除できる
- force modeはproject完全指定を必須とし、データ保護検査と対話確認を省略してrunning/stoppedを削除できる
- force modeでも対象を一意に特定できない場合は削除できない
- Sandbox削除失敗時にhost成果物とmetadataを変更しない
- 成功後はmetadata、project lock、cacheを削除してunmanagedとなり、Dockerfile、exports、host cloneを保持する
- 新しい目標構成を指定した`add`で再登録できる
- E2E 36項目が対象exact versionで完了している
