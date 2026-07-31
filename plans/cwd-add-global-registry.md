# `add`のcwd配置とglobal registry仕様

## 1. 目的

`sbxm add`を実行する利用者が、globalな保存先やowner directoryを事前に決めず、GitHubが表示するclone URLをそのまま貼り付けるだけでprojectを登録できるようにする。

この変更では次を一体として扱う。

- 新規projectの配置先を、固定`base_path`配下からcommand実行時のcwd配下へ変更する
- 任意の場所へ配置したprojectを、単一のglobal registryで発見できるようにする
- `sbxm init`と「初期化済み」という状態を廃止する
- 最初の対話的な`add`を、永続的な表示言語を選ぶ一度限りの入口にする
- `add`の入力をGitHub clone URLへ変更する
- 入力、registry、project metadata、host cloneの不一致は推測で解消せず、mutation前に拒否する

既存config、既存project layout、既存metadataのmigrationと後方互換性は提供しない。この仕様へ切り替えるreleaseでは、旧形式の状態を新形式として読み替えない。

## 2. 利用者向けの基本フロー

```sh
cd ~/Projects
sbxm add git@github.com:example-org/example-repo.git
```

または:

```sh
cd ~/Projects
sbxm add https://github.com/example-org/example-repo.git
```

初回の対話実行では、projectへmutationする前に表示言語を選ぶ。

```text
表示言語を選択してください / Choose a display language

> 日本語 / Japanese
  English
```

`add`はhost cloneとproject固有の成果物を用意し、GitHub tokenを登録する正確な`sbx secret set-custom` commandと、次のcommandを表示する。

```sh
sbxm prepare example-org/example-repo
sbxm open example-org/example-repo
```

利用者はprojectごとのdirectoryを作ったり、owner名を含む配置規則を揃えたりしない。配置先の親directoryへ移動することだけを担当する。

## 3. 公開CLIの変更

### 3.1 `init`

`sbxm init`を廃止し、help、usage、parser、localization、test、READMEから削除する。configやapplication stateが存在しないことを「未初期化」として扱わない。

### 3.2 `add`

公開syntaxを次に変更する。

```text
sbxm add <github-clone-url> [--worktrees <N>] [--detach <BRANCH>]
```

初期versionで受理する入力は、次の2形式だけとする。

```text
git@github.com:<owner>/<repository>.git
https://github.com/<owner>/<repository>.git
```

次は受理しない。

- `owner/repository`
- `.git`を持たないURL
- `ssh://`形式
- `http://`形式
- `github.com`以外のhost
- `git@github.com`以外のSSH userまたはhost
- credential、port、query、fragmentを持つHTTPS URL
- `<owner>/<repository>.git`以外のpath要素数
- 空のownerまたはrepository
- 現行の`ProjectId`規則に違反するownerまたはrepository

GitHubが提供するclone URLをclipboardから無加工で渡せることを、このsyntaxの公開目的とする。入力を寛容に推測して未対応形式へ対応しない。未対応形式は、受理する2形式を示して拒否する。

`--worktrees`、`--detach`とそのvalidation規則は現行仕様を維持する。

### 3.3登録後のproject指定

`prepare`、`open`、`stop`、`status`、`apply`、`rebuild`、`destroy`は、引き続き`owner/repository`をproject IDとして受け取る。`add`の完了出力は、利用者がproject IDを再入力せずコピーできる正確な次commandを表示する。

引数を省略できるcommandの対話選択も維持する。今回の変更では、cwdから既存projectを暗黙選択する機能を追加しない。

## 4. GitHub repository identity

clone URLをparseし、次の値へ分離する。

```text
provider
owner display
repository display
canonical project ID
clone transport
normalized clone URL
```

初期versionの`provider`は`github`だけ、`clone transport`は`ssh`または`https`だけである。canonical project IDは、現行どおりownerとrepositoryをASCII lowercase化した`owner/repository`とする。

例:

```text
input:          git@github.com:Example-Org/Example-Repo.git
provider:       github
owner:          Example-Org
repository:     Example-Repo
canonical ID:   example-org/example-repo
transport:      ssh
clone URL:      git@github.com:Example-Org/Example-Repo.git
```

host cloneには、validation済みの入力と同じtransportとclone URLを使用する。SSH入力をHTTPSへ、HTTPS入力をSSHへ暗黙変換しない。

## 5. Host project layout

### 5.1新規登録先

新規projectの配置先は次のとおりとする。

