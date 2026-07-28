# Phase 2 実装仕様: `add`、`prepare`と`apply`

## 1. 目的

`sbxm add`は、新しいGitHub repositoryを管理対象へ登録し、host cloneを用意する。Sandboxは作らない。

`sbxm prepare`は、登録済み案件について、案件専用Template、Sandbox、Sandbox内bare repository、managed worktreeを作業可能な状態まで構築する。構築が中断した案件へ同じcommandを再実行すると、metadataに保存した目標構成から継続する。

工程を2つに分けるのは、GitHub tokenの登録先がSandbox名であり、その名前が登録時にはじめて確定するためである。`add`が名前を示し、利用者がtokenを登録し、`prepare`が構築する。人間の手続きはcommandとcommandの間に置き、commandの途中に置かない。

`sbxm apply`は、構築済みでrunningの案件へ、Sandboxを作り直さずに反映できる変更を適用する。適用する対象はoptionで明示させ、省略した対象には触れない。projectの登録、構築継続、Dockerfileのbuild、image・Template操作は行わない。

| option | 適用するもの |
|---|---|
| `--files` | 現在のglobal configに宣言されたfileをSandboxへ再配置する。既存のfileを上書きする |
| `--worktrees N` | managed worktreeの目標本数を`N`にし、足りない分を作る |

どちらも指定しない実行はusage errorとする。省略した対象へ触れない以上、何も指定しない実行は何をするか決まらない。`--files`が既存のfileを上書きすることも、明示を必須とする理由になる。

作り直しを要する変更は`rebuild`が担当する。`apply`と`rebuild`の違いは、Sandboxを作り直すかどうかである。

```text
sbxm add <owner>/<repository>
         [--worktrees <N>]
         [--detach <BRANCH>]
sbxm prepare <owner>/<repository>
sbxm apply <owner>/<repository>
           [--files]
           [--worktrees <N>]
```

`add`のhost cloneは利用者のSSH鍵でhost上から取るため、Sandboxのsecretを必要としない。tokenが要るのはSandbox内のbare cloneであり、これは`prepare`の工程である。

Phase 1が実装した共通型、config、command runner、`sbx`出力parserを利用する。調査やlocal実装はPhase 1 PRのreviewと並行できるが、Phase 2 PRはreview結果を取り込む。

## 2. 本Phaseで追加する共通基盤

Phase 1は`init`と`status --global`が必要とする範囲だけを実装した。次はPhase 2が最初の呼び出し側となるため、本Phaseで実装する。実装は利用するworkflowと同じPRへ入れ、呼び出し側のないまま追加しない。

- `ProjectId`のcanonical形式
  - ASCII lowercaseの`owner/repository`を比較の正本とする
  - 表示にはGitHub上の表記を使う
- Sandbox名の導出
  - 導出規則は方向性文書を正本とする
  - 同じcanonical project IDが常に同じ名前になること
  - `sbxm-<slug>-<hash>`が63 byte以内に収まること
  - hash prefixの衝突をname collision errorとして扱うこと
- Project metadataのschemaと永続化
  - schemaは方向性文書を正本とする
  - 本Phaseが書くのは`provisioning`まで
  - `rebuild`のintentはPhase 4で追加する
- Project metadata探索
  - `base_path`直下のowner directoryと、その直下の`*.project/.sbxm/project.toml`だけを読む
  - directory entryとmetadata fileのsymlinkを追跡しない
  - すべてのmetadataをparseしてから結果を返す
  - canonical ID重複、導出path不一致、Sandbox名衝突は一覧化してexit code `1`
  - 1件の破損を無視して部分的な案件一覧を返さない
  - 並び順はcanonical IDのbyte昇順
- 既存fileのatomic置き換え
  - 既存fileのpermissionとidentityを検証する
  - 同一directoryの一時fileからatomic renameする
  - symlinkは拒否する
- 外部command runnerの追加policy
  - 人間向け進捗を転送する`passthrough`
  - 作業directoryの指定
  - timeout classのimage build/save、Sandbox lifecycle、repository転送（cloneとfetch）
- Sandbox作成後にcredentialの隔離をSandboxの中から確認する手順
- `status --global`への行の追加
  - Docker Sandboxes login状態
  - 既存行の並び順を変えないため、追加行は表の末尾へ足す

