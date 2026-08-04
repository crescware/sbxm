# sbxm公式サイト情報設計

## 目的

この文書は、`website-requirements.md`を満たすpage構成、navigation、content責務、将来の
English/Japanese対応関係を定める。初回に公開するpageはすべてEnglishで書く。

## 利用者導線

### 初回導入

```text
Home
  -> Requirements
  -> Install
  -> Quickstart
       -> Verify the host
       -> Register a project
       -> Register the GitHub credential
       -> Prepare and open the sandbox
```

### 日常利用

```text
Home or search
  -> Guides
       -> Daily workflow
       -> Managed worktrees
       -> Customize the image
       -> Place configuration files
       -> Tear down safely
  -> Command reference
```

### 問題解決

```text
Error or unexpected state
  -> Troubleshooting
       -> sbxm status --global
       -> sbxm status <project-id>
       -> Safety refusals
       -> Relevant command reference
```

## Global navigation

headerとsidebarは役割を分ける。

### Header

- `sbxm`: homepage
- search
- theme selector
- GitHub: `https://github.com/crescware/sbxm`
- language selector: 初回は表示しない。日本語追加時にStarlight標準selectorを有効にする

### Sidebar

```text
Get started
  Overview
  Requirements
  Install
  Quickstart

Guides
  Daily workflow
  Managed worktrees
  Customize the sandbox image
  Place configuration files
  Tear down safely

Reference
  CLI overview
  add
  apply
  prepare
  rebuild
  open
  stop
  ls
  status
  destroy
  Global options
  Configuration file
  Files and directories
  Output and color

Troubleshooting
  Diagnose the host
  Diagnose a project
  Safety refusals

Project
  Design principles
  Development
```

sidebar順はlearning pathとCLI lifecycleを表すため明示設定する。directory名のalphabetical orderへ
任せない。

## Route一覧

### Landing

| Route | Page title | 役割 |
| --- | --- | --- |
| `/` | sbxm | product value、security boundary、requirements概要、primary CTA |

### Get started

| Route | Page title | 主な内容 |
| --- | --- | --- |
| `/getting-started/` | Get started | 4段階workflowのoverview、所要前提、次pageへの導線 |
| `/getting-started/requirements/` | Requirements | supported host、Docker Desktop、Docker Sandboxes CLI、Git、SSH、PAT |
| `/getting-started/install/` | Install sbxm | Homebrew command、install確認、upgradeへの短い案内 |
| `/getting-started/quickstart/` | Create your first sandbox | `status --global`から`open`までのend-to-end手順 |

### Guides

| Route | Page title | 主な内容 |
| --- | --- | --- |
| `/guides/daily-workflow/` | Daily workflow | `ls`、`status`、`open`、`stop`の使い分け |
| `/guides/worktrees/` | Managed worktrees | attached/detached、1–32、増加のみ、agent/task分離 |
| `/guides/custom-image/` | Customize the sandbox image | generated Dockerfile、edit、`rebuild`、保護check |
| `/guides/configuration-files/` | Place configuration files | `config.yaml` schema、destination、`apply --files`、secret warning |
| `/guides/teardown/` | Tear down safely | normal destroy、削除対象、残存物、再登録、`--force` |

GitHub credential登録はquickstartの独立sectionとする。tokenの扱いを見落とさせないため、単なる
referenceへ隔離しない。必要なら将来`/guides/github-credentials/`へ分割するが、初回はpage数を
増やさずend-to-end flowを優先する。

### Reference