```text
<cwd>/<repository-lower>.project/
├── <repository-lower>/       # host clone
└── .sbxm/
    ├── project.yaml
    ├── project.lock
    ├── Dockerfile
    └── .cache/
```

cwdはproject rootそのものではなく、sbxmがproject rootを追加する親directoryである。owner directoryは作らない。

これにより、同じcwdで複数のrepositoryを登録できる。

```text
<cwd>/
├── alpha.project/
│   ├── alpha/
│   └── .sbxm/
└── beta.project/
    ├── beta/
    └── .sbxm/
```

同じrepository名を持つ異なるownerのprojectは、同じcwdでは同じpathを要求する。この場合はpath collisionとしてmutation前に拒否し、別の親directoryで`add`するよう案内する。owner名などを自動的に加えて衝突を回避しない。

### 5.2 cwdの確定

新規`add`はprocessのcurrent directoryを取得し、実在するdirectoryであることと、必要なproject rootを作成できることを検証する。registryへは解決済みの絶対project rootを保存する。

cwdを使用するのは新規canonical project IDの登録時だけである。

canonical project IDが既にregistryへ存在する場合、実行時cwdから新しい候補pathを作らない。保存済みproject rootを使用して、同じ登録の継続可否を判定する。したがって、登録済みprojectに対する`add`は、どのcwdから実行しても保存済みの場所だけを対象とする。

`add`をprojectの移動、複製、clone transport変更には使用しない。これらの機能は今回のscope外とする。

## 6. Global state

global領域は次の構成とする。

```text
~/.sbxm/
├── config.yaml
├── registry.yaml
└── registry.lock
```

`config.yaml`は利用者設定、`registry.yaml`は登録projectの索引であり、責務を混ぜない。

### 6.1 `config.yaml`

global configから`base_path`を削除する。

```yaml
version: 1
language: ja
files: []
```

意味:

- `language`: 永続的な表示言語
- `files`: Sandbox内へ配置する任意のhost file宣言

config fileが存在しないことは正常であり、default設定として扱う。`files`は空とみなす。ただし、存在するconfigが構文不正、未知version、permission不正、symlink、またはread失敗である場合はdefaultへfallbackせず、現行の安全規則に従って拒否する。

`config.yaml`と`registry.yaml`は別documentである。言語設定の更新でregistryを書き換えず、project登録でconfigを書き換えない。ただし、初回の対話的`add`で言語を選んだ場合だけは、後述の規則に従ってconfigを新規作成または更新してからproject登録へ進む。

### 6.2表示言語の決定

表示言語の優先順位は次のとおりとする。

1. 有効なglobal option `--lang`
2. 有効な`config.yaml`の`language`
3. system locale
4. 正本localeであるEnglish

`--lang`はそのprocessだけのoverrideであり、永続設定を書き換えない。

helpとusageはpromptを表示せず、上記の利用可能な情報から言語を決定する。config不正だけを理由にhelp表示を失敗させないという現行のbest-effort規則は維持する。

### 6.3初回の言語prompt

対話的な`add`を実行し、`config.yaml`に有効な`language`が保存されていない場合は、project、registry、host cloneへmutationする前に一度だけ言語選択promptを表示する。

```text
表示言語を選択してください / Choose a display language

> 日本語 / Japanese
  English
```

規則:

- prompt本文は選択前でも日本語利用者と英語利用者の双方が理解できる固定の二言語表記とする
- 日本語の選択肢は`日本語 / Japanese`、英語の選択肢は`English`とする
- system localeから推測した言語を初期cursor位置にする
- 選択結果を`config.yaml`の`language`へ保存する
- 保存後、その`add`の残りの出力から選択言語を使用する
- promptをcancelした場合はexit code `130`で終了し、configを含め何も変更しない
- 有効なlanguageが既に保存されていれば、`--lang`で一時overrideしていてもpromptを出さない
- languageが未保存で`--lang`を指定した対話的`add`では、overrideを永続設定とみなさず、一度限りのpromptを省略しない

最後の規則により、`--lang`の「そのprocessだけ」という意味と、永続設定を利用者自身が選択する契約を混同しない。

stdinまたはstderrがTTYでない`add`ではpromptを表示せず、言語を永続化しない。その実行は上記優先順位で選んだ言語を使用して継続する。後日初めて対話的な`add`を実行したときにpromptを表示する。

`status --global`、`ls`、helpなどのread-only操作はpromptを出さず、configを作成しない。

### 6.4 `registry.yaml`

任意の場所へ配置された全projectを、単一のregistry documentで管理する。