metadataと外部状態のvalidationは、作成元や作成履歴を条件にしない共通処理として実装する。read-only commandとmutation commandは同じvalidation規則を使用する。手作業または別toolで作成されたmetadataと成果物も、全規則を満たす場合はsbxmが作成したものと区別せず受け入れる。

## 3. 外部commandの契約

Docker Sandboxes CLIはEarly Accessであり、出力書式は変わり得る。本Phaseが読む出力は、実装PRの中で対象Mac上の実出力に対して検証する。

- 使用するsubcommandの`--help`と、読むstructured outputを実機で確認する
- 確認した出力に対してparser testを書く
- 代表的失敗のexit statusをtestで固定する
- `sbx rm --force`について、runningとstoppedの挙動を確認する
- 新世代のimage、archive、Templateをloadした後も既存Sandboxを維持できることを確認する
- Sandbox削除後に新Templateから同名Sandboxを再作成できることを確認する

採取済み出力をversionごとに束ねるmanifestは持たない。安全性は、mutation直前に読むstructured outputをparseできるかで判定する。

### 3.1 本実装が前提としている外部commandと出力

実装は次のcommandと出力を前提とする。この一覧は対象Mac上での確認対象であり、実出力が異なる場合は実装とこの節を同時に直す。

| 用途 | command | 読む値 |
|---|---|---|
| Sandbox一覧 | `sbx ls --json` | `{"sandboxes": [...]}`で包まれた各entryの`name`、`status`（`running`と`stopped`だけ）、`workspaces`（配列。sbxmのSandboxは1件だけ持つ） |
| Template一覧 | `sbx template ls --json` | `{"images": [...]}`で包まれた各entryの`repository`と`tag`。runtimeは`docker.io/library/`を補って表示する |
| Template load | `sbx template load <archive>` | exit statusのみ |
| Sandbox作成 | `sbx create --name <name> --template <image> shell <workspace>` | exit statusのみ |
| Sandbox内実行 | `sbx exec [--user root] <name> -- <argv>` | stdoutとexit status。`--`の有無はどちらも受け付ける |
| file転送 | `sbx cp --follow-link <source> <name>:<path>` | exit statusのみ |
| secret存在確認 | `sbx secret ls <name>` | 2つの表を出す。前半は`SCOPE TYPE NAME SECRET`のservice secret、後半は`CUSTOM SECRETS`の見出しに続く`SCOPE TARGETS ENV PLACEHOLDER SECRET`のcustom secret。読むのは後半の`TARGETS`と`ENV`だけとする。`TARGETS`は1列に複数hostを並べうるため、列の区切りは空白2つ以上とする。`PLACEHOLDER`と`SECRET`は読まない。1件もない場合は`No secrets found`で始まる文になる。`--service`は`SECRET`列へ値の一部を出すため使わない |
| secret登録（利用者が実行） | `sbx secret set-custom <name> --host <host> ... --env GH_TOKEN --value <token>` | Sandboxへplaceholderを渡し、proxyが登録済みhost宛のrequestで本物のtokenへ差し替える。開発中にtokenを提示する先はすべて`--host`を繰り返して1件のsecretへ載せる。結び付きはSandboxの作成時に決まる |
| login状態 | `sbx login status --json` | login済みかどうかを示す真偽値 |
| image存在確認 | `docker image ls --quiet <image>` | 出力が空かどうか |
| image検証 | `docker image inspect <image>` | `Id`と`Config.Labels` |
| archive生成 | `docker image save <image> --output <path>` | exit statusのみ |

`docker image inspect`はimageが存在しない場合もEngineへ問い合わせられない場合も非ゼロで終わるため、exit statusだけで不在と判定しない。存在の判定には`docker image ls --quiet <image>`を使い、この一覧が失敗した場合はimageを不在へ丸めずexit code `1`とする。

`sbx ls --json`はSandboxの由来Templateも接続中のsession数も示さない。示されない値を`Option`として持ち回ると、その不在をどう読むかを利用側がそれぞれ決めることになるため、`SandboxEntry`はこの2つを持たない。案件との対応は、canonical project IDから導出したSandbox名と、その案件だけが使う中立Workspaceの実pathで判定する。世代の一致はこの検査の保証範囲ではなく、どの検査でも観測しない。

