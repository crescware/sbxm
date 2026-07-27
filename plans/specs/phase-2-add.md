# Phase 2 実装仕様: `add`

## 1. 目的

`sbxm add`は、新しいGitHub repositoryを管理対象へ登録し、ホストclone、案件専用Template、Sandbox内bare repository、managed worktreeを作業可能な状態まで構築する。同じ引数による再実行で、中断した工程を安全に再開する。

```text
sbxm add <owner>/<repository>
         [--worktrees <N>]
         [--detach <BRANCH>]
```

Phase 1のDocker Sandboxes互換性fixtureとdaemon安全性probeが承認済みであることを実装開始条件とする。

## 2. 手動手順からの変更

MVPは既存の手動手順を次のように自動化・変更する。

- `.sbx/create` shell scriptを生成せず、Rust workflowが同じ工程を実行する
- 単一の通常cloneではなく、Sandbox内にbare repositoryとmanaged worktreeを作る
- Sandbox名へcanonical project IDのhashを付け、owner/repository間の衝突を防ぐ
- `sbx ls`のtextへ`grep`せず、Phase 1で固定したJSONを完全一致でparseする
- `SSH_AUTH_SOCK`を外した個別`sbx create`だけで安全とは見なさず、daemon instanceを検証する
- 中断時の目標構成をproject metadataへ保存する

中立Workspace、host path非露出、案件限定GitHub secret、利用者がglobal configへ明示したfileの限定copy、Docker socket非共有という要件は維持する。

## 3. Optionと目標構成

| 指定 | `mode` | `start_ref` | managed数 |
|---|---|---|---:|
| 指定なし | `attached` | remote default branch | 1 |
| `--worktrees 1` | `attached` | remote default branch | 1 |
| `--detach develop` | `detached` | `develop` | 1 |
| `--worktrees 1 --detach develop` | `detached` | `develop` | 1 |
| `--worktrees N --detach develop` | `detached` | `develop` | N |
| `--worktrees N`、N >= 2 | usage error | - | - |

- `N`は1以上32以下
- `BRANCH`は1〜255 byte、NUL、改行、先頭`-`を拒否する
- `BRANCH`はSandbox内で`git check-ref-format --branch`により再検証する
- `--detach`へ`origin/`、`refs/heads/`、commit hashは渡さない。利用者が指定するのはremote branch名だけ
- detached modeでは全managed worktreeを同じ`origin/<BRANCH>` commitから作る
- attached modeではremote default branchをtrackingするlocal branchを1つ作る

## 4. Project単位の排他

既存projectではmutationの前に`<project-root>/.sbx/sbxm.lock`をexclusive lockする。新規projectではowner directory、project root、`.sbx`を作成または検証した直後にproject lockを取得し、以後のmutationへ進む。

- lock待機は10秒
- timeoutは対象projectを表示してexit code `5`
- lockはworkflow終了まで保持する
- daemonを操作する区間はPhase 1のglobal daemon lockをproject lockの後に取得する
- 複数lockが必要な将来機能ではcanonical ID昇順に取得する

lock fileの存在自体は処理中を意味しない。OS file lockの取得結果を使う。

## 5. Project metadataの作成と再実行

### 5.1 新規

入力を検証し、衝突検査を完了した後、次を行う。

1. owner directoryとproject root、`.sbx`を作る
2. project lockを取得する
3. bundled DockerfileのSHA-256を求める
4. 目標構成を含むmetadataをatomic writeする
5. 以後の外部mutationへ進む

`provisioning`には`mode`、`start_ref`、`requested_worktrees`、`dockerfile_sha256`を保存する。attached modeの`start_ref`は、remote default branchを解決するまで空を許可し、解決直後にatomic updateする。

### 5.2 再実行

- canonical IDが違う: exit code `4`
- mode、明示start ref、requested countが違う: exit code `2`
- Dockerfile hashが保存値と違う: 自動buildせず、MVPでは再構築非対応を案内してexit code `4`
- 成果物が期待状態と一致する: skip
- 成果物がない: 実行
- 所有関係または内容を証明できない: exit code `4`

