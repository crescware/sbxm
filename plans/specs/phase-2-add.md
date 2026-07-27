# Phase 2 実装仕様: `add`と`sync-files`

## 1. 目的

`sbxm add`は、新しいGitHub repositoryを管理対象へ登録し、ホストclone、案件専用Template、Sandbox内bare repository、managed worktreeを作業可能な状態まで構築する。構築が中断した案件へ同じcommandを再実行すると、metadataに保存した目標構成から継続する。

`sbxm sync-files`は、構築済みでrunningの案件について、現在のglobal configに宣言されたfileをSandboxへ再配置する。projectの登録、構築継続、worktree構成変更、Dockerfileのbuild、image・Template操作は行わない。

```text
sbxm add <owner>/<repository>
         [--worktrees <N>]
         [--detach <BRANCH>]
sbxm sync-files <owner>/<repository>
```

Phase 1の共通型、command runner、compatibility fixture基盤を利用する。調査やlocal実装はPhase 1 PRのreviewと並行できるが、Phase 2 PRはreview結果を取り込む。`add`の実機受入までに、Template、daemon、create、secret、execについて本Phaseが使用するexact-version fixtureとdaemon安全性probeを完成させる。

## 2. 手動手順からの変更

MVPは既存の手動手順を次のように自動化・変更する。

- `.sbxm/create` shell scriptを生成せず、Rust workflowが同じ工程を実行する
- 単一の通常cloneではなく、Sandbox内にbare repositoryとmanaged worktreeを作る
- Sandbox名へcanonical project IDのhashを付け、owner/repository間の衝突を防ぐ
- `sbx ls`のtextへ`grep`せず、Phase 1で固定したJSONを完全一致でparseする
- `SSH_AUTH_SOCK`を外した個別`sbx create`だけで安全とは見なさず、全Sandboxのactive session不在を確認してdaemonを安全に再起動する
- 中断時の目標構成をproject metadataへ保存し、以降は同じ`add`で継続する

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

再実行した`add`でoptionを省略した場合はmetadataに保存された目標構成を使用する。optionを指定した場合は保存値と完全一致することを要求し、不一致ならmutation前にexit code `1`とする。

## 4. Project単位の排他

既存projectではmutationの前に`<project-root>/.sbxm/project.lock`をexclusive lockする。新規projectではowner directory、project root、`.sbxm`を作成または検証した直後にproject lockを取得し、以後のmutationへ進む。

- lock待機は10秒
- timeoutは対象projectを表示してexit code `1`
- lockはworkflow終了まで保持する
- daemonを操作する区間はPhase 1のglobal daemon lockをproject lockの後に取得する
- 複数lockが必要な将来機能ではcanonical ID昇順に取得する

lock fileの存在自体は処理中を意味しない。OS file lockの取得結果を使う。

`destroy`は管理解除の一部としてlock fileを削除するため、全commandは削除・再作成によるfile identityの変更を考慮してproject lockを取得する。

1. lock pathをsymlinkを追跡せずopenまたはcreateする
2. 開いたfileへexclusive lockを取得する
3. lock取得後、file descriptorと現在のlock pathをそれぞれ`fstat`相当と`lstat`相当で取得し、device IDとinodeが一致することを確認する
4. pathが存在しない、symlinkである、またはidentityが一致しない場合は、削除済みの古いlock fileを取得したものとして解放し、現在のpathで取得をやり直す
5. identityが一致した後にmetadata、filesystem、外部状態を再取得し、preconditionを判定する

retryを含む全体へ10秒のlock timeoutを適用する。permission、owner、file typeが不正な場合は安全に置換せずexit code `1`とする。

## 5. Project metadataの作成と構築継続

### 5.1 新規

入力を検証し、衝突検査を完了した後、次を行う。

1. owner directoryとproject root、`.sbxm`を作る
2. project lockを取得する
3. Dockerfileがなければbundled templateから作り、存在すれば利用者が管理する既存fileとして採用する
4. 採用するDockerfileのSHA-256を求める
5. 目標構成を含むmetadataをatomic writeする
6. 以後の外部mutationへ進む

`provisioning`には`mode`、`start_ref`、`requested_worktrees`、`dockerfile_sha256`を保存する。attached modeの`start_ref`は、remote default branchを解決するまで空を許可し、解決直後にatomic updateする。

### 5.2 `add`の再実行

有効なmetadataが存在する場合、保存済み目標構成とoptionの一致を検証してから、成果物を順にinspectする。

- 構築未完了: 保存済み目標構成から構築を継続する
- rebuild intentが存在する: `sbxm rebuild <owner>/<repository>`を案内し、`add`では継続しない
- 構築完了: 構築済みであることを表示し、何も変更せずexit code `0`
- 保存済み目標構成とoptionが不一致: exit code `1`
- canonical ID、成果物、所有関係が不一致: exit code `1`