runtimeのimage storeは、Templateの由来となったhost imageを示さない。一覧が持つ`id`はruntime内部の短縮idであり、`docker image inspect`の`Id`とは別のstoreの値である。Templateと世代の対応は、loadしたarchiveがlabelで宣言していた案件と世代と、`<image名>:<世代>`という名前で登録されたことの2つを根拠とする。

archiveの検証は、tarの`manifest.json`と、それが名前で指すimage configだけを読む。保存されたtagが期待するimage名と一致し、image configが期待するlabelをすべて宣言していることを条件とする。archive本体のlayerは読まない。

digestを対応の根拠にしない。`docker image inspect`の`Id`は、image storeとattestationの有無によって、image config、manifest、image indexのどれを指すかが変わる。対象Macでは、buildがprovenance attestationを伴うOCI image indexを作るため、`Id`はindexのdigestとなり、archiveが指すimage configのdigestとは一致しない。両者の一致を条件にすると、正常な成果物を毎回拒否する。


- parse不能な出力はmutationを行わずexit code `1`
- 未知のstate値を既知の値へ丸めない
- 最小versionの確認はPhase 1の実装を使う
- 対象CLIのversionが変わった場合は、daemon安全性probeを再実施する

## 4. 手動手順からの変更

MVPは既存の手動手順を次のように自動化・変更する。

- `.sbxm/create` shell scriptを生成せず、Rust workflowが同じ工程を実行する
- 単一の通常cloneではなく、Sandbox内にbare repositoryとmanaged worktreeを作る
- Sandbox名へcanonical project IDのhashを付け、owner/repository間の衝突を防ぐ
- `sbx ls`のtextへ`grep`せず、structured outputを完全一致でparseする
- sbxmはdaemonを停止も起動もしない。daemonを止めるには動作中のSandboxを止める必要があり、作業中のSandboxを巻き込むためである
- SSH Agentが渡っていないことは、daemonの起動条件から推定せず、作成したSandboxの中から`printenv SSH_AUTH_SOCK`と`ssh-add -L`で確認する
- 届いていた場合は工程を止め、動作中のSandboxを停止して`SSH_AUTH_SOCK`を外したshellからdaemonを起動し直す方法を案内する
- 中断時の目標構成をproject metadataへ保存し、以降は同じ`add`で継続する

中立Workspace、host path非露出、案件限定GitHub secret、利用者がglobal configへ明示したfileの限定copy、Docker socket非共有という要件は維持する。

## 5. Optionと目標構成

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

構築後に本数を増やすのは`apply --worktrees N`である。そちらは`start_ref`を保存済みの案件だけを対象とするため、起点branchを訊かない。

## 5.1 modeは最初のworktreeの作り方である

`provisioning.mode`は案件全体の宣言ではなく、**最初のworktreeをどう作るか**である。2本目以降はdetachedとして作る。Gitは同じbranchを2つのworktreeへcheckoutさせないため、attachedなworktreeは案件に1つしか持てない。

1本のattached案件に2本目を足すと、既にあるworktreeはbranchを持ったまま残り、足したものがdetachedになる。案件をdetachedへ移す必要はない。

**worktreeはmetadataへ記録しない。** 名前は`<repository>.tree-<index>`とindexから決まるため、どれが案件のworktreeかは`requested_worktrees`だけで分かる。modeとHEADは`git`が答える。metadataは利用者が要求した目標構成を持つ場所であり、観測できるものを控える場所ではない。

既にあるworktreeへ求めるのは、この共有repositoryのworktreeであり続けていることだけとする。起点commitもmodeも条件にしない。そこは利用者が作業する場所であり、commitすればHEADは動き、branchを切ればmodeも変わる。どちらもsbxmが作るときの事後条件であって、既にあるものへの要件ではない。

`protection`は保存状態を見るためにworktreeごとのmodeを観測しており、attachedにはupstreamとaheadを、detachedにはoriginからの到達性を求める。混在はこの検査の前提を変えない。

## 5.2 目標構成の再指定