引数なしで作成されたattached projectは、保存済みのresolved default branchを再実行時に使用する。GitHub側default branchが変わっても自動変更しない。

`destroy`後の`registered`状態では、保存済み目標構成と正規化後に同じ意味となる引数だけを受け付け、Sandbox以降の工程を再実行する。例えばoptionなしと`--worktrees 1`は同じattached目標構成として扱う。

## 6. 導出名

```text
sandbox_name   = 方向性文書のSandbox名
image_name     = "<sandbox-name>-template:v1"
workspace      = "/tmp/docker-sandboxes/<sandbox-name>"
bare_root      = "/home/agent/work/<repository-lower>"
bare_git_dir   = "<bare-root>/.git"
worktree       = "<bare-root>/<repository-lower>.tree-<index>"
```

Docker imageには次のlabelを付ける。

```text
io.crescware.sbxm.canonical-id=<canonical-id>
io.crescware.sbxm.dockerfile-sha256=<sha256>
io.crescware.sbxm.metadata-version=1
```

既存imageは`docker image inspect`で全labelが一致した場合だけ再利用する。

## 7. Workflow

全工程は`inspect -> decide -> mutate -> verify -> record`で実行する。verifyに失敗したら後続工程へ進まない。

### 7.1 Host directory

作成するdirectory:

```text
<project-root>/
├── <repository-lower>/
└── .sbx/
    ├── exports/
    └── .cache/
```

- owner directory、project root、`.sbx`、`exports`、`.cache`はsymlinkを拒否
- 新規directoryのpermissionは利用者のumaskに従う。ただし`.sbx`と`.cache`は`0700`
- 既存の非directoryはexit code `4`

### 7.2 Host clone

programとarguments:

```text
git clone
  git@github.com:<owner-display>/<repository-display>.git
  <project-root>/<repository-lower>
```

既存cloneを再利用する条件:

- 通常のnon-bare Git worktree
- top-levelが期待pathと一致
- `origin`の正規化済みremoteがcanonical IDと一致
- `.git`がproject root外を指すworktree fileではない

dirty状態は再実行を妨げない。remote不一致、複数origin、壊れたrepositoryはexit code `4`。

### 7.3 Dockerfile

初回だけbundled templateから`<project-root>/.sbx/Dockerfile`を`0600`で作る。内容は元の手動手順を正本とし、少なくとも次を満たす。

- base imageはreview済みdigestでpinする。mutable tagだけを使用しない
- `ca-certificates`、`curl`、`wget`、`gh`、`jq`を導入
- `/home/agent/work`を`agent`所有で作成
- secretの実値を書かない
- `GH_TOKEN`などへ実tokenを書かない。proxy-managed方式が対象`sbx` versionで必要な場合だけfixtureに基づくsentinelを設定
- mise、Codex、Claude Codeのinstallerはversionまたはdigestをpinする
- interactive shellの開始位置を`/home/agent/work`にする
- `WORKDIR /home/agent/work`

既存Dockerfileは保存済みhashと一致する場合だけ再利用する。利用者編集後のrebuildはMVP対象外。

### 7.4 Image buildとarchive

```text
docker build
  --label <label> ...
  --tag <image-name>
  --file <dockerfile>
  <project-root>/.sbx

docker image save
  <image-name>
  --output <project-root>/.sbx/.cache/template.tar.tmp
```

- `docker build`成功後にinspectし、image IDとlabelを検証
- archiveは一時pathへ保存し、成功後にatomic rename
- 既存archiveはtarとして読め、期待image manifestと一致する場合だけ再利用
- `.tmp`が残っている場合は自動上書きせずexit code `4`

### 7.5 Template load

Phase 1 fixtureで確定した次の操作を使用する。

```text
sbx template load <template-archive>
```