| Route | Page title | 主な内容 |
| --- | --- | --- |
| `/reference/cli/` | CLI reference | lifecycle順のcommand一覧、syntax規約、project ID |
| `/reference/cli/add/` | `sbxm add` | accepted clone URL、identity、worktree/detach option、mutation |
| `/reference/cli/apply/` | `sbxm apply` | required scope、files/worktrees、overwriteと増加のみ、optional project prompt |
| `/reference/cli/prepare/` | `sbxm prepare` | build/provision、credential前提、成果物、optional project prompt |
| `/reference/cli/rebuild/` | `sbxm rebuild` | recreation、安全check、rebuild intent、optional project prompt |
| `/reference/cli/open/` | `sbxm open` | optional project prompt、start、SSH接続 |
| `/reference/cli/stop/` | `sbxm stop` | zero-or-more project argument、interactive selection |
| `/reference/cli/ls/` | `sbxm ls` | global project/sandbox state、missing表示 |
| `/reference/cli/status/` | `sbxm status` | `--global`とproject scope、read-only contract |
| `/reference/cli/destroy/` | `sbxm destroy` | prompt、checks、`--force`、delete/keep matrix |
| `/reference/cli/global-options/` | Global options | `--lang`、`--color`、`--help`、`--version` |
| `/reference/configuration/` | Configuration file | version 1、identity/default、files declaration、validation |
| `/reference/filesystem/` | Files and directories | `.project/`、`.sbxm/`、registry/config、sandbox worktree path |
| `/reference/output/` | Output and color | stdout/stderr、auto/always/never、environment precedence |

各command pageは同じtemplateで次を並べる。

1. summary
2. synopsis
3. arguments
4. options
5. behavior and mutations
6. safety checks or refusal conditions
7. examples
8. related commands

「実装が持つ全error IDの辞書」は初回scopeにしない。利用者が行動を変えられる主要条件をguideと
command pageへ載せる。

### Troubleshooting

| Route | Page title | 主な内容 |
| --- | --- | --- |
| `/troubleshooting/` | Troubleshooting | 最初にstatusを使うdecision tree、support情報 |
| `/troubleshooting/host/` | Diagnose the host | platform、command、daemon、login、network、SSH checks |
| `/troubleshooting/project/` | Diagnose a project | registry、artifacts、sandbox、repository、worktree、secret checks |
| `/troubleshooting/safety-refusals/` | Resolve safety refusals | dirty、unpushed、unmanaged worktree、active session、collision |

troubleshootingはerror textを大量に複製しない。「観測するcommand」「原因のcategory」「利用者が
安全に確認する順序」「関連reference」へ絞る。error wordingそのものはCLIを正本とする。

### Project

| Route | Page title | 主な内容 |
| --- | --- | --- |
| `/project/design-principles/` | Design principles | ambiguity、安全側へのrefusal、ownershipを推測しない方針 |
| `/project/development/` | Development | mise、toolchain、`mise run check`、test/coverageへの入口 |

release operator向け手順は初回siteへ全文掲載しない。GitHub Releasesとrepository内の
`scripts/release/README.md`へlinkする。

## Homepage構成

Starlightの`splash` templateとheroを土台にする。hero後のfeature sectionは小さなAstro/MDX
componentへ分け、Tailwind utilityで組み立てる。Starlightのnavigationやdocument layout全体を
置き換えるcustom landing pageにはしない。

### Hero

- eyebrow/site title: `sbxm`
- headline候補: `A Docker Sandbox for every GitHub project.`
- tagline: Git worktreesとlifecycle managementを含む一文
- primary CTA: `Get started`
- secondary CTA: `View on GitHub`
- installation snippet: `brew install crescware/tap/sbxm`

headlineは「securityを完全に保証する」「どのrepositoryでも動く」など、実装を超えたclaimを
含めない。

### Hero後のsection

1. `One project, one sandbox` — project単位のhost artifactとsandbox
2. `Predictable worktrees` — attached/detachedと複数task
3. `Credentials stay outside` — secret proxyと渡さないhost resource
4. `A guarded lifecycle` — status、rebuild、destroyの安全check
5. `What you need` — Apple silicon/macOS 14+/Docker Desktop/sbx 0.37.0+
6. final CTA — requirementsまたはquickstart

diagramを置く場合は次の関係だけを示し、装飾目的のarchitecture図は作らない。

```text
Host project artifacts -> sbxm lifecycle -> Docker Sandbox
                                              |- shared repository
                                              |- managed worktree 1
                                              `- managed worktree N