再実行した`add`でoptionを省略した場合はmetadataに保存された目標構成を使用する。optionを指定した場合は保存値と完全一致することを要求し、不一致ならmutation前にexit code `1`とする。`add`は登録のcommandであり、登録済み案件の構成を変える手段ではない。

構築後にworktreeを増やすのは`apply --worktrees N`である。`N`が現在より多い場合は目標を引き上げて足りない分を作り、少ない場合はmutation前にexit code `1`とする。worktreeを減らすことはcheckoutされた作業を消すことであり、`destroy`と同じ重さの確認が要る。

## 6. Project単位の排他

既存projectではmutationの前に`<project-root>/.sbxm/project.lock`をexclusive lockする。新規projectではowner directory、project root、`.sbxm`を作成または検証した直後にproject lockを取得し、以後のmutationへ進む。

- lock待機は10秒
- timeoutは対象projectを表示してexit code `1`
- lockはworkflow終了まで保持する
- 複数lockが必要な将来機能ではcanonical ID昇順に取得する

lock fileの存在自体は処理中を意味しない。OS file lockの取得結果を使う。

`destroy`は管理解除の一部としてlock fileを削除するため、全commandは削除・再作成によるfile identityの変更を考慮してproject lockを取得する。

1. lock pathをsymlinkを追跡せずopenまたはcreateする
2. 開いたfileへexclusive lockを取得する
3. lock取得後、file descriptorと現在のlock pathをそれぞれ`fstat`相当と`lstat`相当で取得し、device IDとinodeが一致することを確認する
4. pathが存在しない、symlinkである、またはidentityが一致しない場合は、削除済みの古いlock fileを取得したものとして解放し、現在のpathで取得をやり直す
5. identityが一致した後にmetadata、filesystem、外部状態を再取得し、preconditionを判定する

retryを含む全体へ10秒のlock timeoutを適用する。permission、owner、file typeが不正な場合は安全に置換せずexit code `1`とする。

## 7. Project metadataの作成と構築継続

### 7.1 新規

入力を検証し、衝突検査を完了した後、次を行う。

1. owner directoryとproject root、`.sbxm`を作る
2. project lockを取得する
3. Dockerfileがなければbundled templateから作り、存在すれば利用者が管理する既存fileとして採用する
4. 採用するDockerfileのSHA-256を求める
5. 目標構成を含むmetadataをatomic writeする
6. 以後の外部mutationへ進む

`provisioning`には`mode`、`start_ref`、`requested_worktrees`、`dockerfile_sha256`を保存する。attached modeの`start_ref`は、remote default branchを解決するまで空を許可し、解決直後にatomic updateする。

### 7.2 `add`の再実行

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

## 8. 導出名

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

## 9. Workflow

全工程は`inspect -> decide -> mutate -> verify -> record`で実行する。verifyに失敗したら後続工程へ進まない。

`docker build`、`docker image save`、hostとSandbox内のGit clone・fetch、Template load、Sandbox createは本Phaseで追加する`passthrough`を使用し、各外部toolが出す進捗を実行中にそのまま表示する。sbxmは独自のprogress表示を重ねない。inspect、labelやarchiveの検証、secret存在確認など、結果をparseまたは秘匿するcommandは`capture`する。

### 9.1 Host directory

作成するdirectory:

```text
<project-root>/
├── <repository-lower>/
└── .sbxm/
    └── .cache/
```

- owner directory、project root、`.sbxm`、`.cache`はsymlinkを拒否
- 新規directoryのpermissionは利用者のumaskに従う。ただし`.sbxm`と`.cache`は`0700`
- 既存の非directoryはexit code `1`

### 9.2 Host clone

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

### 9.3 Dockerfile

`<project-root>/.sbxm/Dockerfile`がない場合だけbundled templateから`0600`で作る。既存Dockerfileは利用者が管理・編集するfileとして内容を変更せず採用する。内容は元の手動手順を初期templateの正本とし、少なくとも次を満たす。