registryには索引と中断した登録の再開に必要な意図を保存する。project workflowにおけるproject固有情報の正本はproject rootの`.sbxm/project.yaml`とするが、project metadataを作る前の中断から同じ要求だけを安全に再開できるよう、entryにも登録時のrepository identity、provisioning、Git identityを持たせる。

```yaml
version: 1
projects:
  - canonical_id: example-org/alpha
    project_root: /home/user/Projects/alpha.project
    provider: github
    clone_transport: ssh
    clone_url: git@github.com:Example-Org/Alpha.git
    provisioning:
      mode: attached
      start_ref: null
      requested_worktrees: 1
    git_identity:
      user_name: Example User
      user_email: user@example.com
```

registryへ`state` fieldを保存しない。登録状態は、registry entry、project root、project metadata、host cloneというfilesystem上の事実を観測して算出する。

| 観測結果 | 算出する状態 |
|---|---|
| registry entryのみ存在 | project root作成前に中断 |
| project rootは存在するがmetadataがない | metadata作成前に中断、または不整合 |
| 一致するmetadataがあるがhost cloneがない | clone前に中断 |
| 一致するmetadataとhost cloneがある | 登録済み |
| metadataまたはoriginがentryと一致しない | 不整合 |

状態名を永続化して観測事実と二重管理しない。project rootやmetadataがない状態も、entry自体が有効であれば登録意図の予約として扱う。read-only commandは観測結果から登録途中または不整合を表示し、同一要求の`add`だけが続きを実行できる。

`clone_url`、`provisioning`、`git_identity`は、project metadata作成前の中断でも登録意図を完全に復元するためregistryにも保存する。project metadataが存在する場合は同じ値を持つことを検証する。二つの正本を自由に更新するのではなく、registryは登録予約と索引、project metadataはproject workflowの正本として扱う。不一致時はどちらかを推測で採用しない。

registryの不変条件:

- 1 canonical project IDにつき1 project root
- 1 project rootにつき1 canonical project ID
- 異なるcanonical project IDが同じSandbox名を導出しない
- canonical project IDとproject rootはvalidation済みの値だけを持つ
- project rootは絶対pathである
- `provider`、`clone transport`、`clone URL`が同じentryのcanonical project IDと一致する
- `provisioning`と`git_identity`がvalidation済みである
- 観測から算出できる状態を表すfieldを持たない

registry entryが指すdirectoryの移動または消失を、自動探索、cwd、類似名から推測して修復しない。entryを黙って削除しない。

### 6.5 `registry.lock`とatomic update

registry mutationは`~/.sbxm/registry.lock`に対するglobal exclusive lockで直列化する。registryを単純追記しない。更新は次の順で行う。

1. global registry lockを取得する
2. `registry.yaml`全体を読む
3. version、構文、全entry、不変条件を検証する
4. memory上で変更後の完全なdocumentを構築する
5. 同一directoryのtemporary fileへ書く
6. private permissionを設定する
7. fileを`fsync`する
8. atomic renameで`registry.yaml`を置き換える
9. 親directoryを`fsync`する
10. lockを解放する

一部entryだけが正常でも、壊れたregistryの一部をmutationの根拠として信用しない。registryが不正な場合、すべてのmutationを停止する。

registry lockはregistry documentの一意性を守る。project固有のmutationは、registry lockに加えて保存済みproject rootのproject lockで直列化する。deadlockを避けるため、複数lockが必要なworkflowでは常にregistry lock、project lockの順で取得する。

## 7. Project metadata

project metadataはrepository identityとclone方式を明示的に保存する。

```yaml
version: 1
repository:
  provider: github
  owner: Example-Org
  name: Example-Repo
  canonical_id: example-org/example-repo
  clone_transport: ssh
  clone_url: git@github.com:Example-Org/Example-Repo.git
provisioning:
  mode: attached
  start_ref: null
  requested_worktrees: 1
  dockerfile_sha256: "<sha256>"
git_identity:
  user_name: Example User
  user_email: user@example.com
```

実際のfield構成は既存のrebuild metadataを含めて定義するが、少なくとも上記のrepository情報を一つの解釈済み構造として持つ。clone URL文字列から実行時にtransportを推測し直さない。

Sandbox内のGit identityは、新規登録時にhostの次の設定から取得する。

```sh
git config --global user.name
git config --global user.email
```

両方をproject metadataへsnapshotし、`prepare`以降は保存値を使用する。host設定が後から変わっても、登録済みprojectのidentityを暗黙変更しない。どちらかが不在、空、複数値、または観測不能なら、registryやprojectを作る前に拒否し、設定する正確な`git config --global` commandを案内する。