load後、fixtureで定義したread-only commandにより、Templateが期待image IDと対応することを検証する。runtimeが対応関係を観測できない場合、既存Templateは再利用せず、同名存在時にexit code `4`とする。

### 7.6 Safe daemon

Phase 1で選択した方式を使用する。

- marker方式: 現在daemon instance IDと安全markerを照合
- restart方式: active sessionがないことを確認し、`sbx daemon stop`、`SSH_AUTH_SOCK`をunsetした`sbx daemon start --detach`

安全性を証明できなければSandboxを作成せずexit code `6`。

### 7.7 Sandbox create

中立Workspaceを`0700`で作り、symlinkを拒否する。空directoryでなくてもよいが、sbxmが作ったownership marker以外のentryがあればexit code `4`。

期待する外部command:

```text
sbx create
  --name <sandbox-name>
  --template <image-name>
  shell
  <workspace>
```

実際のargumentsはPhase 1 fixtureのexact versionに従う。runnerは`SSH_AUTH_SOCK`をunsetする。

既存Sandboxを再利用する条件:

- `sbx ls --json`のnameが完全一致
- workspaceのreal pathが期待する中立Workspace
- Template/image identityがinspect可能で一致
- metadataのcanonical IDと対応

1項目でも確認不能または不一致ならexit code `4`。

### 7.8 宣言fileの配置

global configの`[[files]]`に宣言されたhost fileを、Sandbox内の`agent` homeからの相対pathへ配置する。特定のAgentやtoolの設定形式を解釈しない。

```text
sbx cp --follow-link
  <source>
  <sandbox-name>:/tmp/sbxm-file-<index>

sbx exec --user root <sandbox-name> --
  install ... /home/agent/<destination>
```

- sourceはabsolute pathの通常fileに限り、symlink、socket、directoryを拒否する
- source fileは1件につき1 MiB以下
- destinationは`/home/agent`を基準とするrelative pathとし、absolute path、`..`、symlink経由の逸脱を拒否する
- Sandbox側の親directoryは`0700`、fileは`0600`、owner/groupは`agent`
- 一時fileは成功・失敗のどちらでも削除する
- 既存destinationが同一内容ならskipする
- 既存destinationが異なる場合は上書きせず、対象pathを示してexit code `4`
- sourceが存在しない、または安全性を検証できない場合はcopyせずexit code `4`
- file内容をstdout、stderr、log、metadataへ出力しない
- credential、token、秘密鍵には使用せず、Docker Sandboxesのsecret機能を案内する

### 7.9 Git identityとprotocol

Sandbox内で次を引数配列として実行する。

```text
git config --global user.name <config.git.user_name>
git config --global user.email <config.git.user_email>
gh config set git_protocol https --host github.com
```

既存値が同一ならskip。異なる場合は、別利用者のSandboxである可能性があるため自動上書きせずexit code `4`。

### 7.10 GitHub secret

案件限定fine-grained personal access tokenの発行と入力は自動化しない。必要permissionは`Contents: read/write`、`Metadata: read`、必要な場合だけPull requests、Issues、Actionsとする。

期待する利用者向けcommand:

```text
sbx secret set <sandbox-name> github
```

存在確認はPhase 1 fixtureで固定したread-only commandとstructured outputだけを使用する。secret値を取得・表示しない。

未登録なら、発行条件と上記commandを表示してexit code `10`で停止する。登録後の再実行はSandboxを再利用して次工程へ進む。

### 7.11 Bare clone

Sandbox内:

```text
mkdir -p <bare-root>
git clone --bare
  https://github.com/<owner-display>/<repository-display>.git
  <bare-git-dir>
git --git-dir <bare-git-dir> config
  remote.origin.fetch +refs/heads/*:refs/remotes/origin/*
git --git-dir <bare-git-dir> fetch --prune origin
```

再利用条件:

- `<bare-git-dir>`がbare repository
- originがcanonical IDと一致
- fetch refspecが完全一致
- `git fsck --connectivity-only`が成功

