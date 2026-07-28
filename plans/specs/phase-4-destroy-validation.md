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

`--force`は、対象特定後のworktreeと保存状態の検査、そして対話確認を省略する。TTYかどうかにかかわらずproject引数の完全指定を必須とする。

`rebuild`はproject引数の完全指定を必須とし、対象選択promptと対話確認を行わない。安全性を証明できない場合は再構築せず、問題の解消方法を表示する。

`rebuild`が再利用するbuild、save、clone、fetch、Template、Sandbox操作と、`destroy`のSandbox削除はPhase 2で追加した`passthrough`を使用し、外部toolの進捗を隠さない。安全検査と事後検証のstructured outputは`capture`する。sbxm独自のprogress表示は追加しない。

Phase 4ではE2E結果を記録する手動検証sectionをproject `README.md`へ追加し、実行command、期待結果、redaction規則を利用者が再実行できる形で残す。

## 2. 本Phaseで追加する共通基盤

次はPhase 4が最初の呼び出し側となるため、本Phaseで実装する。実装は利用するworkflowと同じPRへ入れる。

- Project metadataのrebuild intent
  - 適用予定のDockerfile hashをdurableに記録する
  - rebuild intent中はtarget hashとprevious hashを世代判定の正本とする
  - Sandbox再作成と検証の成功後にintentを削除する
- 利用者向けREADMEの手動検証section
  - Phase 4まで完了して初めて、利用者が通しで実行できる手順になる
  - 実行command、期待結果、redaction規則を記録する

## 3. 共通のデータ保護検査

running Sandboxを削除する通常modeの`rebuild`と`destroy`は、同じworktreeと保存状態のparserと判定規則を使用する。接続中のsessionは、対象versionが示さないため検査しない。

- managed worktreeがmetadataと一致すること
- dirty、untracked、進行中Git操作がないこと
- attached HEADにupstreamがあり、unpushed commitがないこと
- detached HEADが`refs/remotes/origin/*`から到達可能であること
- unreadable、parse不能、path逸脱がないこと

`destroy`は上記を満たすunmanaged worktreeも削除可能とする。`rebuild`はunmanaged worktreeの配置を再現できないため、保存状態にかかわらず1件でも存在すれば拒否する。`rebuild`に`--force`は設けない。

## 4. `rebuild`

### 4.1 状態別動作

| 状態 | 動作 |
|---|---|
| `unmanaged` | exit `1`、`add`を案内 |
| `registered`、rebuild intentなし | 初回構築未完了として`add`を案内 |
| `registered`、rebuild intentあり | 新世代成果物とSandbox不在を検証し、再作成を継続 |
| `stopped` | 内部状態を観測するため`open`後の再実行を案内して拒否 |
| `running` | 共通データ保護検査後に再構築 |
| `inconsistent` | exit `1`、自動変更しない |

Dockerfile hashがmetadataの適用済みhashと同一で、rebuild intentがない場合は、変更がないことを表示して何も変更せずexit code `0`とする。

rebuild intentがある場合は通常の状態表よりintentの継続規則を優先し、intentに固定したtarget世代だけを完成させる。現在のDockerfileを新しいtargetとして解釈せず、観測したSandboxを次のように扱う。

| Sandbox | 継続位置 |
|---|---|
| 不在 | target世代の成果物を検証し、Sandbox作成から継続 |
| `previous_dockerfile_sha256`世代 | identityを検証し、共通データ保護検査を再実行して旧Sandbox削除から継続 |
| `target_dockerfile_sha256`世代 | identityと構築済み工程を検証し、最初の未完了工程から継続 |
| target、previousのどちらでもない | 帰属不能として自動変更せずexit code `1` |

保存状態はSandboxの中からしか読めないため、対象Sandboxがstoppedの場合は非対話で起動してから検査する。`rebuild`はそのSandboxをこれから作り直すのであり、状態を読むためだけの起動を利用者へ求めない。保存状態不合格、検査不能、identity不一致では削除しない。

