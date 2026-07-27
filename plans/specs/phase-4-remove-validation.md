# Phase 4 実装仕様: `rm`とE2E検証

## 1. 目的

`sbxm rm`は、保存されていない作業を失わないことを確認したうえで、対象Sandbox内部の状態だけを破棄する。host projectと再構築に必要な宣言・成果物は保持し、案件を`registered`状態へ戻す。

```text
sbxm rm [<owner>/<repository>]
```

MVPには`--force`を設けない。dirty、untracked、検査不能なworktreeが1つでもあれば削除しない。

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
| `stopped` | 安全なread-only検査方法がfixtureにあれば検査、なければ利用者確認後に一時起動して検査 |
| `running` | session終了を要求し、worktree検査後に削除 |
| `inconsistent` | exit `4`、自動削除しない |

`rm`はmetadataを削除しないため、成功後は常に`registered`となる。

## 4. 排他と事前確認

1. 対象を引数または単一選択promptで解決
2. project lockを取得
3. stateとSandbox identityを取得
4. active sessionをfixtureで検査
5. 必要なら検査目的でSandboxを起動
6. 全worktreeを列挙・検査
7. 削除対象と保持対象を表示
8. 明示確認
9. Sandboxを削除
10. 不在を検証

削除開始前にproject lockを保持し、他の`add`、`open`、`stop`、`rm`を排除する。

## 5. Active session

Codex、Claude Code、SSH、editor、development serverなどのsessionがactiveであると`sbx` structured outputから判定できる場合、対象sessionを表示してexit code `6`とする。MVPはsessionを強制終了しない。

structuredなsession検査を対象versionが提供しない場合:

- running Sandboxの`rm`は利用者へsession終了を案内して一度exit `10`
- 再実行時の確認promptで「すべてのsessionを終了した」と明示確認させる
- 非TTYではexit `6`
- `sbx rm --force`は使用しない

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

unmanaged worktreeにも同じ規則を適用する。利用者が「不要」と判断していてもMVPではoverrideできない。必要なcommitをpushするか、fileを`.sbx/exports`へ取り出してworktreeをcleanにしてから再実行する。

## 8. 停止中Sandboxの検査

対象`sbx` versionで、停止状態を変えずにfilesystemを検査できるstructured APIがあれば使用する。

存在しない場合:

1. safe daemonを確認
2. 「保存確認のため一時起動する。削除をcancelした場合は元のstoppedへ戻す」と表示
3. TTYで確認。非TTYはexit `6`
4. fixtureで固定した非対話commandにより起動
5. worktreeを検査
6. dirty、error、削除確認cancelならSandboxを再停止
7. 再停止失敗時はexit `5`で明示

一時起動によるfilesystem変更が起こり得るため、起動前後の全worktree statusを比較する。差があれば削除せず停止してexit `6`。

## 9. 削除確認

全安全検査に合格した後だけ、次を表示する。

- canonical project ID
- Sandbox名とstate
- managed/unmanagedの分類
- 各path、branch/detached、HEAD、remote到達状態
- 削除対象
- 保持対象
- 再構築command

確認promptは、Sandbox名の完全入力を要求する。yes/noだけでは削除しない。

```text
削除を確認するため、Sandbox名を入力してください:
sbxm-owner-repository-0123456789ab
```

- 完全一致: 続行
- 空または不一致: 削除せずexit `10`
- Ctrl-C/Esc: exit `130`
- 非TTY: exit `2`

## 10. Sandbox削除

active sessionがないことを直前に再確認する。対象versionの契約に従い、forceを付けず実行する。

```text
sbx rm <sandbox-name>
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
- managed/unmanaged全件列挙
- dirty、untracked、operation in progress
- attachedのunpushed、upstreamなし、pushed
- detached HEADのremote到達・未到達
- path逸脱、metadata missing、Git parse failure
- stoppedの一時起動、cancel時再停止、再停止失敗
- active session拒否
- typed confirmation一致、不一致、非TTY、cancel
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
16. dirty managedによる`rm`拒否
17. dirty unmanagedによる`rm`拒否
18. unpushed commitによる`rm`拒否
19. cleanかつremote到達済みでtyped confirmation後の`rm`
20. host clone、metadata、Dockerfile、exports、archive、image、secretの保持
21. `open`がnot-createdを拒否
22. 同じ`add`による再構築
23. 再構築後のworktree目標構成一致

各caseは実行command、期待exit code、期待stdout/stderr、事後状態をREADMEの手動検証sectionへ記録する。token、path内のMac user名、公開鍵は記録前にredactする。

## 14. Phase 4受入条件

- dirty、untracked、進行中Git操作、unpushed commit、到達不能detached HEADを持つSandboxを削除できない
- managedとunmanagedを同じ安全基準で検査する
- 停止中Sandboxの検査が状態を暗黙に変えたまま残さない
- typed confirmationなしに削除できない
- `sbx rm --force`を使用しない
- 削除失敗時にhost成果物とmetadataを変更しない
- 成功後はregisteredとなり、同じ目標構成で`add`再構築できる
- E2E 23項目が対象exact versionで完了している