directoryは存在するが条件不一致なら自動削除せずexit code `4`。

### 7.12 Start ref解決

attached mode:

1. `git ls-remote --symref origin HEAD`相当からdefault branchを得る
2. `refs/heads/<branch>`だけを受け付ける
3. `refs/remotes/origin/<branch>`の存在を確認
4. metadataの空`start_ref`へbranch名をatomic write

detached modeでは`refs/remotes/origin/<start_ref>`の存在を確認する。ambiguous ref、tag、commit直接指定は拒否する。

### 7.13 Managed worktree

indexは0から`requested_worktrees - 1`まで固定する。再実行時に別の空きindexへずらさない。

attached:

```text
git --git-dir <bare-git-dir> worktree add
  --track -b <branch>
  <worktree-path>
  refs/remotes/origin/<branch>
```

detached:

```text
git --git-dir <bare-git-dir> worktree add
  --detach
  <worktree-path>
  refs/remotes/origin/<branch>
```

各作成直後に、path、HEAD、branch/detached、created_fromを検証し、metadataへ1件ずつatomic追記する。

既存pathの扱い:

- metadataに記録済みでGit状態も一致: skip
- Sandboxを再構築した直後でmetadataに記録済みのpathがまだ存在しない: 保存済み宣言を作成予定pathとして使用し、作成後に同じmetadata entryを検証する
- metadata未記録だが、期待path、期待HEAD、期待modeが一致し、作成予定indexである: interrupted createとしてmanagedへ採用
- 内容、HEAD、modeのいずれかが不一致: exit code `4`
- managed用名前空間外のGit worktree: unmanagedとして変更しない

## 8. 成功出力

少なくとも次を表示する。

```text
Project
Sandbox
Creation mode
Start branch
Managed worktree count
Host clone
Sandbox state

WORKTREE  CREATED FROM  HEAD  MODE
...

FILE  DESTINATION  RESULT
...
```

各managed worktreeについて`mise.toml`、`.mise.toml`、`.tool-versions`の有無をread-onlyで確認し、`mise trust`と`mise install`を自動実行せず案内する。

## 9. Errorと副作用

- 各工程失敗後は後続工程を実行しない
- 成功済み成果物をrollback目的で削除しない
- 失敗時は、完了工程、失敗工程、対象、再実行commandを表示する
- 外部command失敗はprogram、safe args、cwd、exit status、stderr原文を表示する
- token、secret、host SSH情報は表示しない
- parse不能な外部出力を推測で成功扱いしない

## 10. 自動test

- option matrixとmutation前validation
- metadata新規作成、同一再開、異なる引数拒否
- 各工程直後に失敗させた再実行
- host clone remote検証
- Dockerfile、image label、archiveの一致・不一致
- Sandbox名完全一致とworkspace検証
- safe daemon成功・不明・active session
- 宣言fileのsource、destination、path逸脱、同一、競合、一時file cleanup
- secret不在による中断と登録後再開
- bare clone、refspec、default branch
- attached 1 tree、detached 1/3 trees
- worktree作成途中のmetadata復元
- managed/unmanaged分離
- stderr、exit code、secret redaction

## 11. 実機受入条件

- 新規案件をoptionなしで最後まで構築できる
- host cloneはSSH、Sandbox cloneはHTTPS proxy credentialを使用する
- workspaceは中立pathだけで、実案件pathとMac user homeをSandboxへ公開しない
- Sandbox内に`SSH_AUTH_SOCK`がなく、`ssh-add -L`がhost keyを返さない
- Docker socketを渡していない
- 1 treeでもbare repositoryとworktreeを分離する
- detached 3 treesが同じ明示branchの同じcommitから作られる
- secret未登録で安全に中断し、登録後に同じcommandで再開できる
- 各工程でprocessを中断しても、同じ目標構成で再開または明確な不整合停止となる
- `destroy`後の`registered`案件を同じ`add`で再構築できる