- base imageはApple siliconを含む公式multi-platform image `docker.io/docker/sandbox-templates:shell-docker@sha256:39cf20eca861ec92747487af6197f6d916f774bdb98245d267dbd8dfd3debb05`へpinする。mutable tagだけを使用しない
- base imageが提供するUID `1000`の非root `agent`、`/home/agent`、passwordless sudo、inner Docker Engineを使用する
- MVPの固定tool setとして`git`、`openssh-client`、`coreutils`、`ca-certificates`、`curl`、`wget`、`gh`、`jq`をDockerfileで導入する。base imageに含まれることだけを導入済みの根拠にしない
- package installationを含む`docker build`の成功を固定tool set導入の判定とし、Sandbox操作のたびに全commandの存在をprobeしない
- `/home/agent/work`を`agent:agent`所有で作成
- secretの実値を書かない
- `GH_TOKEN`などへ実tokenを書かない。proxy-managed方式が対象`sbx` versionで必要な場合だけ、実機で確認した形式のsentinelを設定
- mise、Codex、Claude Codeのinstallerはversionまたはdigestをpinする
- interactive shellの開始位置を`/home/agent/work`にする
- `WORKDIR /home/agent/work`
- MVPではbuild context由来のfileを必要としないため、`COPY`と`ADD`を使用しない

このtool setはオーナーの実務環境として選択したMVPの初期templateであり、列挙した全toolがsbxm自身の直接依存であることを意味しない。利用者が生成後のDockerfileを直接編集し、`sbxm rebuild`でpackage、installer、versionの変更をSandboxへ適用することはMVP要件に含む。別途、これらを設定やoptionで選択・合成する機構はMVP対象外とし、Dockerfileと`rebuild`を唯一のcustomize経路とする。

`add`は採用したDockerfileのSHA-256をmetadataとimage labelへ適用済みhashとして保存する。利用者による手修正を許可し、構築完了後の保存済みhashとの不一致だけを理由にmetadataやDockerfileを不正とは扱わない。変更の適用は`rebuild`が担当する。

### 9.4 Image buildとarchive

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

MVPではbuild、save、loadに必要なhostまたはDocker storage容量を事前に見積もらず、容量不足を避けるための旧世代削除や自動再試行も行わない。必要容量はDockerfile、Docker内部storage、build cache、image、archiveの状態に依存し、根拠のある安全な削除規則をMVPでは定義できないためである。失敗時は`passthrough`と共通error規則に従い、外部toolの出力、失敗工程、対象、同じcommandによる再実行方法を表示する。

### 9.5 Template load

実機で確認した次の操作を使用する。

```text
sbx template load <template-archive>
```

load後、実機で確認したread-only commandにより、Templateが期待image IDと対応することを検証する。runtimeが対応関係を観測できない場合、既存Templateは再利用せず、同名存在時にexit code `1`とする。

### 9.6 Credential isolation

sbxmはdaemonを停止も起動もしない。daemonの再起動には、動作中の全Sandboxを止めることが必要であり、無関係な作業を巻き込む。

hostのSSH AgentがSandboxへ転送されるかどうかは、daemonの起動条件ではなくSandboxの作成時に決まる。したがって、daemonをどう起動したかからは推定せず、Sandboxを作成したあとにSandboxの中から確認する。

- `printenv SSH_AUTH_SOCK`が値を返さないこと
- `ssh-add -L`がagentへ到達しないこと

どちらかが到達を示した場合はexit code `1`とし、どのprobeが反応したかを示す。検査commandが答えを返せなかった場合は、露出していない側へ丸めずexit code `1`とする。

active sessionの検査は行わない。対象versionの`sbx ls --json`はsession数を示さず、示されない値から不在を推定しないためである。接続中の端末を保護する検査は存在せず、`rebuild`と`destroy`が守るのは保存されていない作業である。

#### Daemon安全性probe

Sandbox mutationを実機で成功扱いする前に、次を証明して結果をPRへ記録する。

1. `SSH_AUTH_SOCK`ありで起動したdaemonがSandboxへagentを転送すること
2. `SSH_AUTH_SOCK`をunsetして`sbx daemon start --detach`したdaemonでは転送されないこと
3. daemon停止・起動後にSandboxを再利用または作成できること

### 9.7 Sandbox create

中立Workspaceを`0700`で作り、symlinkを拒否する。sbxm独自のownership markerや作成履歴fileは置かない。既存workspaceはowner、permission、file type、real pathと内容がvalidation規則を満たす場合に、作成元を問わず再利用する。満たさない場合は内容を変更せずexit code `1`とする。

