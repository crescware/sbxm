# Phase 4 実装仕様: `destroy`とE2E検証

## 1. 目的

`sbxm destroy`は、対象Sandboxを一意に特定したうえで、通常modeでは保存されていない作業を失わないことを確認して、force modeではデータ保護検査を省略して、Sandbox内部の状態だけを破棄する。host projectと再構築に必要な宣言・成果物は保持し、案件を`registered`状態へ戻す。

```text
sbxm destroy [<owner>/<repository>]
sbxm destroy --force <owner>/<repository>
sbxm destroy -f <owner>/<repository>
```

通常modeではdirty、untracked、検査不能なworktreeが1つでもあれば削除しない。`-f`は`--force`の短縮形とする。

`--force`は、対象特定後のactive session、worktree、保存状態の検査と対話確認を省略する。TTYかどうかにかかわらずproject引数の完全指定を必須とする。

## 2. 削除対象と保持対象

削除対象:

- Docker Sandboxesの対象Sandbox
- Sandbox内filesystem
- Sandbox内bare repositoryと全worktree
- Sandbox内package、設定、inner Docker Engine状態

保持対象:

- host cloneとその全内容
- `.sbx/sbxm.toml`
- `.sbx/Dockerfile`
- `.sbx/exports`
- `.sbx/.cache/template.tar`
- host Docker image
- loaded Template
- 中立Workspaceとownership marker
- Docker Sandboxes secret

保持対象の自動cleanupはMVP対象外。

## 3. 状態別動作

| 状態 | 動作 |
|---|---|
| `unmanaged` | exit `4` |
| `registered` / `not-created` | `already removed`を表示し、promptなしでexit `0` |
| `stopped` | 通常modeでは内部状態を観測できないため削除を拒否し、完全指定した`destroy --force`を案内 |
| `running` | 通常modeではsession終了を要求し、worktree検査後に削除 |
| `inconsistent` | exit `4`、自動削除しない |

`destroy`はmetadataを削除しないため、成功後は常に`registered`となる。

force modeでは、`stopped`と`running`のどちらもデータ保護検査なしで削除する。`unmanaged`、`inconsistent`、対象を一意に特定できない状態はforce modeの対象にならない。

## 4. 排他と事前確認

1. 対象を引数またはTTY上の単一選択promptで解決
2. project lockを取得
3. stateとSandbox identityを取得
4. 通常modeではactive sessionと全worktreeの保存状態を検査
5. 削除対象と保持対象を表示
6. 通常modeかつTTYでは明示確認
7. Sandboxを削除
8. 不在を検証

削除開始前にproject lockを保持し、他の`add`、`open`、`stop`、`destroy`を排除する。

対象特定ではmetadata、canonical project ID、導出したSandbox名、workspace、ownershipを検証する。対象を一意に特定できない場合は通常・forceのどちらでも削除しない。

## 5. Active session

通常modeでは、Codex、Claude Code、SSH、editor、development serverなどのsession状態を`sbx` structured outputから判定する。

- active sessionを検出: 対象sessionを表示し、終了方法と`destroy --force`を案内してexit code `6`
- 対象versionがstructuredなsession検査を提供しない: session不在を証明できないことと`destroy --force`を案内してexit code `6`
- 対象versionが提供するsession検査commandの実行失敗、timeout、parse失敗: 外部状態を観測できないためexit code `5`
- inactiveを確認: worktreeと保存状態の検査へ進む

通常modeは実行回数を記録せず、「初回」と「再実行」を区別しない。session検査を提供しないversionでは、通常modeを再実行しても毎回exit code `6`とする。

force modeではactive sessionを検査せず、session終了を要求しない。

## 6. Worktree列挙

Phase 3と同じporcelain parserを再利用する。

```text
git --git-dir <bare-git-dir> worktree list --porcelain -z
```

- bare entryを除く全worktreeを対象とする
- metadata一致をmanaged、それ以外をunmanagedと表示する
- metadataにあるがGit一覧にないmanaged pathもmismatchとして削除を拒否
- bare root外のworktree pathはsecurity errorとして削除を拒否
- worktreeが0件、Git command失敗、parse不能も削除を拒否

## 7. 保存状態

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

unmanaged worktreeにも同じ規則を適用する。通常modeで削除するには、必要なcommitをpushするか、fileを`.sbx/exports`へ取り出してworktreeをcleanにしてから再実行する。