GitHub credential -> Docker secret proxy -> allowed GitHub hosts
```

diagramには同じ内容のtext descriptionを付ける。

## Quickstartのcontent contract

quickstartはREADMEの順序を維持する。

1. `sbxm status --global`
2. 必要な場合だけglobal Git name/emailを設定する
3. projectを置くparent directoryへ移動する
4. SSHまたはHTTPSのGitHub clone URLをそのまま`sbxm add`へ渡す
5. first interactive addのlanguage/identity promptを説明する
6. outputされたproject-specific `sbx secret set-custom` commandをPAT付きで実行する
7. `sbxm prepare <project-id>`
8. `sbxm open <project-id>`
9. sandbox内worktree pathを確認する

quickstart中のcredential sectionは、fine-grained tokenの`Contents: read and write`と
`Metadata: read`、classic tokenの`repo` scopeを明記する。exampleの`<token>`は実値でないと
明示し、documentやshell historyへtokenを貼ることを促す補助scriptは作らない。

## Cross-link規約

- guideで初めてcommandを出すとき、そのcommand referenceへlinkする
- command referenceから概念を長く説明せず、該当guideへ戻す
- destructive behaviorからteardown guideへ必ずlinkする
- worktree数の説明からworktree guideへ必ずlinkする
- credential errorからquickstartのcredential sectionへ必ずlinkする
- external requirementは公式sourceへlinkし、第三者blogを根拠にしない
- relative filesystem linkではなくsite rootを基準にしたURLを使う

## English content規約

- page titleはsentence caseとする
- command pageだけはbacktick付きcommand名をtitleに使える
- headingは動作または問いを表す: `Register a project`、`What gets deleted`
- paragraph内でcommand、option、path、environment variableをbacktickで囲む
- `sandbox`一般とproduct名`Docker Sandbox`を文脈に応じて区別する
- `repository`、`worktree`などGit固有語を無理に言い換えない
- `simply`、`obviously`、`just`で前提知識を矮小化しない
- warningは危険と回避策を同じblockに置く

## EnglishからJapaneseへの対応

初回のEnglish fileと将来のJapanese fileは次のように対応させる。

```text
website/src/content/docs/getting-started/quickstart.md
website/src/content/docs/ja/getting-started/quickstart.md

website/src/content/docs/reference/cli/destroy.md
website/src/content/docs/ja/reference/cli/destroy.md
```

### 追加手順

1. Starlight locale設定へ`ja`を加える
2. sidebar group labelへJapanese translationを加える
3. `src/content/docs/ja/`へhomepageと優先pageを同じ相対pathで作る
4. custom visible stringがあれば`src/content/i18n/ja.json`へ追加する
5. language selector、`lang=ja`、alternate URL、fallback noticeを検証する
6. English route snapshotが変化していないことを検証する
7. PagefindがJapanese queryでJapanese contentを返すことをproduction previewで確認する

### Translation policy

- incomplete translationはStarlight fallbackを利用できる構成にする
- fallback pageには未翻訳noticeを表示し、English textをJapaneseとして見せない
- code、command、option、pathは原則翻訳しない
- external productの正式名称を翻訳しない
- Japanese固有slugを作らず、Englishと同じstable slugを使う
- English pageのrename時は両localeのredirectとtranslation対応を同時にreviewする

## Navigation acceptance scenarios

### New user

homepageから2 interaction以内にrequirementsへ到達し、対応hostであるか判断できる。そこから
next navigationだけでinstallとquickstartを完走できる。

### Existing user

header searchへ`rebuild`と入力し、command referenceとcustom image guideの双方を区別して選べる。

### Safety check

`destroy` pageでnormalと`--force`の違い、削除されるもの、残るもの、再登録時の注意を一画面の
heading構造から見つけられる。

### Future Japanese user

`/ja/getting-started/quickstart/`からlanguage selectorで同じEnglish pageへ移動でき、その
English URLが`/getting-started/quickstart/`のまま保たれる。