workspaceのrootは共有される一時領域の下にあり、rootを別accountが所有していると、その下のworkspaceを差し替えられる。rootにも同じ規則を適用し、`0700`、symlink拒否、所有者一致を満たす場合だけ検証または作成する。所有者の判定は、permissionではなく観測したowner IDと現在の実効user IDの一致で行う。

期待する外部command:

```text
sbx create
  --name <sandbox-name>
  --template <image-name>
  shell
  <workspace>
```

実際のargumentsは対象CLI versionの実出力に従う。runnerは`SSH_AUTH_SOCK`をunsetする。

既存Sandboxを再利用する条件:

- `sbx ls --json`のnameが完全一致
- workspaceのreal pathが期待する中立Workspace
- Template/image identityがinspect可能で一致
- metadataのcanonical IDと対応

1項目でも確認不能または不一致ならexit code `1`。誰が作成したか、またはsbxm独自のmarkerがあるかは条件にしない。

### 9.8 宣言fileの配置と`sync-files`

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

`sync-files`はproject metadata、Sandbox identity、running stateをread-onlyで検証してから、本sectionのfile配置だけを実行する。rebuild intentが存在する場合はfileを配置せず、同じtargetの`sbxm rebuild <owner>/<repository>`再実行を案内してexit code `1`とする。stopped Sandboxを暗黙に起動せず、`sbxm open <owner>/<repository>`後の再実行を案内してexit code `1`とする。registered、unmanaged、inconsistentでは何も配置しない。

### 9.9 Git identityとprotocol

Sandbox内で次を引数配列として実行する。

```text
git config --global user.name <config.git.user_name>
git config --global user.email <config.git.user_email>
gh config set git_protocol https --host github.com
```

既存値が同一ならskip。異なる場合は、別利用者のSandboxである可能性があるため自動上書きせずexit code `1`。

### 9.10 GitHub secret

案件限定personal access tokenの発行と入力は自動化しない。必要権限は対象repositoryのread/writeとし、fine-grainedなら`Contents: read/write`と`Metadata: read`、必要な場合だけPull requests、Issues、Actionsを追加する。classicなら`repo` scopeとする。

期待する利用者向けcommand:

```text
sbx secret set-custom <sandbox-name> \
  --host github.com \
  --host '**.github.com' \
  --host '**.githubusercontent.com' \
  --host ghcr.io \
  --env GH_TOKEN --value <token>
```

登録のないhostにはplaceholderがそのまま届き、tokenが正しくても`401`になる。`git`は`github.com`へ、`gh`は`api.github.com`へ話すため、前者だけの登録ではpushが通る一方で`gh`が全滅する。hostを複数のsecretへ分けることもできない。secretごとにplaceholderが分かれる一方、Sandboxの`GH_TOKEN`は1つの値しか持てないためである。

`github` service secretは使わない。proxyのgithub presetはtokenの形で扱いを変え、classic tokenを注入しない。実機では、同一のclassic tokenがSandboxの外から`200`、中から`401`を返した。custom secretはtokenの形を問わず、Sandboxにはplaceholderだけを見せる。

存在確認は実機で確認したread-only commandとstructured outputだけを使用する。secret値もplaceholderも取得・表示しない。

custom secretはSandboxの作成時に結び付く。したがって確認はSandboxを作る前、かつimageを組む前に行う。未登録なら、発行条件と上記commandを表示して前提条件不足のexit code `1`で停止する。登録後は同じ`prepare`を再実行し、次工程へ進む。

登録済みであることからSandboxへ届いたと推定しない。Sandbox作成後に環境変数`GH_TOKEN`を中から読み、空ならexit code `1`で停止し、`sbx rm <sandbox-name>`による作り直しを示す。

Sandbox内のgitには、placeholderをcredentialとして使わせる。`credential.https://github.com.helper`へ、usernameに任意の値、passwordに`$GH_TOKEN`を返すhelperを設定する。helperはtokenを持たず、変数名だけを持つ。

### 9.11 Bare clone

Sandbox内:

```text
mkdir -p <bare-root>
git init --bare <bare-git-dir>
git --git-dir <bare-git-dir> remote add origin
  https://github.com/<owner-display>/<repository-display>.git
git --git-dir <bare-git-dir> config
  remote.origin.fetch +refs/heads/*:refs/remotes/origin/*
git --git-dir <bare-git-dir> fetch --prune origin
```

`git clone --bare`は使わない。remoteのbranchをすべて`refs/heads/*`へ複製するため、attached modeが同じ名前でlocal branchを作ろうとした時点で`a branch named <branch> already exists`となる。空のbare repositoryへfetchすれば`refs/heads/*`は空のまま始まり、local branchはworktreeを作るときにだけ生まれる。bare repositoryの中のlocal branchがsbxmの作ったものだけになるため、managed worktreeの判定も素直になる。

再利用条件:

- `<bare-git-dir>`がbare repository
- originがcanonical IDと一致
- fetch refspecが完全一致
- `git fsck --connectivity-only`が成功

directoryは存在するが条件不一致なら自動削除せずexit code `1`。

### 9.12 Start ref解決

attached mode:

1. `git ls-remote --symref origin HEAD`相当からdefault branchを得る
2. `refs/heads/<branch>`だけを受け付ける
3. `refs/remotes/origin/<branch>`の存在を確認
4. metadataの空`start_ref`へbranch名をatomic write

detached modeでは`refs/remotes/origin/<start_ref>`の存在を確認する。ambiguous ref、tag、commit直接指定は拒否する。

### 9.13 Managed worktree

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

## 10. 成功出力

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

## 11. Errorと副作用

- 各工程失敗後は後続工程を実行しない
- 成功済み成果物をrollback目的で削除しない
- 失敗時は、完了工程、失敗工程、対象、同じ引数の`add`を次のcommandとして表示する
- 外部command失敗はprogram、safe args、cwd、exit status、stderr原文を表示する
- token、secret、host SSH情報は表示しない
- parse不能な外部出力を推測で成功扱いしない

## 12. 自動test

本Phaseで追加する共通基盤のtestは、それを使うworkflowのtestと同じPRへ入れる。

- canonical project IDによる比較とcase正規化
- Sandbox名の決定性、63 byte上限、slug衝突、name collision
- metadata探索のsymlink拒否、canonical ID重複、導出path不一致
- 既存fileのatomic置き換えのidentity検証とsymlink拒否
- option matrixとmutation前validation
- metadata新規作成、構築途中と構築済みでの`add`再実行
- project lock取得後のidentity一致、削除・再作成による不一致時retry、symlink・owner・permission拒否、retry timeout
- 各工程直後に失敗させた同じ`add`による継続
- 再実行optionの省略、一致、不一致
- host clone remote検証
- 新規・既存Dockerfileの採用、初回build前のhash更新、構築済みDockerfile変更時の`rebuild`案内
- 一時build contextのpermission、通常directory、空状態、project file非包含、成功・失敗時cleanup
- archive工程ごとの一律再生成、中断後の`.tmp`削除と再開、検証完了前の既存正式archive維持、atomic置換
- Sandbox名完全一致とworkspace検証
- 手作業で作成したvalidなmetadata、workspace、image、Sandbox、Git repository、worktreeの受け入れ
- 作成元にかかわらず同じ不整合を同じ診断とexit codeで拒否すること
- credential隔離の確認成功・確認不能・露出
- build、save、clone、fetch、Template load、Sandbox createのpassthroughと、structured出力のcapture
- 宣言fileのsource、destination、path逸脱、同一、`add`時の競合、`sync-files`時の上書き、宣言削除時の保持、一時file cleanup
- `sync-files`のrunning限定、stopped非起動、他工程への副作用なし
- rebuild intent中の`sync-files`拒否と同じ`rebuild`の案内
- secret不在による中断と登録後の`add`再実行
- bare clone、refspec、default branch
- attached 1 tree、detached 1/3 trees
- worktree作成途中のmetadata復元
- managed/unmanaged分離
- stderr、exit code、secret redaction

## 13. 実機受入条件

- daemon安全性probeの5項目を証明し、結果をPRへ記録している
- 呼び出し側のない型、policy、error ID、messageを追加していない
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
