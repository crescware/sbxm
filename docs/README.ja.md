# sbxm

`sbxm`は、GitHubプロジェクトごとに専用のDocker Sandboxと、構成の予測できる
Git worktree一式を用意します。ホスト側のclone、Sandbox image、repositoryのセットアップ、
日常的な接続、診断、再構築、破棄までを扱います。

Sandboxには、指定したGit identityと設定ファイルだけが渡されます。ホスト側の
プロジェクトディレクトリ、Docker socket、SSH agentは渡されません。GitHubの認証情報は
Sandboxへコピーせず、Docker Sandboxesのsecret proxyを通じて提供します。

English: [README.md](../README.md)

## 必要要件

- macOS 14以降を搭載したApple silicon Mac
- Docker Engineが起動しているDocker Desktop
- **[Docker Sandboxes CLI 0.42.1以降](https://docs.docker.com/ai/sandboxes/get-started/)**
- GitとSSH
- 管理対象のrepositoryごとに発行したGitHub personal access token

`sbxm status --global`を実行すると、これらの要件とDocker Sandboxes環境を
確認できます。

## インストール

```sh
brew install crescware/tap/sbxm
```

## クイックスタート

### 1. ホスト環境を検証する

sbxmが必要とするものが揃っているかを確認します。

```sh
sbxm status --global
```

Sandbox内で使うGitの名前とメールアドレスは、利用者自身の設定から読み取ります。
未設定であれば、先に宣言してください。

```sh
git config --global user.name "Your Name"
git config --global user.email "you@example.com"
```

### 2. プロジェクトを登録する

プロジェクトを置きたいディレクトリへ`cd`し、GitHubが表示するclone URLを
そのまま渡します。

```sh
cd ~/Projects
sbxm add git@github.com:<owner>/<repository>.git
```

```sh
sbxm add https://github.com/<owner>/<repository>.git
```

`sbxm add`が受理するのはこの2形式だけです。ホスト側のcloneは渡したtransportを
そのまま使います。

sbxmは、実行したディレクトリの直下に`<repository>.project/`を作ります。プロジェクト
ごとのディレクトリを用意したり、owner名を含む配置規則を揃えたりする必要はありません。
最初の対話実行では、表示言語を一度だけ選び、その結果を`~/.sbxm/config.yaml`へ
保存します。

同じ最初の`add`で、プロジェクトのcommitに使う名前とmail addressも訊きます。ホスト側の
`git config --global`の値が初期値として入力欄に置かれるため、そのままEnterを2回押せば
採用され、打ち直せば別の値になります。答えは`~/.sbxm/config.yaml`へ保存され、以降の
実行では訊きません。sbxmがホスト側のGit設定を勝手に答えとして採用することはありません。

プロジェクト側にも、登録時点の名義が別に書き込まれます。あとから既定を変えても、登録済み
プロジェクトは登録時の名義のままです。

特定のプロジェクトだけ別の名義にする場合や、答える端末がない環境で登録する場合は、
両方を宣言します。

```sh
sbxm add git@github.com:<owner>/<repository>.git \
  --git-user-name '<名前>' --git-user-email '<mail address>'
```

宣言はその実行にだけ効き、保存された既定を書き換えません。`--lang`が保存済みの言語を
書き換えないのと同じです。片方だけの指定は、何かを読む前にも作る前にも拒否します。
端末も、保存された既定も、宣言も無い実行は、推測せずに停止します。

このコマンドはプロジェクトを登録し、ホスト側のcloneとDockerfileを作成したうえで、
Sandbox名と次に実行する正確なコマンドを表示します。この時点ではまだSandboxを
構築しません。

デフォルトでは、repositoryのdefault branch上にworktreeを1つ作成します。独立した
worktreeを複数用意する場合は、起点となるbranchとdetached modeを指定します。

```sh
sbxm add git@github.com:<owner>/<repository>.git --detach main --worktrees 3
```

複数のagentやタスクで作業ディレクトリを分離したい場合に、detached worktreeが役立ちます。
指定できる個数は1〜32です。`--worktrees`は`-t`と短く書けます。

### 3. GitHubの認証情報を登録する

repositoryを読み書きできるpersonal access tokenを発行します。

- fine-grained tokenには**Contents: read and write**と**Metadata: read**が必要です。
- classic tokenには`repo` scopeが必要です。

`sbxm add`は、プロジェクト専用の`sbx secret set-custom`コマンドを表示します。
Sandboxを構築する前に、そのコマンドへtokenを渡して実行してください。
表示されるコマンドは次のような形です。

```sh
sbx secret set-custom <sandbox> \
  --host github.com \
  --host '**.github.com' \
  --host '**.githubusercontent.com' \
  --host ghcr.io \
  --env GH_TOKEN \
  --value <token>
```

secret proxyにより、本物のtokenはSandboxの外側に保たれます。Sandboxへtokenは入らず、
sbxmは中のgitへplaceholderを渡します。登録済みのhostへのrequestに限って、proxyが
placeholderを本物のtokenへ置き換えます。

Docker Sandboxesの組み込み`github` serviceではなくcustom secretを使うのは、
serviceのpresetがtokenの形で扱いを変え、classic personal access tokenを注入しない
ためです。custom secretなら、classicでもfine-grainedでも動作します。

この組み込みserviceは、tokenを1件も保存していなくても、各Sandboxの`GH_TOKEN`と
`GITHUB_TOKEN`を自身のsentinelで埋めます。sbxmはlogin shellが読むfileで両方を
placeholderへ上書きするため、Sandboxの中の`gh`もgitと同じproxy経由で認証できます。

### 4. Sandboxを構築して接続する

```sh
sbxm open <project-id>
```

最初の`open`は、プロジェクトのimageをbuildし、Sandboxを作成して、その中へrepositoryを
cloneし、managed worktreeを作成してからSSHで接続します。2回目以降は、必要に応じて
停止中のSandboxを起動して接続します。途中で中断した場合、次の`open`が完成済み成果物を
検証して再利用し、不足工程を完了してから作業を失わず接続します。

接続時の起点は`/home/agent/work/<repository>`です。managed worktreeを起点にする場合は
0始まりのindexを指定します。たとえば`sbxm open <project-id> -i 0`です。

対話端末でproject IDを省略すると、1つのpromptで上下キーから案件、左右キーから
0始まりのmanaged worktree indexを選び、Enter 1回で両方を確定します。認証を確認したら、
projectのmetadataを待たずにpromptを開きます。結果が届くまでのindex行は`(計算中)`と述べるだけで、
まだ分からない範囲を数として示しません。そのあいだもindexは動かせます。metadataは裏で計算し、
選択中の案件の結果が届いたらその案件自身の範囲を表示して、indexをその中に収めます。
確定時にもproject lockのmetadataで再確認し、下げた場合は接続前に警告します。

Sandbox内のworktreeは次の場所にあります。

```text
/home/agent/work/<repository>/<repository>.tree-1
/home/agent/work/<repository>/<repository>.tree-2
...
```

## 日常的な操作

`open`、`apply`、`repair`、`rebuild`、`stop`、`destroy`、`ls`、`guide`と、案件指定または
対話実行の`status`は、案件選択前にDocker Sandboxesの認証を確認します。未loginなら
`sbx-login-missing`と`sbx login`の案内を表示して終了します。`status --global`は
未loginでもほかのhost要件と合わせて診断でき、`add`はDocker Sandboxesへのloginを必要としません。

```sh
# 管理対象の全プロジェクトとSandboxの状態を表示する
sbxm ls

# 変更を加えずに1つのプロジェクトを検査する
sbxm status <project-id>

# プロジェクトへ接続する
sbxm open <project-id>

# 1つ以上のプロジェクトを削除せずに停止する
sbxm stop <project-id>
sbxm stop <project-id> ...
```

### 保守手順を案内してもらう

`guide`は、利用者が行いたいことを起点に対象案件を選び、その現在状態から次の手順を
組み立てます。GitHub tokenの期限切れが近い場合や交換済みの場合は、credential交換の
guideを使います。

```sh
# guideのtopicを選び、続いて案件を選ぶ
sbxm guide

# 案件だけを選ぶ
sbxm guide credential-rotation

# 1案件についてすぐ案内を始める
sbxm guide credential-rotation <project-id>
```

credential交換guideは、現在の登録から公開情報であるscopeとplaceholderだけを読み、
同じ登録を更新する`sbx secret set-custom`コマンドを表示します。古いtokenも交換後の
tokenも`sbxm`へは渡しません。表示された`sbx`コマンドの`<token>`だけを置き換えて
ください。placeholderを維持するため、Sandboxの再構築は不要です。

`STATE`は`sbxm open`がすぐ接続できるかを示します。`stopped`は開くと
Sandboxが起動し、`open-blocked`は接続前の準備が残っています。初回構築が始まったまま
完了していない案件もここに含まれます。理由は`sbxm status <project-id>`で確認でき、
次に実行するcommandを1つだけ示します。中断・欠落した接続準備は`sbxm open`、
世代交代は`sbxm rebuild`です。

対話端末で実行した場合、`repair`、`apply`、`rebuild`、`open`、`stop`、`destroy`、
`status`はプロジェクト引数を省略すると対象を選択するpromptを表示できます。
`status`では先頭に`global`を表示し、その後へ登録済みproject IDを並べます。
`guide`はtopicを省略すると最初にtopicを問い、続いて案件を問います。
`credential-rotation`を指定した場合は案件選択から始めます。非対話端末では、これらの
commandにプロジェクト引数を明示してください。`guide`にはtopicの明示も必要で、
`status`だけはproject IDまたは`--global`をscopeとして指定できます。

ただし`rebuild`と`destroy`は、引数を明示しても非対話端末では実行できません。どちらも
削除計画を表示し、対象Sandbox名の完全一致入力を得た場合にだけ進むためです。`destroy`
には確認を省略する`--force`がありますが、`rebuild`にはありません。

## プロジェクトをカスタマイズする

### Sandbox imageを編集する

`sbxm add`はプロジェクトのホスト側ディレクトリにDockerfileを作成します。ツールや
system dependencyを追加するにはこのファイルを編集し、変更を適用します。

```sh
sbxm rebuild <project-id>
```

rebuildはDockerfileの変更有無にかかわらずSandboxを作り直し、元のSandboxの書き込み可能な
層を失わせます。作業内容を保護するため、dirty file、publishしていないcommit、進行中のGit
操作、またはunmanaged worktreeがある場合、sbxmは通常のrebuildを拒否します。

cleanなworktreeだけでは見えないrepository単位の状態も検査します。checkoutしていない
local branch、tag、note、stash、追加remote、reflogだけに残るcommitも対象です。表示された
Layer Aのblockerを保存または解決してから、もう一度実行してください。originに無いcommitは、
`sbxm fetch`でホスト側のrepositoryへ保存しても構いません。`status`ではworktreeの
`STATE`とoriginからの回収根拠を、別の`REMOTE`列に表示します。

拒否しない場合も、作り直しで何が失われるかを先に表示します。無視対象のpath、
checkoutしていないbranchやtagの名前、追加remote、reflogにだけ残るcommit、Sandboxの
書き込み層が対象です。表示のあと、対象Sandbox名の完全一致入力を得た場合にだけ進みます。
`rebuild`に確認を省略する方法はないため、非対話端末では実行できません。表示した状態が
入力から実際の作り直しまでのあいだに変わった場合は、何も削除せずに中止します。

### Sandboxのroot sizeを選ぶ

Docker Sandboxesは、Sandboxを作成するprocessのenvironmentから`DOCKER_SANDBOXES_ROOT_SIZE`
を読み取ります。sbxmはこの変数を解釈も書き換えもせず、`sbx create`を実行する時点で
設定されている値をそのまま渡します。

```sh
# 初回作成
DOCKER_SANDBOXES_ROOT_SIZE=40g sbxm open <project-id>

# 既存Sandboxの作り直し
DOCKER_SANDBOXES_ROOT_SIZE=40g sbxm rebuild <project-id>
```

この設定が*しないこと*もいくつかあります。

- この変数は、これから作られるSandboxにだけ効きます。既存Sandboxのfilesystemを
  in-place resizeするものではないため、sizeを変えるにはSandboxの作り直しが必要です。
  `rebuild`はこの場合も他のrebuildと同じdata保護検査を通ります。
- requested sizeは作成時にその場で予約される量ではありません。各Sandboxが書き込める
  上限を引き上げるだけで、host上の複数Sandboxの実使用量は引き続きhostの実容量に対して
  合算されます。
- build済みimageとload済みtemplateは各Sandboxのroot filesystemとは別にhost容量を
  消費し、この設定とは独立しています。sbxmがその間に書き出すarchiveはloadが終われば
  消える短命fileであり、積み上がりません。

どちらのcommandを実行する前にも、要求するsizeに対してhostに十分な空き容量があるか
確認してください。

### Sandboxのディスクを何が埋めるかを理解する

`sbxm status <project-id>`は、Sandboxの現在の空き容量・実効天井・使用率を示すDISK
sectionを表示します。この数値が何を反映しているかを理解するための事実です。

- `/home`、`/tmp`を含むSandbox内のすべては、1枚のroot filesystemを共有します。上の
  `DOCKER_SANDBOXES_ROOT_SIZE`でsizeを決めるのと同じfilesystemであり、build成果物や
  一時fileのための別volumeはありません。
- `/tmp`はSandboxを停止して再度開いて（`sbxm open`）も消えません。中でperiodicな
  掃除を行うinit systemが無いため、置いたfileはSandbox自体を破棄または作り直すまで
  残り続けます。
- Sandbox内でfileを削除すると、その場で空き容量が戻ります。root filesystemは通常の
  書き込み可能な層であり、作り直したときだけ空きが戻るsnapshotではありません。
- managed worktreeはそれぞれ独立してbuildするため、`--worktrees N`はbuild成果物
  （例えばRustの`target/`）をworktree数だけ増やします。
- 共有build cache directoryをサポートする言語・toolであれば（例えばRustの
  `CARGO_TARGET_DIR`）、下の「設定ファイルを配置する」で宣言するfile経由で全worktree
  を同じdirectoryへ向けられ、この増加を避けられます。ただし1つのdirectoryを共有すると、
  本来は並行にできるworktreeごとのbuildが直列化されるため、既定にはせず明示的な
  trade-offとして選んでください。

diskの復旧が必要なときは、次の順序で進めてください。

1. Sandbox内の不要なfileを削除する。空き容量はその場で戻ります。
2. rebuildで何を破棄するかを確認する。書き込み可能な層の作り直しが必要なら、
   通常の保護付き`rebuild`を実行し、表示されたplanを確認する。
3. 保護検査が示すLayer Aのblockerをすべて保存または解決してから再実行する。publishしていない
   commit、dirtyまたは未追跡の作業、進行中のGit操作、active session、repository単位のrefも
   含まれます。

### managed worktreeを追加する

構築済みのプロジェクトには、rebuildせずにmanaged worktreeを追加できます。

```sh
sbxm apply <project-id> --worktrees 4
```

worktree数は増やすことだけができます。デフォルトのattached modeで登録したプロジェクト
では、最初のworktreeはtracking branch上に残り、追加のworktreeはdetachedになります。
ここでも`--worktrees`は`-t`と短く書けます。

### 設定ファイルを配置する

すべてのSandboxへ配置するホスト側のファイルを宣言します。

```sh
sbxm files add ~/.claude/CLAUDE.md
```

配置先を省略すると、home directoryからの相対pathをそのまま使います。この例では
Sandboxのhomeの`.claude/CLAUDE.md`へ配置します。別の場所へ置く場合は`--dest`を
指定します。sbxmは、通常のファイルでサイズの上限内であることを確かめ、認証情報に
よく使われる名前には警告を出します。対話端末では、続けて登録済みのすべての案件へ
今すぐ配置するかを訊きます。`sbxm files ls`で宣言を一覧し、`sbxm files rm <配置先>`で
宣言を外します。Sandboxへ配置済みのファイルはそのまま残ります。

宣言は`~/.sbxm/config.yaml`に保存され、手で編集することもできます。sbxmはこのファイルを
編集するとき、コメントや書き方をそのまま残します。

```yaml
version: 1

files:
  - source: /Users/you/.gitconfig
    destination: .gitconfig

  - source: /Users/you/.config/another-tool/settings.yaml
    destination: .config/another-tool/settings.yaml
```

配置先はSandbox userのhome directoryからの相対pathです。宣言したファイルは
Sandboxの初回構築時に配置されます。あとから加えた変更は明示的に適用します。

```sh
sbxm apply <project-id> --files
```

`--files`が置き換えるのは、sbxmが前回配置した内容のままの配置先だけです。Sandboxの
中で編集されたファイルや、sbxmが配置した記録のないファイルがあれば、`apply`は1件も
配置する前に拒否し、該当するファイルをすべて示します。Sandbox側の内容で必要なものを
退避してから、明示的に置き換えてください。

```sh
sbxm apply <project-id> --files --force
```

宣言したファイルをSandboxの中で編集しても、`open`と`repair`はその案件を拒否せず、
編集した内容を宣言ファイルで置き戻すこともしません。

`sbxm status <project-id>`は宣言ファイルごとに、sbxmが配置したあとにホスト側の
ファイルが変わったか（`updated`）と、Sandbox側で編集されたか（`modified`）を示します。

Sandboxの中で編集した内容をホスト側のファイルへ持ち帰るには、次を実行します。

```sh
sbxm files pull .claude/CLAUDE.md <project-id>
```

sbxmはSandbox側のファイルを案件の`.sbxm/incoming`へ取り出し、ホスト側のファイルとの
差分を示してから、採用するかを訊きます。自動では混ぜず、対話端末でない実行では差分を
示すだけです。Sandboxから来た制御文字は端末へ届けず、`\u{...}`の形で示します。

登録済みのすべての案件へまとめて配置する場合は、次を実行します。

```sh
sbxm apply --files --all
```

案件ごとにlockを取り、同じ規則で1件ずつ配置します。1件を配置できなくても、ほかの案件は
続けます。停止中のSandboxは起動せず、Sandboxのない案件には初回構築で宣言が配置されます。
どちらも結果に示します。配置できなかった案件が1件でもあれば、終了statusは`1`です。

token、private keyなどの認証情報はこれらのファイルに含めず、Docker Sandboxesの
secretを使用してください。

2つのapply対象は同時に指定できます。

```sh
sbxm apply <project-id> --files --worktrees 4
```

### Sandboxのcommitをホストへ保存する

案件のSandboxのcommitを、ホスト側のrepositoryへ写します。

```sh
sbxm fetch <project-id>
```

sbxmはSandboxのbranch、tag、各worktreeの`HEAD`を`git bundle`にして`sbx exec`越しに
取り出し、案件の`.sbxm/bundles`へ置いて`git bundle verify`で確かめます。そのうえで
objectの検査を有効にして、ホスト側のrepositoryの`refs/sbx/<sandbox>/`へだけ取り込みます。
ホスト側のbranchとtagには触れません。Sandboxで書き換えたbranchや消したbranchの前の先端は
`refs/sbx/<sandbox>/archive/<時刻>/`へ退避し、自動では消さないため、一度取り込んだ
commitはどれも辿れ続けます。Sandboxは動いている必要があり、停止中のSandboxは起動しません。

こうして保存したcommitはSandboxを消しても残るため、rebuildとdestroyはoriginから辿れる
commitと同じく、失われないものとして数えます。originに無いcommitだけを理由にrebuildや
destroyが拒否された場合、対話端末ではホスト側のrepositoryへ保存してから続けるかを訊きます。
止める選択では何も変えません。

## GitHubを使わずに開発する

このhostにすでにあるrepositoryも、プロジェクトにできます。sbxmはcloneしません。
そのrepositoryの`.git`がoriginの役を持ち、Sandboxは作業のための使い捨ての場所になります。

```sh
cd ~/Projects
sbxm add --local ~/code/<repository>
sbxm open local/<repository>
```

pathはGit working treeの最上位でなければなりません。プロジェクトIDは`local/<name>`で、
名前は`--name <name>`を渡さなければdirectory名です。Sandboxは、hostのrepositoryが今いる
branch、または`--detach`で渡したbranchから始まります。

GitHub tokenは関わらないため、登録の手順は要りません。`open`がSandboxを構築するとき、
sbxmはhostのrepositoryのbranchとtagを1つの`git bundle`にし、`sbx exec`越しにSandboxへ
送り、Sandboxの`origin`をそのbundleへ向けます。Sandboxの中からhostのrepositoryへ届く
経路はありません。

作業は`sbxm fetch local/<name>`で持ち帰ります。commitはhostのrepositoryの
`refs/sbx/<sandbox>/`へ入るので、たとえば`git merge refs/sbx/<sandbox>/heads/main`で
自分のbranchへ取り込みます。

そのあとhostのrepositoryに増えたものは`sbxm send local/<name>`で送ります。sbxmは新しい
bundleを送ってSandboxの中で`git fetch --prune origin`を行うので、そこでの`origin/<branch>`が
hostと揃います。Sandboxのworktreeとbranchはそのままです。取り込むときは、Sandboxの中で
`origin/<branch>`をmergeするかrebaseしてください。

Sandboxの中のbundleはSandboxと一緒に消えるため、rebuildとdestroyは、hostのrepositoryから
辿れるcommitだけを失われないものとして数えます。hostのbranchやtag、または`sbxm fetch`が
`refs/sbx/<sandbox>/`へ保存したものから辿れる必要があります。hostに無いcommitがあれば止まり、
対話端末では先にfetchするかを訊きます。

失われうるのは最後のfetchのあとにcommitした作業だけなので、sbxmは自分でもfetchします。
`stop`、`rebuild`、`destroy`（`--force`を含む）がSandboxを止める前、`open`のsessionが
つながっているあいだの10分ごと、そしてsessionを閉じたあとです。このために停止中のSandboxを起動することはありません。fetchに失敗しても
操作は続き、Sandboxにだけ残っているものがあることをwarningで伝えます。

`sbxm rebuild local/<name>`は、hostのrepositoryからSandboxを作り直し、
`refs/sbx/<sandbox>/heads/`へ保存したbranchをSandboxのbranchとして戻します。hostに
同じ名前のbranchがあれば、`origin/<branch>`をupstreamにします。起点branchのworktreeは
保存した先端から作り直すので、fetchしたところから作業を続けられます。Sandboxがsbxmの
外で消えた場合のように、`open`や`repair`が新しいSandboxを作るときも同じです。

## プロジェクトを破棄する

```sh
sbxm destroy <project-id>
```

sbxmは何かを削除する前に、削除するものと残すものを表示します。通常のdestroyでは、
dirty worktree、publishしていないcommit、repository単位のref、active sbxm sessionを
検査します。停止中のSandboxは、中を検査するために起動し、その事実を計画のそばに表示します。
この起動はほかの準備を行わず、確認をcancelしてもSandboxは起動したまま残ります。対話端末では、
続いて案件の登録ID（command lineへ渡す`<owner>/<repository>`）の入力を求め、一致しない場合は
入力し直せます。Sandbox自体の削除では、Docker Sandboxes自身のruntimeが行うactive-session
検査（sbxmが開始していないsession）も尊重します。この確認はsbxmが内部で答えるため、利用者に
二重には尋ねません。通常のdestroyを非対話端末で実行した場合は、確認を省略せず拒否します。

Sandbox、sbxmのプロジェクトmetadata、そのSandbox向けに登録した`GH_TOKEN`のcustom
secretは削除されます。登録が残ると、同じプロジェクトに対する次の`sbx secret set-custom`
が重複として失敗し、存在しないSandbox宛のtokenを預けたままになります。ホスト側のclone、
プロジェクトのDockerfile、build済みimage、load済みtemplate、それ以外を対象に登録した
secretは残るため、tokenを再登録すればあとからプロジェクトを再登録できます。

データ保護・active session・runtimeのin-use検査、および確認promptを意図的に省略する
必要がある場合は、次を実行します。

```sh
sbxm destroy --force <project-id>
```

Sandbox内に残すべきものがないと別途確認できた場合に限って、`--force`を使用してください。

## sbxmが置くもの

プロジェクトは、登録したディレクトリの中で完結します。

```text
<親ディレクトリ>/<repository>.project/
├── <repository>/       # ホスト側のclone
└── .sbxm/              # metadata、Dockerfile、lock、cache
```

`--local`で追加したプロジェクトは、このディレクトリの中にホスト側のcloneを持ちません。
repositoryは追加したときの場所に残ります。

`~/.sbxm`には、登録済みプロジェクトとその場所の索引である`registry.yaml`を置きます。
表示言語か名義を選ぶか、配置するファイルを宣言した時点で`config.yaml`も作られます。
プロジェクトの場所を知っているのはregistryだけであるため、プロジェクトのディレクトリを
移動すると、sbxmは新しい場所を推測せず`ls`で`missing`として表示します。

## コマンド一覧

| コマンド | 用途 |
|---|---|
| `sbxm add <github-clone-url>` | GitHub repositoryをsbxmへ追加し、このhostへcloneする |
| `sbxm add --local <path> [--name <name>]` | このhostにあるGit repositoryを、cloneせずに`local/<name>`として追加する。その`.git`がoriginの役を持つ |
| `sbxm open [<project-id>] [--index N]` | SandboxへのSSH接続を開く。初回はSandboxを構築し、以降は必要なら先に起動する。`N`は0始まりのmanaged worktree index |
| `sbxm stop [<project-id> ...]` | 1件以上の案件のSandboxを、削除せず停止する |
| `sbxm ls` | 管理案件と管理外Sandboxを、その状態とともに一覧する |
| `sbxm guide` | 目的と案件を対話選択し、credentialを受け取らず案件も変更せず、現在状態に応じた次の手順を示す |
| `sbxm guide credential-rotation [<project-id>]` | 選んだ案件のscopeとplaceholderを維持してGitHub credentialを交換する手順を示す |
| `sbxm status` | 対話端末でhostまたは案件を選択して診断する。`global`を先頭にpromptを表示する |
| `sbxm status --global` | hostの状態を変更せずに診断する |
| `sbxm status <project-id>` | 案件の状態を変更せずに診断する |
| `sbxm fetch [<project-id>]` | 案件のSandboxのcommitを、ホスト側のrepositoryの`refs/sbx/<sandbox>/`へ保存する。branchには触れない |
| `sbxm send [<project-id>]` | `--local`で追加した案件のSandboxへ、ホスト側のrepositoryのbranchとtagを送る。Sandboxのbranchには触れない |
| `sbxm files add\|ls\|rm ...` | すべてのSandboxへ配置するホスト側のファイルを宣言、一覧、または宣言を外す |
| `sbxm apply [<project-id>] ...` | 宣言済みファイルを配置するか、managed worktreeを追加する。`--files --all`で登録済みのすべての案件へ配置する |
| `sbxm repair [<project-id>]` | SSH接続を開かず、中断または未完成の案件を明示的に準備する。接続時は`open`が同じ復旧を行う |
| `sbxm rebuild [<project-id>]` | Dockerfileから案件のSandboxを作り直す（元の書き込み可能な層は失われる） |
| `sbxm destroy [<project-id>]` | Sandboxを破棄して案件を管理対象から外し、host cloneとDockerfileは残す |

完全なCLI referenceは、`sbxm --help`または`sbxm <command> --help`で確認できます。

## 出力

sbxmは結果を標準出力へ、進捗、prompt、警告、errorを標準エラー出力へ書きます。結果を
リダイレクトしても、結果以外は混ざりません。

色はstreamごとに判定します。標準出力だけをpipeした場合、結果はplain textになり、端末に
残る診断は色付きのままです。色は固定値ではなく端末themeが定義するANSIの標準色を使うため、
利用者が選んだcontrastをそのまま尊重します。

| 設定 | 効果 |
|---|---|
| `--color=auto` | streamが端末のときだけ色を付ける（既定） |
| `--color=always` | リダイレクト先にも色を付ける |
| `--color=never` | 色を付けない |
| `NO_COLOR` | 値にかかわらず、空文字でも色を無効にする |
| `CLICOLOR_FORCE` | `0` 以外なら色を有効にする |
| `TERM=dumb` | 色を無効にし、markerをASCIIへ切り替える |

明示した `--color` は、どの環境変数よりも優先されます。色を消しても情報は失われません。
marker、label、空行だけで同じ意味を読み取れます。