各工程では次の共通規則を使用する。

- 成果物が期待状態と一致する: skip
- 成果物がない: 実行
- 所有関係または内容を証明できない: exit code `1`

再実行では保存済みのmode、resolved start ref、requested countを使用する。GitHub側default branchが変わっても自動変更しない。現在のDockerfile hashがmetadataの適用済みhashと異なり、対応imageのbuild前なら、現在のDockerfileを初回構築の目標としてhashを更新して続行する。対応imageが既に完成している場合は、保存済みhashの世代を使って初回構築を完了し、現在のDockerfileを反映する`sbxm rebuild <owner>/<repository>`を成功出力で案内する。初回構築の途中へ別世代を混在させない。

## 6. 導出名

```text
sandbox_name   = 方向性文書のSandbox名
image_name     = "<sandbox-name>-template:<dockerfile-sha256-first-12-hex>"
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

`docker build`、`docker image save`、hostとSandbox内のGit clone・fetch、Template load、Sandbox createはPhase 1 runnerの`passthrough`を使用し、各外部toolが出す進捗を実行中にそのまま表示する。sbxmは独自のprogress表示を重ねない。inspect、labelやarchiveの検証、secret存在確認など、結果をparseまたは秘匿するcommandは`capture`する。

### 7.1 Host directory

作成するdirectory:

```text
<project-root>/
├── <repository-lower>/
└── .sbxm/
    ├── exports/
    └── .cache/
```

- owner directory、project root、`.sbxm`、`exports`、`.cache`はsymlinkを拒否
- 新規directoryのpermissionは利用者のumaskに従う。ただし`.sbxm`と`.cache`は`0700`
- 既存の非directoryはexit code `1`

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

dirty状態は`add`の構築継続を妨げない。remote不一致、複数origin、壊れたrepositoryはexit code `1`。

GitHubへのSSH認証とrepository accessは、owner、repository、利用者のSSH設定によって結果が変わるため、projectを持たない`status --global`ではgenericな疎通検査を行わない。本工程の対象remoteに対するcloneを正本の検査とし、失敗時は外部commandの診断規則に従ってGit/SSHのexit statusとredact済みstderrを表示する。

### 7.3 Dockerfile

`<project-root>/.sbxm/Dockerfile`がない場合だけbundled templateから`0600`で作る。既存Dockerfileは利用者が管理・編集するfileとして内容を変更せず採用する。内容は元の手動手順を初期templateの正本とし、少なくとも次を満たす。

- base imageはApple siliconを含む公式multi-platform image `docker.io/docker/sandbox-templates:shell-docker@sha256:39cf20eca861ec92747487af6197f6d916f774bdb98245d267dbd8dfd3debb05`へpinする。mutable tagだけを使用しない
- base imageが提供するUID `1000`の非root `agent`、`/home/agent`、passwordless sudo、inner Docker Engineを使用する
- MVPの固定tool setとして`git`、`openssh-client`、`coreutils`、`ca-certificates`、`curl`、`wget`、`gh`、`jq`をDockerfileで導入する。base imageに含まれることだけを導入済みの根拠にしない
- package installationを含む`docker build`の成功を固定tool set導入の判定とし、Sandbox操作のたびに全commandの存在をprobeしない
- `/home/agent/work`を`agent:agent`所有で作成
- secretの実値を書かない
- `GH_TOKEN`などへ実tokenを書かない。proxy-managed方式が対象`sbx` versionで必要な場合だけfixtureに基づくsentinelを設定
- mise、Codex、Claude Codeのinstallerはversionまたはdigestをpinする
- interactive shellの開始位置を`/home/agent/work`にする
- `WORKDIR /home/agent/work`
- MVPではbuild context由来のfileを必要としないため、`COPY`と`ADD`を使用しない

このtool setはオーナーの実務環境として選択したMVPの初期templateであり、列挙した全toolがsbxm自身の直接依存であることを意味しない。利用者が生成後のDockerfileを直接編集し、`sbxm rebuild`でpackage、installer、versionの変更をSandboxへ適用することはMVP要件に含む。別途、これらを設定やoptionで選択・合成する機構はMVP対象外とし、Dockerfileと`rebuild`を唯一のcustomize経路とする。

`add`は採用したDockerfileのSHA-256をmetadataとimage labelへ適用済みhashとして保存する。利用者による手修正を許可し、構築完了後の保存済みhashとの不一致だけを理由にmetadataやDockerfileを不正とは扱わない。変更の適用は`rebuild`が担当する。

### 7.4 Image buildとarchive

各buildでは、Rustの`tempfile::TempDir`を使い、OSの一時領域へprefix `sbxm-build-context-`を持つ一時directoryを作成する。

- 作成時permissionは`0700`
- symlinkではない通常directoryであることを確認する
- pathをcanonicalizeし、build直前にも空であることを確認する
- project file、config、secretその他の内容を配置しない
- Dockerfileはabsolute pathを`--file`へ渡し、一時directoryだけをbuild contextへ渡す
- buildの成功・失敗にかかわらず`TempDir`のdropで削除する
- cleanup失敗は残存pathをwarningとして表示する。build自体が成功していればcleanup失敗だけで成果物を失敗扱いにしない
- process強制終了で残存しても次回実行では再利用せず、探索や自動削除を行わない

```text
docker build
  --label <label> ...
  --tag <image-name>
  --file <dockerfile>
  <ephemeral-empty-build-context>

