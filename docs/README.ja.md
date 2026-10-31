# sbxm

`sbxm`は、GitHubプロジェクトごとに専用のDocker Sandboxと、構成の予測できるGit worktree一式を用意します。ホスト側のclone、Sandbox image、repositoryのセットアップ、日常的な接続、診断、再構築、破棄までを扱います。

Sandboxには、指定したGit identityと設定ファイルだけが渡されます。ホスト側のプロジェクトディレクトリ、Docker socket、SSH agentは渡されません。GitHubの認証情報はSandboxへコピーせず、Docker Sandboxesのsecret proxyを通じて提供します。

English: [README.md](../README.md)

## 必要要件

- macOS 14以降を搭載したApple silicon Mac
- Docker Engineが起動しているDocker Desktop
- **[Docker Sandboxes CLI 0.48.0以降](https://docs.docker.com/ai/sandboxes/get-started/)**
- Git 2.36以降と、SSH
- 管理対象のrepositoryごとに発行したGitHub personal access token

`sbxm status --global`を実行すると、これらの要件とDocker Sandboxes環境を確認できます。Docker Sandboxesを使うほかのcommandも、最初にCLIのversionを確認し、上記より古ければほかの処理を行わずに停止します。hostのGitを使うcommandも、同じようにGitのversionを確認します。一度確認が通った実行ファイルは`~/.sbxm/host-tools.yaml`に記録し、その実行ファイルが変わらない限り確認を省略します。実行ファイルが変われば確認し直します。

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

Sandbox内で使うGitの名前とメールアドレスは、利用者自身の設定から読み取ります。未設定であれば、先に宣言してください。

```sh
git config --global user.name "Your Name"
git config --global user.email "you@example.com"
```

### 2. プロジェクトを登録する

プロジェクトを置きたいディレクトリへ`cd`し、GitHubが表示するclone URLをそのまま渡します。

```sh
cd ~/Projects
sbxm add git@github.com:<owner>/<repository>.git
```

```sh
sbxm add https://github.com/<owner>/<repository>.git
```

`sbxm add`が受理するのはこの2形式だけです。ホスト側のcloneは渡したtransportをそのまま使います。

sbxmは、実行したディレクトリの直下に`<repository>.project/`を作ります。プロジェクトごとのディレクトリを用意したり、owner名を含む配置規則を揃えたりする必要はありません。最初の対話実行では、表示言語を一度だけ選び、その結果を`~/.sbxm/config.yaml`へ保存します。

同じ最初の`add`で、プロジェクトのcommitに使う名前とmail addressも訊きます。ホスト側の`git config --global`の値が初期値として入力欄に置かれるため、そのままEnterを2回押せば採用され、打ち直せば別の値になります。答えは`~/.sbxm/config.yaml`へ保存され、以降の実行では訊きません。sbxmがホスト側のGit設定を勝手に答えとして採用することはありません。

プロジェクト側にも、登録時点の名義が別に書き込まれます。あとから既定を変えても、登録済みプロジェクトは登録時の名義のままです。

特定のプロジェクトだけ別の名義にする場合や、答える端末がない環境で登録する場合は、両方を宣言します。

```sh
sbxm add git@github.com:<owner>/<repository>.git \
  --git-user-name '<名前>' --git-user-email '<mail address>'
```

宣言はその実行にだけ効き、保存された既定を書き換えません。`--lang`が保存済みの言語を書き換えないのと同じです。片方だけの指定は、何かを読む前にも作る前にも拒否します。端末も、保存された既定も、宣言も無い実行は、推測せずに停止します。

このコマンドはプロジェクトを登録し、ホスト側のcloneとDockerfileを作成したうえで、Sandbox名と次に実行する正確なコマンドを表示します。この時点ではまだSandboxを構築しません。

デフォルトでは、repositoryのdefault branch上にworktreeを1つ作成します。独立したworktreeを複数用意する場合は、起点となるbranchとdetached modeを指定します。

```sh
sbxm add git@github.com:<owner>/<repository>.git --detach main --worktrees 3
```

複数のagentやタスクで作業ディレクトリを分離したい場合に、detached worktreeが役立ちます。指定できる個数は1〜32です。`--worktrees`は`-t`と短く書けます。

### 3. GitHubの認証情報を登録する

repositoryを読み書きできるpersonal access tokenを発行します。

- fine-grained tokenには**Contents: read and write**と**Metadata: read**が必要です。
- classic tokenには`repo` scopeが必要です。

`sbxm add`は、プロジェクト専用の`sbx secret set-custom`コマンドを表示します。Sandboxを構築する前に、そのコマンドへtokenを渡して実行してください。表示されるコマンドは次のような形です。

```sh
sbx secret set-custom --sandbox <sandbox> \
  --host github.com \
  --host '**.github.com' \
  --host '**.githubusercontent.com' \
  --host ghcr.io \
  --env GH_TOKEN \
  --value <token>
```

secret proxyにより、本物のtokenはSandboxの外側に保たれます。Sandboxへtokenは入らず、sbxmは中のgitへplaceholderを渡します。登録済みのhostへのrequestに限って、proxyがplaceholderを本物のtokenへ置き換えます。

Docker Sandboxesの組み込み`github` serviceではなくcustom secretを使うのは、serviceのpresetがtokenの形で扱いを変え、classic personal access tokenを注入しないためです。custom secretなら、classicでもfine-grainedでも動作します。

この組み込みserviceは、tokenを1件も保存していなくても、各Sandboxの`GH_TOKEN`と`GITHUB_TOKEN`を自身のsentinelで埋めます。sbxmはlogin shellが読むfileで両方をplaceholderへ上書きするため、Sandboxの中の`gh`もgitと同じproxy経由で認証できます。

### 4. Sandboxを構築して接続する

```sh
sbxm open <project-id>
```

最初の`open`は、プロジェクトのimageをbuildし、Sandboxを作成して、その中へrepositoryをcloneし、managed worktreeを作成してからSSHで接続します。2回目以降は、必要に応じて停止中のSandboxを起動して接続します。途中で中断した場合、次の`open`が完成済み成果物を検証して再利用し、不足工程を完了してから作業を失わず接続します。

接続時の起点は`/home/agent/work/<repository>`です。managed worktreeを起点にする場合は0始まりのindexを指定します。たとえば`sbxm open <project-id> -i 0`です。

対話端末でproject IDを省略すると、1つのpromptで上下キーから案件、左右キーから0始まりのmanaged worktree indexを選び、Enter 1回で両方を確定します。認証を確認したら、各案件のmetadataを読んでからpromptを開きます。index行は選択中の案件自身の範囲を示します。metadataを読めない案件はindex行でそう述べ、indexは0から動かず、選ぶとその理由を報告します。確定した値はproject lockの下で再確認し、その案件がそのmanaged worktreeを宣言していなければ、`--index`と同じく警告してrepository rootを開きます。

Sandbox内のworktreeは次の場所にあります。

```text
/home/agent/work/<repository>/<repository>.tree-1
/home/agent/work/<repository>/<repository>.tree-2
...
```

## 日常的な操作

`open`、`apply`、`repair`、`rebuild`、`stop`、`destroy`、`ls`、`guide`と、案件指定または対話実行の`status`は、案件選択前にDocker Sandboxesの認証を確認します。未loginなら`sbx-login-missing`と`sbx login`の案内を表示して終了します。`status --global`は未loginでもほかのhost要件と合わせて診断でき、`add`はDocker Sandboxesへのloginを必要としません。

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

`guide`は、利用者が行いたいことを起点に対象案件を選び、その現在状態から次の手順を組み立てます。GitHub tokenの期限切れが近い場合や交換済みの場合は、credential交換のguideを使います。

```sh
# guideのtopicを選び、続いて案件を選ぶ
sbxm guide

# 案件だけを選ぶ
sbxm guide credential-rotation

# 1案件についてすぐ案内を始める
sbxm guide credential-rotation <project-id>
```

credential交換guideは、現在の登録から公開情報であるscopeとplaceholderだけを読み、同じ登録を更新する`sbx secret set-custom`コマンドを表示します。古いtokenも交換後のtokenも`sbxm`へは渡しません。表示された`sbx`コマンドの`<token>`だけを置き換えてください。placeholderを維持するため、Sandboxの再構築は不要です。

`STATE`は`sbxm open`がすぐ接続できるかを示します。`stopped`は開くとSandboxが起動し、`open-blocked`は接続前の準備が残っています。初回構築が始まったまま完了していない案件もここに含まれます。理由は`sbxm status <project-id>`で確認でき、次に実行するcommandを1つだけ示します。中断・欠落した接続準備は`sbxm open`、世代交代は`sbxm rebuild`です。

対話端末で実行した場合、`repair`、`apply`、`rebuild`、`open`、`stop`、`destroy`、`status`はプロジェクト引数を省略すると対象を選択するpromptを表示できます。`status`では先頭に`global`を表示し、その後へ登録済みproject IDを並べます。検査の進捗は全commandで同じ方針に揃え、処理を待つ前に工程名をstderrへ表示して、直ちにflushします。認証、案件lock、Docker・Sandboxの応答待ちのほか、`status`、`open`、`repair`、`apply`（`--all`を含む）、`rebuild`のrepository・worktree・file検査が対象です。worktreeとfileは対象名と全体の何件目かも示します。`destroy`、`sync`、`stop`、`files pull`、`ls`、`guide`の検査や、`add --local`のhost repository確認にも同じ工程表示を使います。`status --global`はDocker、CLI version、network policy、daemon、認証、Remote SSHの確認を分けて表示します。削除保護では無視対象file、進行中のGit操作、originのfetch、確認後の安全性の再検査も表示します。読み取り専用の`status`は保存済みのorigin refだけを読み、fetchは行いません。停止中のSandboxも内部やディスクの確認のために起動しません。診断結果や削除計画は、検査が終わったあとにstdoutへ表示します。外部commandの逐次ログは出しません。`guide`はtopicを省略すると最初にtopicを問い、続いて案件を問います。`credential-rotation`を指定した場合は案件選択から始めます。非対話端末では、これらのcommandにプロジェクト引数を明示してください。`guide`にはtopicの明示も必要で、`status`だけはproject IDまたは`--global`をscopeとして指定できます。

ただし`rebuild`と`destroy`は、引数を明示しても非対話端末では実行できません。どちらも削除計画を表示し、対象Sandbox名の完全一致入力を得た場合にだけ進むためです。`destroy`には確認を省略する`--force`がありますが、`rebuild`にはありません。

## プロジェクトをカスタマイズする

### Sandbox imageを編集する

`sbxm add`はプロジェクトのホスト側ディレクトリにDockerfileを作成します。ツールやsystem dependencyを追加するにはこのファイルを編集し、変更を適用します。

```sh
sbxm rebuild <project-id>
```

rebuildはDockerfileの変更有無にかかわらずSandboxを作り直し、元のSandboxの書き込み可能な層を失わせます。作業内容を保護するため、dirty file、publishしていないcommit、進行中のGit操作、またはunmanaged worktreeがある場合、sbxmは通常のrebuildを拒否します。

cleanなworktreeだけでは見えないrepository単位の状態も検査します。checkoutしていないlocal branch、tag、note、stash、追加remote、reflogだけに残るcommitも対象です。表示されたLayer Aのblockerを保存または解決してから、もう一度実行してください。originに無いbranch、tag、worktreeの`HEAD`のcommitは、rebuildが申し出たときにホスト側のrepositoryへ保存しても構いません。stashやnotesのcommitは保存できません。`status`ではworktreeの`STATE`とoriginからの回収根拠を、別の`REMOTE`列に表示します。

拒否しない場合も、作り直しで何が失われるかを先に表示します。無視対象のpath、checkoutしていないbranchやtagの名前、追加remote、reflogにだけ残るcommit、Sandboxの書き込み層が対象です。表示のあと、対象Sandbox名の完全一致入力を得た場合にだけ進みます。`rebuild`に確認を省略する方法はないため、非対話端末では実行できません。表示した状態が入力から実際の作り直しまでのあいだに変わった場合は、何も削除せずに中止します。

### Sandboxのroot sizeを選ぶ

Docker Sandboxesは、Sandboxを作成するprocessのenvironmentから`DOCKER_SANDBOXES_ROOT_SIZE`を読み取ります。sbxmはこの変数を解釈も書き換えもせず、`sbx create`を実行する時点で設定されている値をそのまま渡します。

```sh
# 初回作成
DOCKER_SANDBOXES_ROOT_SIZE=40g sbxm open <project-id>

# 既存Sandboxの作り直し
DOCKER_SANDBOXES_ROOT_SIZE=40g sbxm rebuild <project-id>
```

この設定が*しないこと*もいくつかあります。

- この変数は、これから作られるSandboxにだけ効きます。既存Sandboxのfilesystemをin-place resizeするものではないため、sizeを変えるにはSandboxの作り直しが必要です。`rebuild`はこの場合も他のrebuildと同じdata保護検査を通ります。
- requested sizeは作成時にその場で予約される量ではありません。各Sandboxが書き込める上限を引き上げるだけで、host上の複数Sandboxの実使用量は引き続きhostの実容量に対して合算されます。
- build済みimageとload済みtemplateは各Sandboxのroot filesystemとは別にhost容量を消費し、この設定とは独立しています。sbxmがその間に書き出すarchiveはloadが終われば消える短命fileであり、積み上がりません。

どちらのcommandを実行する前にも、要求するsizeに対してhostに十分な空き容量があるか確認してください。

### Sandboxのディスクを何が埋めるかを理解する

`sbxm status <project-id>`と`sbxm open <project-id>`は、Sandboxの空き容量、残りの割合、空きが減るほど短くなるゲージを表示します。全体容量は使用済みと空き容量の合計で、予約領域を含みません。

```text
ディスク  /
空き容量  4.7 GiB（残り25%）
          [█████░░░░░░░░░░░░░░░]  残り少なめ
全体容量  18.5 GiB
```

残量の割合は小数点以下を切り捨てます。容量と残量%は常に併記し、容量が1 GiB未満なら整数のMiB、1 GiB以上なら小数1桁のGiBで表示します。0より大きく1 MiB未満なら`<1 MiB`と表示し、KiBは使いません。ゲージと残量%は26%以上が緑、11〜25%が黄色の「残り少なめ」、10%以下が赤色の「残りわずか」です。空き100 MiB未満は割合より優先して赤色の「空きなし」とします。色なしでも状態ラベルを残し、ASCII端末ではゲージに`#`と`-`を使います。失敗時の診断にも同じ表示を使います。

この容量が何を反映しているかを理解するための事実です。

- `/home`、`/tmp`を含むSandbox内のすべては、1枚のroot filesystemを共有します。上の`DOCKER_SANDBOXES_ROOT_SIZE`でsizeを決めるのと同じfilesystemであり、build成果物や一時fileのための別volumeはありません。
- `/tmp`はSandboxを停止して再度開いて（`sbxm open`）も消えません。中でperiodicな掃除を行うinit systemが無いため、置いたfileはSandbox自体を破棄または作り直すまで残り続けます。
- Sandbox内でfileを削除すると、その場で空き容量が戻ります。root filesystemは通常の書き込み可能な層であり、作り直したときだけ空きが戻るsnapshotではありません。
- managed worktreeはそれぞれ独立してbuildするため、`--worktrees N`はbuild成果物（例えばRustの`target/`）をworktree数だけ増やします。
- 共有build cache directoryをサポートする言語・toolであれば（例えばRustの`CARGO_TARGET_DIR`）、下の「設定ファイルを配置する」で宣言するfile経由で全worktreeを同じdirectoryへ向けられ、この増加を避けられます。ただし1つのdirectoryを共有すると、本来は並行にできるworktreeごとのbuildが直列化されるため、既定にはせず明示的なtrade-offとして選んでください。

diskの復旧が必要なときは、次の順序で進めてください。

1. Sandbox内の不要なfileを削除する。空き容量はその場で戻ります。
2. rebuildで何を破棄するかを確認する。書き込み可能な層の作り直しが必要なら、通常の保護付き`rebuild`を実行し、表示されたplanを確認する。
3. 保護検査が示すLayer Aのblockerをすべて保存または解決してから再実行する。publishしていないcommit、dirtyまたは未追跡の作業、進行中のGit操作、active session、repository単位のrefも含まれます。

### managed worktreeを追加する

構築済みのプロジェクトには、rebuildせずにmanaged worktreeを追加できます。

```sh
sbxm apply <project-id> --worktrees 4
```

worktree数は増やすことだけができます。デフォルトのattached modeで登録したプロジェクトでは、最初のworktreeはtracking branch上に残り、追加のworktreeはdetachedになります。ここでも`--worktrees`は`-t`と短く書けます。

### 設定ファイルを配置する

すべてのSandboxへ配置するホスト側のファイルを宣言します。

```sh
sbxm files add ~/.claude/CLAUDE.md
```

配置先を省略すると、home directoryからの相対pathをそのまま使います。この例ではSandboxのhomeの`.claude/CLAUDE.md`へ配置します。別の場所へ置く場合は`--dest`を指定します。sbxmは、通常のファイルでサイズの上限内であることを確かめ、認証情報によく使われる名前には警告を出します。対話端末では、続けて登録済みのすべての案件へ今すぐ配置するかを訊きます。`sbxm files ls`で宣言を一覧し、`sbxm files rm <配置先>`で宣言を外します。Sandboxへ配置済みのファイルはそのまま残ります。

宣言は`~/.sbxm/config.yaml`に保存され、手で編集することもできます。sbxmはこのファイルを編集するとき、コメントや書き方をそのまま残します。

```yaml
version: 1

files:
  - source: /Users/you/.gitconfig
    destination: .gitconfig

  - source: /Users/you/.config/another-tool/settings.yaml
    destination: .config/another-tool/settings.yaml
```

配置先はSandbox userのhome directoryからの相対pathです。宣言したファイルはSandboxの初回構築時に配置されます。あとから加えた変更は明示的に適用します。

```sh
sbxm apply <project-id> --files
```

`--files`が置き換えるのは、sbxmが前回配置した内容のままの配置先だけです。Sandboxの中で編集されたファイルや、sbxmが配置した記録のないファイルがあれば、`apply`は1件も配置する前に拒否し、該当するファイルをすべて示します。Sandbox側の内容で必要なものを退避してから、明示的に置き換えてください。

```sh
sbxm apply <project-id> --files --force
```

宣言したファイルをSandboxの中で編集しても、`open`と`repair`はその案件を拒否せず、編集した内容を宣言ファイルで置き戻すこともしません。

`sbxm status <project-id>`は宣言ファイルごとに、sbxmが配置したあとにホスト側のファイルが変わったか（`updated`）と、Sandbox側で編集されたか（`modified`）を示します。

Sandboxの中で編集した内容をホスト側のファイルへ持ち帰るには、次を実行します。

```sh
sbxm files pull .claude/CLAUDE.md <project-id>
```

sbxmはSandbox側のファイルを案件の`.sbxm/incoming`へ取り出し、ホスト側のファイルとの差分を示してから、採用するかを訊きます。自動では混ぜず、対話端末でない実行では差分を示すだけです。Sandboxから来た制御文字は端末へ届けず、`\u{...}`の形で示します。

登録済みのすべての案件へまとめて配置する場合は、次を実行します。

```sh
sbxm apply --files --all
```

案件ごとにlockを取り、同じ規則で1件ずつ配置します。1件を配置できなくても、ほかの案件は続けます。停止中のSandboxは起動せず、Sandboxのない案件には初回構築で宣言が配置されます。どちらも結果に示します。配置できなかった案件が1件でもあれば、終了statusは`1`です。

token、private keyなどの認証情報はこれらのファイルに含めず、Docker Sandboxesのsecretを使用してください。

2つのapply対象は同時に指定できます。

```sh
sbxm apply <project-id> --files --worktrees 4
```

### Sandboxのcommitをホスト側のrepositoryに残す

originに無いbranch、tag、worktreeの`HEAD`のcommitだけを理由にrebuildやdestroyが拒否された場合、対話端末ではホスト側のrepositoryへ保存してから続けるかを訊きます。止める選択では何も変えません。

ホスト側のrepositoryが、`sbxm open`と同じSSHの接続でSandboxのbranch、tag、各worktreeの`HEAD`をfetchし、objectの検査を有効にして`refs/sbx/<sandbox>/`へだけ取り込みます。運ぶのは、ホスト側にまだ無いobjectだけです。ホスト側のbranchとtagには触れません。Sandboxで書き換えたbranchや消したbranchの前の先端は`refs/sbx/<sandbox>/archive/<時刻>/`へ退避し、自動では消さないため、一度保存したcommitはどれも辿れ続けます。Sandboxは動いている必要があり、停止中のSandboxは起動しません。

こうして保存したcommitはSandboxを消しても残るため、rebuildとdestroyはoriginから辿れるcommitと同じく、失われないものとして数えます。originに無いstashやnotesのcommitはこの方法では保存されず、pushするか捨てる必要があります。

## GitHubを使わずに開発する

このhostにすでにあるrepositoryも、プロジェクトにできます。sbxmはcloneしません。そのrepository自身がoriginの役を持ち、Sandboxは作業のための使い捨ての場所になります。working treeではなく、repositoryのgit directoryを渡します。

```sh
cd ~/Projects
sbxm add --local ~/code/<repository>/.git
sbxm open local/<repository>
```

pathはgit directoryでなければなりません。working treeの`.git`、bare repository、worktreeやsubmoduleの`.git` fileを渡せます。working treeのdirectoryは、その中にrepositoryがいくつあってもよく、どれを指すのかが決まらないため断ります。プロジェクトIDは`local/<name>`で、名前は`--name <name>`を渡さなければ`git clone`が使うdirectory名です。Sandboxは、渡したgit directoryが今いるbranch（worktreeの`.git`なら、そのworktreeのbranch）、または`--detach`で渡したbranchから始まります。

GitHub tokenは関わらないため、登録の手順は要りません。hostのrepositoryは`sbxm open`と同じ`ssh <sandbox>.sbx`でSandboxへ届くため、Docker SandboxesのRemote SSHの設定が要ります。設定があるかは`sbxm status --global`が示します。`open`がSandboxを構築するとき、hostのrepositoryがこの接続で、branchとtagをSandboxの`origin/*`へpushします。接続はhostから始まり、Sandboxの中からhostのrepositoryへ届く経路はありません。Sandboxの中で`git fetch origin`をしても、何も変えずに失敗します。

hostのrepositoryとSandboxは`sbxm sync local/<name>`で同期します。hostのrepositoryは、GitHubの案件でのGitHubの役を持ちます。Sandboxのbranchとtagは`git push`と同じ規則でhostへ、hostのbranchとtagは`git fetch --prune`と同じ規則でSandboxの`origin`へ届きます。早送りでないbranchや、別の先を指す同じ名前のtagのように、pushでGitが断るものは動かさず、そのcommitは`refs/sbx/<sandbox>/`に残ります。hostでcheckoutしているbranchは、そのworktreeで`git merge --ff-only`が通るなら、fileごと進めます。重なる未commitの変更があれば進めず、そのfileを示します。Sandboxのworktreeとbranchはそのままです。hostのcommitが足りないSandboxのbranchは、`sync`が名指しします。Sandboxの中で`origin/<branch>`へ取り込んでください。hostと分かれたbranchは、GitHubへもう一度pushする前と同じく、mergeかrebaseをしてから、もう一度同期してください。

Sandboxの`origin`はSandboxと一緒に消える写しにすぎないため、rebuildとdestroyは、hostのrepositoryから辿れるcommitだけを失われないものとして数えます。hostのbranchやtag、または`refs/sbx/<sandbox>/`へ保存したものから辿れる必要があります。hostに無いcommitがあれば止まり、対話端末では先に保存するかを訊きます。

失われうるのは最後に保存したあとにcommitした作業だけなので、sbxmは自分でも、branchを動かさずに`refs/sbx/<sandbox>/`へ保存します。`stop`、`rebuild`、`destroy`（`--force`を含む）がSandboxを止める前、`open`のsessionがつながっているあいだの10分ごと、そしてsessionを閉じたあとです。このために停止中のSandboxを起動することはありません。保存に失敗しても操作は続き、Sandboxにだけ残っているものがあることをwarningで伝えます。

`sbxm rebuild local/<name>`は、hostのrepositoryからSandboxを作り直し、`refs/sbx/<sandbox>/heads/`へ保存したbranchをSandboxのbranchとして戻します。hostに同じ名前のbranchがあれば、`origin/<branch>`をupstreamにします。起点branchのworktreeは保存した先端から作り直すので、保存したところから作業を続けられます。Sandboxがsbxmの外で消えた場合のように、`open`や`repair`が新しいSandboxを作るときも同じです。

## プロジェクトを破棄する

```sh
sbxm destroy <project-id>
```

sbxmは何かを削除する前に、削除するものと残すものを表示します。通常のdestroyでは、dirty worktree、publishしていないcommit、repository単位のref、active sbxm sessionを検査します。停止中のSandboxは、中を検査するために起動し、その事実を計画のそばに表示します。この起動はほかの準備を行わず、確認をcancelしてもSandboxは起動したまま残ります。対話端末では、続いて案件の登録ID（command lineへ渡す`<owner>/<repository>`）の入力を求め、一致しない場合は入力し直せます。Sandbox自体の削除では、Docker Sandboxes自身のruntimeが行うactive-session検査（sbxmが開始していないsession）も尊重します。Sandboxの削除要求ではscoped secretを保持し、その後で案件のGitHub用登録だけを削除します。通常のdestroyを非対話端末で実行した場合は、確認を省略せず拒否します。

Sandbox、sbxmのプロジェクトmetadata、そのSandbox向けに登録した`GH_TOKEN`のcustom secretは削除されます。登録が残ると、同じプロジェクトに対する次の`sbx secret set-custom`が重複として失敗し、存在しないSandbox宛のtokenを預けたままになります。ホスト側のclone、プロジェクトのDockerfile、build済みimage、load済みtemplate、それ以外を対象に登録したsecretは残るため、tokenを再登録すればあとからプロジェクトを再登録できます。

データ保護・active session・runtimeのin-use検査、および確認promptを意図的に省略する必要がある場合は、次を実行します。

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

`--local`で追加したプロジェクトは、このディレクトリの中にホスト側のcloneを持ちません。repositoryは追加したときの場所に残ります。

`~/.sbxm`には、登録済みプロジェクトとその場所の索引である`registry.yaml`を置きます。表示言語か名義を選ぶか、配置するファイルを宣言した時点で`config.yaml`も作られます。Docker Sandboxes CLIとGitのversion確認が通った実行ファイルは`host-tools.yaml`に記録し、変わらない限り確認を省略します。プロジェクトの場所を知っているのはregistryだけであるため、プロジェクトのディレクトリを移動すると、sbxmは新しい場所を推測せず`ls`で`missing`として表示します。

## sbxmがsbxを起動するとき

sbxmは、`sbx`をいつも`SBX_NO_TELEMETRY=1`と`DOCKER_SANDBOXES_DISABLE_UNLEASH=1`を付けて起動します。付けないと、sign-inした`sbx`は起動のたびに、利用状況の送信に約0.45秒、feature flagの取得にも同じくらいかけます。daemonが一覧を返すのは数msです。2つを付けると、`sbx ls`1回が約0.9秒から約0.05秒になります。

この2つの変数は、sbxmが起動した`sbx`のprocessにだけ届きます。sbxmはDocker Sandboxesの設定に何も書かないため、利用者が自分で実行する`sbx`の動きは変わりません。ただし、sbxmが起動した`sbx`がDocker Sandboxesのdaemonを起動した場合は、daemonが再起動するまで、この変数を持ち続けることがあります。

## コマンド一覧

| コマンド | 用途 |
|---|---|
| `sbxm add <github-clone-url>` | GitHub repositoryをsbxmへ追加し、このhostへcloneする |
| `sbxm add --local <git-dir> [--name <name>]` | このhostにあるGit repositoryを、cloneせずに`local/<name>`として追加する。`.git`などのgit directoryを渡し、それがoriginの役を持つ |
| `sbxm open [<project-id>] [--index N]` | SandboxへのSSH接続を開く。初回はSandboxを構築し、以降は必要なら先に起動する。`N`は0始まりのmanaged worktree index |
| `sbxm stop [<project-id> ...]` | 1件以上の案件のSandboxを、削除せず停止する |
| `sbxm ls` | 管理案件と管理外Sandboxを、その状態とともに一覧する |
| `sbxm guide` | 目的と案件を対話選択し、credentialを受け取らず案件も変更せず、現在状態に応じた次の手順を示す |
| `sbxm guide credential-rotation [<project-id>]` | 選んだ案件のscopeとplaceholderを維持してGitHub credentialを交換する手順を示す |
| `sbxm status` | 対話端末でhostまたは案件を選択して診断する。`global`を先頭にpromptを表示する |
| `sbxm status --global` | hostの状態を変更せずに診断する |
| `sbxm status <project-id>` | 案件の状態を変更せずに診断する |
| `sbxm sync [<project-id>]` | `--local`で追加した案件のSandboxとホスト側のrepositoryを、Gitのpushとfetchの規則で同期する |
| `sbxm files add\|ls\|rm ...` | すべてのSandboxへ配置するホスト側のファイルを宣言、一覧、または宣言を外す |
| `sbxm files pull <destination> [<project-id>]` | 宣言ファイルについて、案件のSandbox側の内容とホスト側のファイルの差分を示し、選べばホスト側のファイルへ採用する |
| `sbxm apply [<project-id>] ...` | 宣言済みファイルを配置するか、managed worktreeを追加する。`--files --all`で登録済みのすべての案件へ配置する |
| `sbxm repair [<project-id>]` | SSH接続を開かず、中断または未完成の案件を明示的に準備する。接続時は`open`が同じ復旧を行う |
| `sbxm rebuild [<project-id>]` | Dockerfileから案件のSandboxを作り直す（元の書き込み可能な層は失われる） |
| `sbxm destroy [<project-id>]` | Sandboxを破棄して案件を管理対象から外し、host cloneとDockerfileは残す |

完全なCLI referenceは、`sbxm --help`または`sbxm <command> --help`で確認できます。

## 出力

sbxmは結果を標準出力へ、進捗、prompt、警告、errorを標準エラー出力へ書きます。結果をリダイレクトしても、結果以外は混ざりません。

色はstreamごとに判定します。標準出力だけをpipeした場合、結果はplain textになり、端末に残る診断は色付きのままです。色は固定値ではなく端末themeが定義するANSIの標準色を使うため、利用者が選んだcontrastをそのまま尊重します。

| 設定 | 効果 |
|---|---|
| `--color=auto` | streamが端末のときだけ色を付ける（既定） |
| `--color=always` | リダイレクト先にも色を付ける |
| `--color=never` | 色を付けない |
| `NO_COLOR` | 値にかかわらず、空文字でも色を無効にする |
| `CLICOLOR_FORCE` | `0` 以外なら色を有効にする |
| `TERM=dumb` | 色を無効にし、markerをASCIIへ切り替える |

明示した `--color` は、どの環境変数よりも優先されます。色を消しても情報は失われません。marker、label、空行だけで同じ意味を読み取れます。

`--verbose` を付けると、debug用のtraceが標準エラー出力に並びます。標準出力は結果だけのままです。
