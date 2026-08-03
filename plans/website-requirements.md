# sbxm公式サイト要件

## 文書の位置づけ

この文書は、repository rootの`website/`にAstro Starlightで構築するsbxm公式サイトの
プロダクト要件を定める。初回公開は英語だけとするが、英語ページのURLや配置を変えずに
日本語を追加できることを初回実装の必須要件とする。

サイト本体の実装、package導入、deployment設定はこの計画の承認後に行う。この計画では
`website/`を作らない。

## 決定事項

- frameworkはAstro Starlightとする
- stylingはTailwind CSS v4とStarlight公式のTailwind compatibility packageを使う
- Tailwind以外の追加UI frameworkやdesign systemは必須にせず、Starlightの基礎構成を維持する
- site sourceはrepository rootの`website/`以下にまとめる
- 初回公開するcontentとUIは英語だけとする
- 英語はURL prefixを持たないroot localeとする
- 将来の日本語は同じslugを使って`/ja/`以下に追加する
- 静的HTMLとしてbuildし、利用者のbrowserからserver-side APIを必要としない
- documentation navigationのprimary groupは`Get started`、`Guides`、`Reference`、
  `Troubleshooting`とする
- GitHub repositoryへのlinkと各documentの`Edit page` linkを提供する
- site内検索はStarlight標準のPagefindを使う
- analytics、cookie banner、外部font、広告、利用者追跡は初回scopeに含めない

## sbxmの位置づけ

### 一文での説明

> sbxm gives each GitHub project its own Docker Sandbox and a predictable set of Git worktrees.

homepageではこの説明を中心に、次の3点を一目で伝える。

1. GitHub projectごとに独立したDocker Sandboxを用意するCLIである
2. host clone、sandbox image、repository setup、接続、診断、再構築、破棄を一つのworkflowで扱う
3. host project directory、Docker socket、SSH agent、実tokenをsandboxへ持ち込まない

「一般的なcontainer manager」「任意のGit provider向け」「Linux向け」と誤認させる表現は使わない。

## 対象利用者

### Primary

- Apple silicon MacでDocker DesktopとDocker Sandboxesを使うdeveloper
- GitHub repositoryごとに再現可能な隔離環境を用意したいdeveloper
- 複数のagentやtaskへ独立したGit worktreeを割り当てたいdeveloper

### Secondary

- sbxm導入可否を評価するtechnical lead
- sbxmのCLI、設定file、safety modelを確認したい既存利用者
- sourceからbuild、test、contributionを行うmaintainerまたはcontributor

## 利用者の主要課題

- 自分のhostが対応しているか短時間で判断したい
- installationから最初のsandboxへ接続するまで、順序を間違えず進めたい
- GitHub tokenをsandboxへ直接copyせず登録したい
- 複数worktree、Dockerfile、配置fileを安全に変更したい
- destructive operationの前に、何が消え何が残るか確認したい
- commandの引数と実行結果をすぐ参照したい
- errorやrefusalの次に何を確認すべきか知りたい

## 成功条件

初回公開は次をすべて満たしたときに成功とする。

- homepageだけで、用途、対応platform、security boundary、installationへの入口が分かる
- 初見の利用者が`Requirements`から`open`まで順番どおり辿れる
- 全9 subcommandとglobal optionのreferenceへsite navigationまたは検索から到達できる
- `destroy --force`、token登録、配置fileについて、危険性と安全な通常手順が明記される
- 現在の英語URLを一つも変更せずに日本語相当pageを追加できる
- 主要pageがmobile幅、keyboard操作、light/dark themeで利用できる
- production build、内部link検査、contentとCLI surfaceの整合検査がCIで成功する

page view、conversionなどの数値KPIはtrackingを導入しない初回には設定しない。利用状況を測る
必要が生じた場合は、privacy要件と収集目的を別計画で決める。

## 初回公開scope

### 必須content

- landing page
- system requirements
- Homebrew installation
- first projectを登録しsandboxへ接続するend-to-end quickstart
- GitHub credentialとDocker Sandboxes secret proxyの説明
- daily operation
- managed worktree
- Dockerfileによるimage customization
- global configuration fileによるfile placement
- teardownとsafety checks
- 全commandとglobal optionのCLI reference
- filesystem layoutとglobal state
- stdout、stderr、colorの規約
- `status --global`と`status <project-id>`を入口にしたtroubleshooting
- projectのdesign principles
- development environmentへの案内
- GitHub repository、release、licenseへのlink

### 初回scope外