global configからGit identity fieldを削除する。既存projectのidentityを変更する機能は今回提供しない。

## 8. `add`の判定とmutation順

### 8.1 mutation前validation

`add`は少なくとも次をmutation前に検証する。

1. CLI syntaxと`--worktrees`、`--detach`
2. clone URLのprovider、形式、repository identity
3. configの有効性
4. 必要なら初回言語promptとlanguage保存
5. host Git identity
6. registry全体の有効性
7. canonical project ID、project root、Sandbox名の衝突
8. 新規登録ならcwdと候補project root
9. 登録済みならregistryが指すproject metadata
10. 保存済みclone URLおよびtransportとの一致
11. host cloneが存在する場合は、その構造と`origin`との一致

言語保存はproject mutationではないが、利用者がpromptで明示的に選んだ独立した設定mutationである。language保存後にrepository validationやcloneが失敗しても、選択済みlanguageをrollbackしない。

### 8.2新規登録

新規登録は次の順でrecordする。

1. global registry lockを取得する
2. registry全体と新規要求の衝突を検査する
3. 登録意図を持つentryをregistryへatomic recordする
4. registry lockを保持したままproject rootを作成し、project lockを取得する
5. Dockerfileとproject metadataをatomic createする
6. entryとproject metadataを再検証する
7. project lockとregistry lockを解放する
8. host cloneをproject lock下で作成または検証する

project metadata作成までの短いlocal filesystem工程ではglobal registry lockを保持する。長時間かかるclone中はglobal registry lockを保持しない。登録意図のentryを先に記録するため、別cwdから同じcanonical project IDを登録しようとしても、先行要求を見失わない。同じcanonical ID、root、clone URL、provisioning要求による再実行だけが保存済みrootで続きを実行できる。

工程途中の失敗をrollback目的で暗黙削除しない。再実行が同じ意図を安全に継続できるよう、registry entryと成功済み成果物を残す。少なくとも次のcrash pointをtestする。

- registry entry記録後、project root作成前
- project root作成後、project metadata作成前
- project metadata作成後、clone前
- clone途中
- clone後、結果表示前

entryが指すrootを作成できない状態になっても、別pathへ暗黙変更しない。read-only診断と、同じ要求による再実行で観測した原因を表示する。利用者の成果物やentryを自動削除しない。

### 8.3登録済みproject

registryにcanonical project IDが存在する場合、cwdを無視して保存済みrootを読む。次のすべてが一致した場合だけ、現行`add`と同様に中断した工程を継続できる。

- registryのcanonical project ID
- registryのproject root
- project metadataのcanonical project ID
- project metadataから導出するSandbox名
- 入力clone URLのprovider
- 入力clone transport
- 保存済みclone URL
- host cloneが存在する場合、その唯一の`origin` URL
- 明示指定されたprovisioning option

一致しない場合はmutationしない。

特に、同じcanonical project IDでもSSHとHTTPSを同一構成とみなさない。

```text
registered: git@github.com:Example-Org/Example-Repo.git
requested:  https://github.com/Example-Org/Example-Repo.git
result:     target configuration mismatch
```

errorはrequested、registered、再実行すべき正確な`sbxm add <registered-url>`を表示する。`add`が保存済みclone URL、clone transport、registry root、実際のoriginを暗黙変更しない。

clone URLの一致は、parse済みのprovider、canonical project ID、transportで判定する。GitHubではownerとrepositoryの表示上の大文字小文字だけが異なっても同じidentityとして受理するが、保存済みdisplay spellingとclone URLを暗黙更新しない。host cloneの`origin`も同じ規則でparseして比較する。parse不能、複数origin、provider差異、canonical ID差異、transport差異は常に拒否する。

## 9. `ls`、project選択、project command

`ls`と引数省略時のproject候補は、`base_path`走査ではなくregistryから構築する。

`ls`は少なくとも次を表示する。

- project display ID
- host project root
- Sandbox state
- project pathまたはmetadataの異常状態

registry entryが指すpathが消失しても、一覧から黙って消さず`missing`として表示する。metadataが一致しなければ`inconsistent`として表示する。

read-only一覧では、復旧に必要な全entryを可能な範囲で表示したあと、1件でも不正があれば非zeroで終了する。構文上registry document全体をparseできない場合は、安全に復元できるentryだけを推測して表示しない。