target世代のSandboxは、各工程の事後条件をinspectして成功済み工程をskipする。metadataの適用済みhash更新とintent削除は、Sandbox identity、managed worktree、宣言file、Git identity、credential隔離の全検証が成功した後だけ行う。

### 4.2 新世代成果物

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

### 4.3 Sandbox切替

rebuild intentの記録後は次を行う。

1. 共通データ保護条件を直前に再確認する
2. 対象Sandboxを通常modeの削除commandで削除する
3. `sbx ls --json`で不在を確認する
4. Phase 2のdaemon安全再起動手順でdaemonを再起動し、Phase 2と同じ中立Workspaceと新Templateで同名Sandboxを作成する
5. Git identity、protocol、宣言fileを配置する
6. bare repositoryをcloneし、metadataにあるmanaged worktreeだけを同じmode、start ref、indexで再作成する
7. Sandbox identity、worktree、credential隔離を検証する
8. metadataの適用済みDockerfile hashを新hashへ更新し、rebuild intentを削除する

利用者が編集したDockerfile、host clone、global config、GitHub secretは保持する。旧image、旧archive、旧Templateの自動cleanupはMVP対象外とする。

storageの可視化やcleanupはオーケストレーターの利便機能として将来扱い得るが、MVPでは世代間の参照とrebuild intentから安全な削除対象を判定する機能を実装しない。通常運用中は容量を理由に成果物を推測で削除せず、`destroy`時だけ5章で定義したproject cacheを削除する。

Sandbox削除後に失敗した場合は、metadataとrebuild intentを保持し、exit code `1`で終了する。利用者は同じ`sbxm rebuild <owner>/<repository>`を再実行する。rebuild intentがある状態では`add`、`sync-files`、`open`、`stop`、通常の新規`rebuild`を開始せず、同じtarget hashの`rebuild`継続だけを許可する。

intent記録後に現在のDockerfileが変わっていても、検証済みのtarget image、archive、Templateが揃っている場合は、それらを用いてintentの世代を完成させる。現在のDockerfileを上書きせず、成功後の適用済みhashはintentのtarget hashとし、現在のDockerfileに未適用の変更が残っていることと、もう一度`rebuild`を実行する案内を表示する。

target成果物が欠落または不正な場合は、現在のDockerfile hashがtarget hashと一致するときだけ成果物を再生成できる。hashが異なる場合は世代を混在させずexit code `1`とし、期待するtarget hash、欠落または不正な成果物、Dockerfileを期待内容へ復元して再実行する方法を表示する。復元できない場合は、保持対象と失われる対象を示したうえで、明示的な最終手段として`sbxm destroy --force <owner>/<repository>`を案内する。metadataのintentを手動編集または削除する案内は行わない。

### 4.4 Confirmationとforce

`rebuild`というcommandとproject完全指定を再構築意思の表明とし、追加のtyped confirmationは要求しない。TTY、非TTYのどちらでも同じ安全検査を実行する。

- `--force`、`-f`はparserで受け付けない
- unmanaged worktree、保存状態不合格、検査不能では常に拒否する
- 新規`rebuild`ではstopped Sandboxを暗黙に起動しない。intent中のprevious世代を復旧する場合だけ安全検査のため起動する
- 新世代成果物の準備前に既存Sandboxを変更しない

## 5. `destroy`の削除対象と保持対象

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
- host Docker image
- loaded Template
- 中立Workspace
- Docker Sandboxes secret

host Docker image、loaded Template、中立Workspace、secretのcleanupはMVP対象外。Dockerfileは利用者が手修正するfileであり、管理解除後も保持する。

## 6. `destroy`の状態別動作