- 日本語contentの公開
- blog、news、roadmap、mailing list
- release noteのsite内mirrorとversion selector
- browser上でのbinary download/install wizard
- interactive terminal emulator、playground、login、account
- CMS、database、server-side search、server-side rendering
- community plugin catalog
- analytics、support chat、feedback widget
- READMEやRust sourceから全文documentを自動生成する仕組み

scope外の項目に備えて過剰なcomponentやdata layerを先に作らない。ただし日本語追加に必要な
locale、route、content対応規約だけは初回から組み込む。

## 機能要件

### Navigation

- headerにsite title、search、theme selector、GitHub linkを置く
- documentation sidebarはtask順で固定し、filesystem順へ依存させない
- page内にはh2とh3のtable of contentsを表示する
- guideは前後page navigationで自然に読み進められる
- current pageとcurrent sectionが視覚以外でも識別できる
- mobileでは同じ情報へmenuから到達できる

### Search

- production buildで全user-facing documentをfull-text検索できる
- title、description、heading、本文、command名を検索対象とする
- landing pageの装飾的な重複文言は必要に応じてindexから除外する
- 初回は英語indexだけを持ち、日本語追加後はlocaleごとの検索体験を確認する

### Code examples

- shell exampleはcopy可能なcode blockとして表示する
- command prompt記号をcopy対象へ含めない
- placeholderは`<project-id>`、`<owner>`、`<repository>`、`<token>`のように一貫させる
- token、個人名、実repository名、実pathをsampleへ含めない
- multiline commandはPOSIX shellでcopyして解釈できる形にする
- destructive commandは前提説明と一緒に示す

### Contribution links

- 全documentにGitHub上のsourceへ向く`Edit page` linkを表示する
- generated pageは初回に作らないため、全linkが実際に編集可能なsourceを指す
- GitHub issue templateの有無を推測せず、一般的なrepository linkだけを常設する

## Content要件

### 正本の優先順位

website contentを作成・reviewするときは次の順でrepositoryの事実を確認する。

1. CLI signature: `tests/snapshots/cli-surface.txt`
2. CLI descriptionとerror wording: `locales/en.ftl`
3. 実際のbehaviorとsafety condition: production sourceとtest
4. platform minimum: `src/commands/status/global/platform/`
5. Docker Sandboxes CLI minimum: `src/compatibility/version/minimum_cli_version.rs`
6. package version: `Cargo.toml`
7. user workflow: `README.md`
8. cross-cutting policy: `docs/design-principles.md`
9. contributor workflow: `docs/development.md`と`scripts/release/README.md`

READMEとsourceが食い違う場合はwebsiteへ都合のよい方を写さない。behaviorをtest/sourceで確認し、
不一致を同じ変更で修正するか、公開を止めて判断を求める。

### 公開後のownership

- websiteを利用者向けdocumentの正本とする
- root READMEはproject概要、requirements、install、最短quickstart、website linkに絞る
- source内の定数・CLI snapshotはmachine behaviorの正本であり続ける
- websiteがsourceから読み取れる値を手書きする場合、CIで差を検知する
- release固有のversion番号をevergreen pageへ固定表示しない。latest releaseはGitHub Releasesへlinkする

READMEの短縮はwebsite実装とは別のreview可能な変更として扱う。

### Writing style

- 公開contentは自然なEnglishで書く
- 一文を短くし、最初に利用者が得る結果を書く
- product固有の用語は`Docker Sandbox`、`Git worktree`、`project ID`など一貫した表記にする
- sourceが区別しているhostとsandboxを省略しない
- safety refusalを「制限」ではなく、保護対象と回復手順が分かる説明にする
- 未実装のfeature、platform、providerを将来予定として約束しない
- marketing表現より観測可能なbehaviorを優先する

## 国際化要件

初回が英語だけでも、monolingual default構成にはしない。Starlightのroot localeを明示して
`root: { label: 'English', lang: 'en' }`として設計する。

- English contentは`website/src/content/docs/`直下に置く
- English routeは`/getting-started/`のようにlocale prefixを付けない
- 将来のJapanese contentは`website/src/content/docs/ja/`へ同じ相対pathで置く
- Japanese routeは`/ja/getting-started/`のように`/ja/`を付ける
- translationの対応は同じrelative pathと同じslugで表す
- locale固有のlinkはhard-codeせず、Starlight/Astroのlocale-aware routeを使う
- Japanese追加時は`ja: { label: '日本語', lang: 'ja' }`をlocale設定へ加える
- Japanese追加前は空の`ja/`、language selector、未翻訳pageへのlinkを公開しない
- custom componentのvisible stringはcomponent内へ散らさず、将来locale dataへ移せる単位で管理する
- date、punctuation、line wrappingをEnglishの長さだけに合わせない
- imageへ説明文を焼き込まない。必要なlabelはHTML textとして描画する