force modeでは本sectionのworktree列挙と保存状態検査を行わない。

## 8. 停止中Sandbox

通常modeでは停止中Sandboxを起動せず、内部のworktreeと保存状態を観測不能としてexit code `6`で削除を拒否する。完全指定した次のcommandを案内する。

```text
sbxm destroy --force <owner>/<repository>
```

## 9. 削除確認

通常modeでは全データ保護検査に合格した後、force modeでは対象特定後に、次を表示する。

- canonical project ID
- Sandbox名とstate
- managed/unmanagedの分類
- 各path、branch/detached、HEAD、remote到達状態
- 削除対象
- 保持対象
- 再構築command

通常modeをTTYで実行した場合だけ、Sandbox名の完全入力を要求する。yes/noだけでは削除しない。

```text
削除を確認するため、Sandbox名を入力してください:
sbxm-owner-repository-0123456789ab
```

- 完全一致: 続行
- 空または不一致: 削除せずexit `10`
- Ctrl-C/Esc: exit `130`

projectを完全指定した非TTYの通常modeとforce modeでは対話確認を行わない。force modeでは、データ保護検査を省略して削除することをstderrへ明示する。

## 10. Sandbox削除

通常modeではactive sessionがないことを直前に再確認する。force modeでは再確認しない。対象versionのfixtureで固定した各modeのcommandを実行する。

```text
sbx rm <sandbox-name>
sbx rm --force <sandbox-name>
```

削除後、`sbx ls --json`を最大60秒pollし、nameが存在しないことを確認する。

- command失敗: metadataやhost成果物を変更せずexit `5`
- timeout: exit `5`
- 不在確認成功: `registered`と再構築commandを表示してexit `0`

metadataのmanaged worktree宣言は、再構築時の目標構成として保持する。ただしruntime上の作成済み一覧と混同しないよう、Phase 2の`add`はSandbox不在時に一覧を目標として扱い、再作成後に実状態を再検証する。

## 11. 再構築command

metadataから決定的に表示する。

attached:

```text
sbxm add <owner>/<repository> --worktrees 1
```

detached:

```text
sbxm add <owner>/<repository> --worktrees <N> --detach <branch>
```

`open`はnot-createdから自動再構築しない。

## 12. 自動test

- not-createdのno-op
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
- delete command失敗、poll timeout、成功
- host成果物とmetadataが変更されないこと
- 成功後のstateと再構築command

## 13. E2E実機検証

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
12. 2案件切替時のdaemon維持
13. `stop`の複数対象とno-op
14. `ls`の3 stateとunmanaged Sandbox
15. `status`のmanaged/unmanaged、dirty、security診断
16. dirty managedによる`destroy`拒否
17. dirty unmanagedによる`destroy`拒否
18. unpushed commitによる`destroy`拒否
19. cleanかつremote到達済みでtyped confirmation後の`destroy`
20. dirty、unpushed、active sessionを持つrunning Sandboxの`destroy --force`
21. stopped Sandboxの`destroy --force`
22. 非TTYかつ完全指定した通常`destroy`と`destroy --force`
23. host clone、metadata、Dockerfile、exports、archive、image、secretの保持
24. `open`がnot-createdを拒否
25. 同じ`add`による再構築
26. 再構築後のworktree目標構成一致

各caseは実行command、期待exit code、期待stdout/stderr、事後状態をREADMEの手動検証sectionへ記録する。token、path内のMac user名、公開鍵は記録前にredactする。

## 14. Phase 4受入条件

- 通常modeではdirty、untracked、進行中Git操作、unpushed commit、到達不能detached HEADを持つSandboxを削除できない
- managedとunmanagedを同じ安全基準で検査する
- 通常modeは停止中Sandboxを起動せず削除を拒否する
- TTYの通常modeはtyped confirmationなしに削除できない
- projectを完全指定した非TTYの通常modeはデータ保護検査後に対話なしで削除できる
- force modeはproject完全指定を必須とし、データ保護検査と対話確認を省略してrunning/stoppedを削除できる
- force modeでも対象を一意に特定できない場合は削除できない
- 削除失敗時にhost成果物とmetadataを変更しない
- 成功後はregisteredとなり、同じ目標構成で`add`再構築できる
- E2E 26項目が対象exact versionで完了している