| 状態 | 動作 |
|---|---|
| `unmanaged` | exit `1` |
| `registered` / `not-created` | Sandboxは削除済みとして、管理情報を破棄して`unmanaged` |
| `stopped` | 通常modeでは内部状態を観測できないため削除を拒否し、完全指定した`destroy --force`を案内 |
| `running` | 通常modeではsession終了を要求し、worktree検査後に削除 |
| `inconsistent` | exit `1`、自動削除しない |

`destroy`成功後はmetadataを削除するため、常に`unmanaged`となる。以後の再構築は`add`で新規登録する。

force modeでは、`registered`は管理情報を破棄し、`stopped`と`running`はデータ保護検査なしでSandboxと管理情報を削除する。`unmanaged`、`inconsistent`、対象を一意に特定できない状態はforce modeの対象にならない。

## 7. `destroy`の排他と事前確認

1. 対象を引数またはTTY上の単一選択promptで解決
2. project lockを取得
3. stateとSandbox identityを取得
4. Sandboxが存在する通常modeでは全worktreeの保存状態を検査
5. 削除対象と保持対象を表示
6. 通常modeかつTTYでは明示確認
7. Sandboxが存在すれば削除
8. Sandboxの不在を検証
9. `.sbxm/.cache`を削除
10. metadataを削除して管理解除を確定
11. 最後の保護対象mutationとしてproject lock fileを削除し、lockを解放

削除開始前にproject lockを保持し、他の`add`、`sync-files`、`rebuild`、`open`、`stop`、`destroy`を排除する。

対象特定ではmetadata、canonical project ID、導出したSandbox名、workspace、Template/image identityを共通validation規則で検証する。作成元やsbxm独自のmarkerは条件にしない。対象を一意に特定できない場合は通常・forceのどちらでも削除しない。

引数なしのTTY実行で管理案件が0件の場合は、方向性文書の共通規則に従い、promptを表示せず`no-managed-projects`でexit code `1`とする。

## 8. Active session

通常modeでは、Codex、Claude Code、SSH、editor、development serverなどのsession状態を`sbx` structured outputから判定する。

- 対象versionが提供するsession検査commandの実行失敗、timeout、parse失敗: 外部状態を観測できないためexit code `1`
- inactiveを確認: worktreeと保存状態の検査へ進む

通常modeは実行回数を記録せず、「初回」と「再実行」を区別しない。session検査を提供しないversionでは、通常modeを再実行しても毎回exit code `1`とする。

接続中のsessionは、通常modeでもforce modeでも検査しない。対象versionの`sbx ls --json`がsession数を示さないためである。`rebuild`と`destroy`が守るのは保存されていない作業であり、接続している端末ではない。

## 9. Worktree列挙

Phase 3と同じporcelain parserを再利用する。

```text
git --git-dir <bare-git-dir> worktree list --porcelain -z
```

- bare entryを除く全worktreeを対象とする
- metadata一致をmanaged、それ以外をunmanagedと表示する
- metadataにあるがGit一覧にないmanaged pathもmismatchとして削除を拒否
- bare root外のworktree pathはsecurity errorとして削除を拒否
- worktreeが0件、Git command失敗、parse不能も削除を拒否

## 10. 保存状態

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

unmanaged worktreeにも同じ規則を適用する。通常modeで削除するには、必要な変更をGit管理へ追加してcommit・pushし、不要なuntracked fileを削除してから再実行する。Git管理外fileのhostへの搬出はMVP対象外とする。

force modeでは本sectionのworktree列挙と保存状態検査を行わない。

## 11. 停止中Sandbox

通常modeでは停止中Sandboxを起動せず、内部のworktreeと保存状態を観測不能としてexit code `1`で削除を拒否する。完全指定した次のcommandを案内する。

```text
sbxm destroy --force <owner>/<repository>
```

## 12. 削除確認

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
- データ保護検査を省略すること
- 削除対象
- 保持対象
- 再登録command

通常modeをTTYで実行した場合だけ、Sandbox名の完全入力を要求する。yes/noだけでは削除しない。