完全指定されたproject commandも、canonical IDからregistryを引いてproject rootを解決する。固定layoutを再計算せず、cwdから対象を推測しない。

## 10. `status --global`

`status --global`は「初期化済み」を診断しない。次をread-onlyで診断する。

- Docker、Docker Sandboxes CLI、Gitなどのhost tool
- global state directoryの読取・書込可否
- 任意global configの状態
- registry documentのversion、構文、permission
- canonical project ID、project root、Sandbox名の重複
- 各登録project rootの存在
- registryとproject metadataの一致
- project metadataとhost clone originの一致
- Git identityの利用可能性

config不在は`defaults`として正常扱いする。registry不在は登録project 0件として正常扱いする。不整合を報告しても、自動作成、自動削除、自動移動、自動修復を行わない。

## 11. `destroy`とregistry

projectを管理対象から外す成功した`destroy`は、project固有stateの処理と同じtransaction境界でregistry entryを削除する。registry entryだけを先に消して、projectを発見不能にしない。

通常destroyがDockerfileやhost cloneなど利用者の成果物を残す現行方針は維持する。残したdirectoryをregistryから外したあとは、未登録のhost artifactとして扱う。

registryまたはproject metadataが不整合な場合、通常modeで推測してentryを削除しない。不整合状態からregistryだけを明示的に修復するworkflowは、この仕様とは別に設計する。

## 12. Securityとfailure policy

- clone URLはshellへ渡さず、validation済みargumentとして`git clone`へ渡す
- HTTPS URLにcredentialを許可しない
- project root、`.sbxm`、registry、configのsymlink規則を明示し、既存のsecurity errorを維持する
- registry entryのabsolute pathを信用する前に、path type、ownership、metadata対応を検証する
- 既存fileやdirectoryを、名前が一致するだけでsbxmの成果物として採用しない
- 外部状態を観測できなければ、一致していると推測しない
- registry、metadata、originの不一致は危険側へ倒してmutationしない
- 成功済み成果物をrollback目的で暗黙削除しない
- errorには観測した値、期待値、安全な再実行commandまたは手動確認方法を含める

## 13. Test契約

少なくとも次をunit、workflow、CLI contract testへ固定する。

### Clone URL

- SSH形式を受理する
- HTTPS形式を受理する
- canonical ID、display ID、transport、URLを正しく分離する
- `owner/repository`を拒否する
- `.git`なし、別host、credential、port、query、fragment、余分なpathを拒否する
- SSHとHTTPSの再登録差異を拒否する
- host cloneのorigin不一致を拒否する

### Layout

- 新規rootが`<cwd>/<repository-lower>.project`になる
- owner directoryを作らない
- 同じcwdへ異なるrepositoryを追加できる
- 同じcwdの同名repository path collisionをmutation前に拒否する
- 登録済みprojectの再実行はcwdを無視する

### Language

- 初回の対話的`add`だけpromptを出す
- promptの固定文言と選択肢を契約として検証する
- 選択をconfigへ保存し、同じ実行からlocaleを切り替える
- 保存済みlanguageがあればpromptを出さない
- 非TTYではpromptも保存も行わない
- `--lang`が永続設定を書き換えない
- help、`ls`、`status --global`がpromptやconfig mutationを行わない
- cancel時は何も作らずexit `130`

### Registry

- registry不在を0件として扱う
- 単一YAMLへatomic updateする
- concurrentな異なるproject登録でentryを失わない
- concurrentな同一project登録を直列化する
- duplicate canonical ID、root、Sandbox名を拒否する
- malformed、unknown version、permission不正、symlinkを拒否する
- temporary file、rename、`fsync` failureで不完全なdocumentを正本にしない
- missing rootを黙って削除しない
- lock順序を全mutationで統一する

### Commands

- `init`がCLI surfaceに存在しない
- `ls`と選択候補がregistry由来になる
- 完全指定commandがregistryからrootを解決する
- `status --global`がconfig不在とregistry不在を正常扱いする
- `destroy`成功時に対応entryだけを削除する
- registry不整合時にmutationしない

## 14. Scope外

次は今回提供しない。

- 旧config、旧metadata、旧layoutからのmigration
- GitHub以外のprovider
- 登録済みprojectの移動または複製
- clone transportまたはoriginの変更
- 登録済みprojectのGit identity変更
- cwdから既存projectを暗黙選択する機能
- registry不整合を推測で修復するcommand
- `config.yaml`を対話編集する新しい公開command

不整合の明示修復workflowが必要になった場合は、観測、対象指定、削除範囲を別仕様で定義する。