docker image save
  <image-name>
  --output <project-root>/.sbxm/.cache/template-<dockerfile-sha256-first-12-hex>.tar.tmp
```

- `docker build`成功後にinspectし、image IDとlabelを検証
- archive工程へ到達するたびに既存archiveを再利用せず、`docker image save`で新しく生成する。再利用による性能最適化はMVP対象外とする
- archiveはSHA-256先頭12桁を含む世代別pathへ保存する
- project lock保持下で同世代の`.tmp`が残っている場合は、中断した未完成cacheとして削除してから生成する
- 正式なarchiveが既にあっても、新しい一時archiveの生成と検証が完了するまでは変更しない
- `docker image save`成功後に一時archiveのmanifest、full SHA-256 label、image IDを検証し、同じ`.cache` directory内で正式pathへatomic renameして置き換える

MVPではbuild、save、loadに必要なhostまたはDocker storage容量を事前に見積もらず、容量不足を避けるための旧世代削除や自動再試行も行わない。必要容量はDockerfile、Docker内部storage、build cache、image、archiveの状態に依存し、根拠のある安全な削除規則をMVPでは定義できないためである。失敗時はPhase 1 runnerのpassthroughと共通error規則に従い、外部toolの出力、失敗工程、対象、同じcommandによる再実行方法を表示する。

### 7.5 Template load

Phase 1 fixtureで確定した次の操作を使用する。

```text
sbx template load <template-archive>
```

load後、fixtureで定義したread-only commandにより、Templateが期待image IDと対応することを検証する。runtimeが対応関係を観測できない場合、既存Templateは再利用せず、同名存在時にexit code `1`とする。

### 7.6 Safe daemon

Phase 1の共通手順を使用する。global daemon lockを取得し、全Sandboxのactive session不在をstructured outputから確認した後、`sbx daemon stop`を実行し、`SSH_AUTH_SOCK`をunsetした環境で`sbx daemon start --detach`を実行する。

active sessionを検出した場合、またはsession不在を証明できない場合は、daemonを変更せずSandboxも作成せずexit code `1`とする。session検査commandの失敗、timeout、parse不能はexit code `1`とする。

### 7.7 Sandbox create

中立Workspaceを`0700`で作り、symlinkを拒否する。sbxm独自のownership markerや作成履歴fileは置かない。既存workspaceはowner、permission、file type、real pathと内容がvalidation規則を満たす場合に、作成元を問わず再利用する。満たさない場合は内容を変更せずexit code `1`とする。

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

1項目でも確認不能または不一致ならexit code `1`。誰が作成したか、またはsbxm独自のmarkerがあるかは条件にしない。

### 7.8 宣言fileの配置と`sync-files`

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
- `add`では既存destinationが異なる場合は上書きせず、対象pathを示してexit code `1`
- `sync-files`では現在のglobal configにある宣言を明示的な再配置要求として扱い、既存destinationが異なる場合も安全な一時fileとrenameを使って上書きする
- global configから削除された宣言のdestinationは、`sync-files`でもSandboxから削除しない
- sourceが存在しない、または安全性を検証できない場合はcopyせずexit code `1`
- file内容をstdout、stderr、log、metadataへ出力しない
- credential、token、秘密鍵には使用せず、Docker Sandboxesのsecret機能を案内する

`sync-files`はproject metadata、Sandbox identity、running stateをread-onlyで検証してから、本sectionのfile配置だけを実行する。stopped Sandboxを暗黙に起動せず、`sbxm open <owner>/<repository>`後の再実行を案内してexit code `1`とする。registered、unmanaged、inconsistentでは何も配置しない。

### 7.9 Git identityとprotocol

Sandbox内で次を引数配列として実行する。

```text
git config --global user.name <config.git.user_name>
git config --global user.email <config.git.user_email>
gh config set git_protocol https --host github.com
```

既存値が同一ならskip。異なる場合は、別利用者のSandboxである可能性があるため自動上書きせずexit code `1`。

### 7.10 GitHub secret

案件限定fine-grained personal access tokenの発行と入力は自動化しない。必要permissionは`Contents: read/write`、`Metadata: read`、必要な場合だけPull requests、Issues、Actionsとする。

期待する利用者向けcommand:

```text
sbx secret set <sandbox-name> github
```

存在確認はPhase 1 fixtureで固定したread-only commandとstructured outputだけを使用する。secret値を取得・表示しない。

未登録なら、発行条件と上記commandを表示して前提条件不足のexit code `1`で停止する。登録後は同じ`add`を再実行し、Sandboxを再利用して次工程へ進む。

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

directoryは存在するが条件不一致なら自動削除せずexit code `1`。

### 7.12 Start ref解決

attached mode:

1. `git ls-remote --symref origin HEAD`相当からdefault branchを得る
2. `refs/heads/<branch>`だけを受け付ける
3. `refs/remotes/origin/<branch>`の存在を確認
4. metadataの空`start_ref`へbranch名をatomic write

detached modeでは`refs/remotes/origin/<start_ref>`の存在を確認する。ambiguous ref、tag、commit直接指定は拒否する。

### 7.13 Managed worktree

indexは0から`requested_worktrees - 1`まで固定する。`add`再実行時に別の空きindexへずらさない。

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
- 内容、HEAD、modeのいずれかが不一致: exit code `1`
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
- 失敗時は、完了工程、失敗工程、対象、同じ引数の`add`を次のcommandとして表示する
- 外部command失敗はprogram、safe args、cwd、exit status、stderr原文を表示する
- token、secret、host SSH情報は表示しない
- parse不能な外部出力を推測で成功扱いしない

## 10. 自動test

- option matrixとmutation前validation
- metadata新規作成、構築途中と構築済みでの`add`再実行
- project lock取得後のidentity一致、削除・再作成による不一致時retry、symlink・owner・permission拒否、retry timeout
- 各工程直後に失敗させた同じ`add`による継続
- 再実行optionの省略、一致、不一致
- host clone remote検証
- 新規・既存Dockerfileの採用、初回build前のhash更新、構築済みDockerfile変更時の`rebuild`案内
- 一時build contextのpermission、通常directory、空状態、`.cache`・`exports`非包含、成功・失敗時cleanup
- archive工程ごとの一律再生成、中断後の`.tmp`削除と再開、検証完了前の既存正式archive維持、atomic置換
- Sandbox名完全一致とworkspace検証
- 手作業で作成したvalidなmetadata、workspace、image、Sandbox、Git repository、worktreeの受け入れ
- 作成元にかかわらず同じ不整合を同じ診断とexit codeで拒否すること
- safe daemon成功・不明・active session
- build、save、clone、fetch、Template load、Sandbox createのpassthroughと、structured出力のcapture
- 宣言fileのsource、destination、path逸脱、同一、`add`時の競合、`sync-files`時の上書き、宣言削除時の保持、一時file cleanup
- `sync-files`のrunning限定、stopped非起動、他工程への副作用なし
- secret不在による中断と登録後の`add`再実行
- bare clone、refspec、default branch
- attached 1 tree、detached 1/3 trees
- worktree作成途中のmetadata復元
- managed/unmanaged分離
- stderr、exit code、secret redaction

## 11. 実機受入条件

- 新規案件をoptionなしで最後まで構築できる
- host cloneはSSH、Sandbox cloneはHTTPS proxy credentialを使用する
- bundled Dockerfileが固定済み`docker/sandbox-templates:shell-docker`をbaseとし、MVPで固定したtool setを導入できる
- workspaceは中立pathだけで、実案件pathとMac user homeをSandboxへ公開しない
- Sandbox内に`SSH_AUTH_SOCK`がなく、`ssh-add -L`がhost keyを返さない
- Docker socketを渡していない
- 1 treeでもbare repositoryとworktreeを分離する
- detached 3 treesが同じ明示branchの同じcommitから作られる
- secret未登録で安全に中断し、登録後に同じ`add`で継続できる
- 各工程でprocessを中断しても、同じ`add`で同じ目標構成を継続または明確な不整合停止となる
- 構築済み案件への`add`再実行は副作用のないno-op成功となる
- `sync-files`が宣言fileだけを再配置し、Git、worktree、Dockerfile、image、Templateを変更しない
- Dockerfileを手修正でき、`status`で適用済みhashとの差を確認できる
- `destroy`後はmetadataがなくなり、新しい`add`で再構築できる