```text
削除を確認するため、Sandbox名を入力してください:
sbxm-owner-repository-0123456789ab
```

- 完全一致: 続行
- 空または不一致: 削除せずexit `1`
- Ctrl-C/Esc: exit `130`

projectを完全指定した非TTYの通常modeとforce modeでは対話確認を行わない。force modeでは、データ保護検査を省略して削除することをstderrへ明示する。

## 13. Sandbox削除

Sandboxが存在する場合だけ、実機で確認したcommandを実行する。

```text
sbx rm --force <sandbox-name>
```

`--force`は常に付ける。これが省くのは`sbx`の確認promptだけであり、削除してよいかはsbxmが先に判定している。`destroy`は自前の確認も済ませている。非対話で走る実行ではpromptに答える手段がなく、`sbx`は`stdin is not a terminal`で失敗する。sbxmの`--force`はsbxm自身のデータ保護検査を省くことを指し、別物である。

削除後、`sbx ls --json`を最大60秒pollし、nameが存在しないことを確認する。`registered`では削除commandを実行せず、一覧で不在を1回確認して管理情報のcleanupへ進む。

- command失敗: metadataやhost成果物を変更せずexit `1`
- timeout: exit `1`
- 不在確認成功: 管理情報のcleanupへ進む

Sandbox不在を確認した後、`.sbxm/.cache`を削除し、`project.toml`を削除する。metadata削除を管理解除のcommit pointとする。最後の保護対象mutationとして、project lockを保持したまま`project.lock`を削除し、その後lockを解放する。lock file削除後は、表示を除いてproject状態を変更しない。

cleanupに失敗した場合は残ったpathを表示してexit code `1`とする。metadata削除前の失敗では案件は引き続き管理対象であり、`destroy`を再実行できる。metadata削除後にlock fileだけが残った場合、案件は`unmanaged`として扱い、残存lock fileのcleanup失敗をwarningとして表示してexit code `0`とする。

削除前から古いlock fileを開いて待機していたprocessは、lock取得後に現在のpathとのidentity不一致を検出し、Phase 2の共通取得手順に従って新しいlock fileで取得をやり直す。これにより、`destroy`後の`add`が新規作成したlock fileと、削除済みinodeを待っていたprocessが同時に保護区間へ入ることを防ぐ。lock fileはprojectの管理状態そのものではないが、`destroy`ではsbxm管理物を残さないため削除する。

## 14. 再登録command

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

## 15. 自動test

- `rebuild`のproject完全指定と`--force`拒否
- Dockerfile変更なしのno-op
- 新image build、世代別archive、Template検証
- build、archive、Template失敗時に既存Sandboxを維持すること
- rebuildとdestroyの外部進捗passthrough、structured出力のcapture
- dirty、untracked、unpushed、検査不能による`rebuild`拒否
- unmanaged worktreeによる`rebuild`拒否
- stoppedでの`rebuild`拒否と`open`案内
- rebuild intentのatomic write、Sandbox削除前後の各中断点、同じ`rebuild`による継続
- intent中のprevious世代からの削除再開、target世代からの構築継続、その他世代とidentity不一致の拒否
- intent中のDockerfile再変更を保持した継続、target成果物欠落時の再生成条件と復旧案内
- managed worktree、宣言file、Git identityの再構築
- 適用済みhash更新とrebuild intent削除
- not-createdからの管理情報破棄
- TTY/非TTYの対象指定共通規則
- 通常modeの管理案件0件でpromptを表示せず`no-managed-projects`
- managed/unmanaged全件列挙
- dirty、untracked、operation in progress
- attachedのunpushed、upstreamなし、pushed
- detached HEADのremote到達・未到達
- path逸脱、metadata missing、Git parse failure
- stoppedの通常mode拒否とforce mode削除
- typed confirmation一致、不一致、cancel、非TTYでの省略
- `-f`と`--force`、project完全指定、データ保護検査の全省略
- force modeでも対象を一意に特定できなければ拒否
- 通常modeとforce modeの表示項目
- delete command失敗、poll timeout、成功
- Sandbox削除失敗時にhost成果物とmetadataが変更されないこと
- cleanupの各失敗点、metadata削除のcommit point、lock file残存
- lock file削除前からの待機者がidentity不一致を検出し、新しいlock fileへretryすること
- 成功後の`unmanaged` stateと再登録command