Starlightはroot localeをprefixなしで配信し、別localeをprefix付きで追加できる。また対応pageを
同じfilenameで管理するとfallbackを提供できる。初回構成はこの公式mechanismに沿う。

## Security・privacy要件

- 本物のPAT、credential、private keyをsource、build artifact、screenshotへ含めない
- credential guideではsecret proxyにより実tokenがsandbox外に留まることを説明する
- `GH_TOKEN` placeholderとtokenのhost scopeをREADMEにある正確なcommandで示す
- declared configuration fileへsecretを置かないwarningを必須とする
- external script、tracking pixel、remote fontを初回buildへ含めない
- external linkはlink先が第三者siteであると文脈から分かるlabelにする
- dependencyはlockfileへ固定し、installはfrozen lockfileで行う
- deployment permissionは`contents: read`、`pages: write`、`id-token: write`の必要範囲に限定する

## Accessibility要件

- WCAG 2.2 AAをtargetとする
- semantic heading順を守り、page titleの次をh2から始める
- keyboardだけでnavigation、search、theme selector、copy buttonを操作できる
- focus indicatorを消さない
- light/dark双方でtextとinteractive controlのcontrastを満たす
- 色だけでwarning、success、current stateを表さない
- meaningful imageには内容を示すalt textを付け、decorative imageは空altにする
- animationは必須情報を持たせず、`prefers-reduced-motion`を尊重する
- 320 CSS px相当でもhorizontal page scrollを発生させない。code block内部のscrollは許容する

## Performance・compatibility要件

- JavaScript無効でもdocument本文、navigation link、code exampleを読める
- static buildを維持し、SSR adapterを導入しない
- initial pageへ不要なclient-side frameworkを追加しない
- Tailwindはbuild時のCSS生成にだけ使い、Tailwindを理由にclient-side JavaScriptを追加しない
- local assetを使い、layout shiftを避けるためimage dimensionを確定する
- Chromium、Firefox、Safariのcurrent majorを基本targetとする
- Lighthouseのproduction build測定でPerformance、Accessibility、Best Practices、SEOを各90以上にする
- Pagefindを含むため、searchはdev serverではなくproduction buildのpreviewで検証する

Lighthouse scoreは回帰検知の補助であり、個別のaccessibility検査を置き換えない。

## SEO・metadata要件

- production originをAstroの`site`へ明示する
- 全pageに固有のtitleとdescriptionを付ける
- canonical URL、Open Graph title/description/image、faviconを提供する
- sitemapとrobots.txtを生成し、draft pageを含めない
- root HTMLの`lang`は初回`en`、将来の日本語pageは`ja`とする
- GitHub Pagesのrepository subpathとcustom domainのどちらでもasset/linkが壊れないbase path設計にする
- preview deploymentを検索engineにindexさせない

## 未決定事項

deployment追加前にownerが決める必要があるのはproduction originだけとする。custom domainが
決まっていなければ`https://crescware.github.io/sbxm/`を使う。contentとlocal buildはこの決定を
待たずに進める。

初回のvisual identityは`sbxm`のtext wordmarkとsimpleなlocal faviconで成立させる。専用logoは
必要になった時点で別に設計し、初回実装のblockerにしない。未決定のcustom domainを推測した
DNS設定は作らない。

## 完了条件

- `website/`だけをcheckoutしてもlockfileに従ってinstall、type check、buildできる
- route一覧が`website-information-architecture.md`と一致する
- 全9 subcommandがreferenceに存在し、signatureがCLI snapshotと一致する
- requirementsのmacOSとDocker Sandboxes CLI minimumがproduction sourceと一致する
- internal linkに404がない
- external link検査はnetwork failureと404を区別して報告する
- production previewでPagefind searchがcommand名とguide本文を返す
- English pageのURL snapshotがあり、日本語追加時のroute regressionを検知できる
- accessibility、responsive layout、light/dark themeを代表pageで検証する
- build artifactにsecret、absolute developer path、source mapの意図しない公開がない

## 参照した公式資料

2026-08-03時点で、次のStarlight/Astro公式資料に基づく。

- [Starlight internationalization](https://starlight.astro.build/guides/i18n/)
- [Starlight configuration reference](https://starlight.astro.build/reference/configuration/)
- [Starlight frontmatter reference](https://starlight.astro.build/reference/frontmatter/)
- [Starlight CSS and Tailwind](https://starlight.astro.build/guides/css-and-tailwind/)
- [Starlight site search](https://starlight.astro.build/guides/site-search/)
- [Astro: deploy to GitHub Pages](https://docs.astro.build/en/guides/deploy/github/)