## 16. E2E実機検証

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
13. `stop`の複数対象とno-op
14. `ls`の3 stateとunmanaged Sandbox
15. `status`のmanaged/unmanaged、dirty、security診断
16. `sync-files`による宣言file再配置と他成果物の不変
17. Dockerfile変更なしの`rebuild` no-op
18. 新世代build失敗時の既存Sandbox維持
19. dirty、unpushedによる`rebuild`拒否
20. unmanaged worktreeによる`rebuild`拒否
21. stopped Sandboxの`rebuild`拒否
22. cleanなmanaged worktreeだけを持つSandboxの`rebuild`
23. Sandbox削除直後に中断した`rebuild`の再実行
24. 新Dockerfile hash、managed worktree、file、Git identityの適用確認
25. dirty managedによる`destroy`拒否
26. dirty unmanagedによる`destroy`拒否
27. unpushed commitによる`destroy`拒否
28. cleanかつremote到達済みでtyped confirmation後の`destroy`
29. dirty、unpushedを持つrunning Sandboxの`destroy --force`
30. stopped Sandboxの`destroy --force`
31. 非TTYかつ完全指定した通常`destroy`と`destroy --force`
32. host clone、Dockerfile、image、Template、workspace、secretの保持
33. metadata、project lock、cacheの削除
34. `open`がunmanagedを拒否
35. 新しい`add`による再登録
36. 保持したDockerfileを使う初回build

各caseは実行command、期待exit code、期待stdout/stderr、事後状態をREADMEの手動検証sectionへ記録する。token、path内のMac user名、公開鍵は記録前にredactする。

## 17. Phase 4受入条件

- `rebuild`はproject完全指定を必須とし、force optionを持たない
- 新世代image、archive、Templateの検証前に既存Sandboxを変更しない
- 保存状態不合格、検査不能、unmanaged worktreeがあれば`rebuild`できない
- 新規`rebuild`は停止中Sandboxを暗黙に起動せず、intent中のprevious世代を復旧する場合だけ安全検査のため起動する
- `rebuild`成功後にmanaged worktreeと宣言設定を復元し、適用済みDockerfile hashを更新する
- Sandbox切替中に失敗してもrebuild intentを保持し、Sandbox不在、previous世代、target世代の各中断状態から同じ`rebuild`で継続できる
- intent記録後のDockerfile編集を上書きせず、固定済みtarget成果物が健全ならintent世代を完成させて未適用変更を案内できる
- 通常modeではdirty、untracked、進行中Git操作、unpushed commit、到達不能detached HEADを持つSandboxを削除できない
- managedとunmanagedを同じ安全基準で検査する
- 通常modeは停止中Sandboxを起動せず削除を拒否する
- TTYの通常modeはtyped confirmationなしに削除できない
- projectを完全指定した非TTYの通常modeはデータ保護検査後に対話なしで削除できる
- force modeはproject完全指定を必須とし、データ保護検査と対話確認を省略してrunning/stoppedを削除できる
- force modeでも対象を一意に特定できない場合は削除できない
- Sandbox削除失敗時にhost成果物とmetadataを変更しない
- 呼び出し側のない型、policy、error ID、messageを追加していない
- 成功後はmetadata、project lock、cacheを削除してunmanagedとなり、Dockerfileとhost cloneを保持する
- 新しい目標構成を指定した`add`で再登録できる
- E2E 36項目が対象exact versionで完了している
