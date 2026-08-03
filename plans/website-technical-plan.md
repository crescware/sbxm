# sbxm公式サイト技術計画

## 目的

この文書は、`website-requirements.md`と`website-information-architecture.md`をAstro Starlightで
実現するためのrepository構成、build、deployment、段階的な実装順を定める。これは実装
指示書であり、website本体とrepository rootのdeployment workflowの責務を分けて定める。

## Architecture

### Runtime model

- Astro Starlightによるstatic site generationを使う
- Starlight pageはすべてprerenderする
- searchはbuild時に生成するPagefind indexを使う
- server adapter、API route、database、CMSを使わない
- client-side UIはStarlightが必要とする範囲に留める

### Package boundary

websiteはRust crateと同じrepositoryに置くが、独立したNode packageとする。

- package manifestとlockfileは`website/`が持つ
- dependency install、dev server、build、previewは`website/`をworking directoryにする
- root Rust workspaceへNode-specific設定を混ぜない
- generated `website/dist/`、`.astro/`、Pagefind artifact、dependency directoryはcommitしない
- package managerはpnpmとし、`packageManager` fieldと`website/mise.toml`でversionを固定する
- Node.jsはAstroのcurrent requirementを満たす24 LTSを`website/mise.toml`で固定する

## 想定directory構成

```text
website/
├── README.md
├── mise.toml
├── package.json
├── pnpm-lock.yaml
├── astro.config.mjs
├── tsconfig.json
├── public/
│   ├── favicon.svg
│   ├── robots.txt
│   └── social-card.svg
└── src/
    ├── assets/
    │   └── brand/
    ├── components/
    ├── content.config.ts
    ├── content/
    │   ├── docs/
    │   │   ├── index.mdx
    │   │   ├── getting-started/
    │   │   ├── guides/
    │   │   ├── reference/
    │   │   ├── troubleshooting/
    │   │   └── project/
    │   └── i18n/
    ├── config/
    │   └── sidebar.ts
    └── styles/
        └── global.css
```

`src/content/docs/ja/`は日本語を公開するときに追加する。空directoryやplaceholder translationを
初回buildへ入れない。

Astro/Starlight標準の構成が上記と変わった場合、初回実装時のcurrent official templateを優先し、
この文書の役割境界を保った最小差分にする。

## Dependency方針

### 必須

- `astro`
- `@astrojs/starlight`
- `tailwindcss`
- `@tailwindcss/vite`
- `@astrojs/starlight-tailwind`
- Astroが公式templateで必要とするsupport package

### 条件付き

- `@astrojs/sitemap`: Starlight/Astroのcurrent templateでsitemapが自動提供されない場合

### 初回に追加しないもの

- React、Vue、SvelteなどのUI framework
- analytics SDK
- remote font package
- Starlight community plugin
- icon package。Starlight built-in iconまたはlocal SVGを使う

versionは初回実装時のcurrent stableで相互互換な組を選び、pnpm lockfileへ固定する。major
updateはsite content変更と分けてreviewする。

## Starlight configuration contract

### Core

- title: `sbxm`
- description: repository READMEの一文説明を短くしたEnglish metadata
- social: `https://github.com/crescware/sbxm`
- editLink: `main/website/`以下のcurrent sourceへ向ける
- pagefind: enabled
- prerender: enabled/default
- lastUpdated: enabled。ただしdeployment checkoutで正確なhistoryを取得する
- customCss: Tailwind/Starlight compatibility layerを定義する`src/styles/global.css`を先頭で読む
- vite: Tailwind CSS v4の`@tailwindcss/vite` pluginを使う
- credits: Starlight creditは初回に残す

### Locales

初回から次の意味になる設定を持つ。

```js
locales: {
  root: {
    label: 'English',
    lang: 'en',
  },
}
```

`defaultLocale`はroot localeがdefaultになるStarlight contractに従い、省略するか`root`を明示する。
初回実装時の型とofficial exampleに合わせる。

日本語追加時だけ次を加える。

```js
ja: {
  label: '日本語',
  lang: 'ja',
}
```

この構成によりEnglishはrootのまま、Japaneseは`/ja/`になる。Starlightは同じrelative filenameを
locale間の対応として扱い、未翻訳contentのfallbackを提供する。

### Sidebar

- `src/config/sidebar.ts`の明示配列を唯一のnavigation定義とする
- group labelはEnglishをdefault labelとする
- Japanese追加時にStarlightの`translations` fieldへlabelを加える
- command順は`src/commands/specs.rs`とCLI snapshotのlifecycle順に合わせる
- page追加だけで勝手にnavigationへ現れないようautogenerateへ全面依存しない

## Styling方針

Tailwind CSS v4を採用し、Starlight公式の`@astrojs/starlight-tailwind` compatibility layerを使う。
新規projectはcurrent official `starlight/tailwind` templateを起点にする。独自landing section、card、
diagram、small UI componentはTailwind utilityで実装し、通常のMarkdown proseはStarlightの
document styleに任せる。

`src/styles/global.css`は公式のcascade layer順を保つ。

```css
@layer base, starlight, theme, components, utilities;

@import '@astrojs/starlight-tailwind';
@import 'tailwindcss/theme.css' layer(theme);
@import 'tailwindcss/utilities.css' layer(utilities);
```

- Tailwind v4のCSS-first configurationを使い、不要な`tailwind.config.*`を作らない
- brand color、gray scale、fontは`@theme` tokenとして定義し、Starlight UIにも共有する
- Starlightのdark modeに対応するcompatibility layerを通して`dark:` variantを使う
- reusableな見た目はAstro componentにまとめ、長いutility列のcopyを増やさない
- arbitrary valueはlayout上の固有値だけに限定し、色やspacingはtheme tokenを使う
- global selectorと`!important`はStarlight compatibility修正が必要な場合だけ使う
- Starlightのlayout、theme selector、focus behavior、responsive navigationは置き換えない
- component overrideは要件を満たせない場合だけ行う
- brandはneutralなterminal/documentationの印象を持たせ、派手なgradient animationに依存しない
- system font stackを使いremote font requestを発生させない
- accent colorはlight/dark双方でAA contrastを確認する
- prose幅、code block、table、asideはStarlight defaultを基本とする
- command、path、artifactの関係にだけ小さなdiagramを使う
- screenshotを主要説明に使わない。CLI outputはtext/codeとして選択可能にする
- logo未決定時はtext wordmarkとlocal SVG faviconで成立させる

Tailwindはauthoring toolでありruntime dependencyではない。utilityを使うためにReactなどのUI
frameworkやclient hydrationを追加しない。追加のUI technologyは必要な機能が具体化した時点で
個別に判断し、初回scaffoldへ先回りして含めない。

## Content implementation

### File format

- 通常documentはMarkdownを使う
- landing pageとStarlight componentが必要なpageだけMDXを使う
- custom Astro componentはlocale-aware textとaccessibility上の理由がある場合だけ追加する
- content schemaを拡張し、全pageのfrontmatterで`title`と`description`を必須にする
- draftはfrontmatterの`draft: true`を使いproduction buildから除外する

### Reuseとduplication

利用者が読むworkflowをbuild時にrepository rootのREADMEからincludeしない。includeは編集contextを
分断し、translation対応とlink解決を複雑にするためである。website contentはsite上の文脈で書く。

次の短い事実は、contentを編集する際にsourceと照合して一貫した表記にする。

- subcommand名とoption
- macOS minimum
- Docker Sandboxes CLI minimum
- worktree range
- package install command

長いbehavior説明は自動生成せず、siteの文脈に合わせたhuman-authored contentとして管理する。

## Local command contract

ホームページを開発・公開するための最小限のpackage scriptだけを提供する。実際のpackage managerを
prefixに付けても意味は同じにする。

| Script | 役割 |
| --- | --- |
| `dev` | local development server |
| `build` | production static buildとPagefind index生成 |
| `preview` | production artifactのlocal preview |

`website/README.md`にはfresh cloneからinstall、dev、build、previewまでを記載する。rootの
`docs/development.md`へNode toolchainを混ぜ込む変更はwebsite実装と別にreviewできるようにする。

## Deployment計画

### Default proposal

hostingが未指定のため、GitHub Pagesをdefault proposalとする。Astro公式はstatic siteをGitHub
ActionsでGitHub Pagesへdeployするworkflowを案内している。repository project pageなら初期URLは
`https://crescware.github.io/sbxm/`となり、後からcustom domainを設定できる。

### Workflow boundary

GitHub Pages workflowはGitHubの仕様上repository rootの`.github/workflows/`に置く。
website本体は`website/`に閉じ、deploy workflowはその外側でartifactを受け取る。現在の実装では
`.github/workflows/deploy-website.yml`がこの役割を担う。

workflow要件は次のとおり。

- `main` pushとmanual dispatchで起動する
- Astro公式GitHub Actionのcurrent supported majorをcommit SHAまたは組織方針に沿う形で固定する
- actionへwebsite root pathを明示する
- build jobはread-only contents permissionを使う
- deploy jobだけがPagesとOIDCのpermissionを持つ
- pull requestではdeployせずbuildだけ行う
- GitHub Pages environment protectionがあれば尊重する
- concurrent deploymentは新しいmain buildを優先し、途中artifactを公開しない

### Base URL

- project pageでは`site=https://crescware.github.io`と`base=/sbxm`に相当する設定を使う
- custom domainへ移る場合は`site`と`base`を一箇所で切り替える
- Markdown内で`/`始まりを無条件に組み立てず、Starlight/Astroのbase-aware link処理を使う
- local preview、project page、custom domainの3 contextでasset URLを検査する
- custom domain決定前にCNAMEやDNS recordを推測して作らない

### Last updated

Starlightの`lastUpdated`はGit historyを使い、shallow cloneでは不正確になり得る。workflowは必要な
historyを取得するか、表示を無効にする。誤った日付を表示するより、検証できるまで無効にする。

## SEO・social assets

- Astro `site`決定後にsitemapを生成する
- default social cardはlocal asset一枚とし、textを詰め込みすぎない
- page固有social card生成は初回scope外
- canonicalとalternate locale linkはStarlight/Astroの生成結果を公開前に確認する
- production robotsはindex許可、previewはindex拒否とする
- 404 pageはStarlight標準をbrandに合わせ、searchとgetting startedへのlinkを持たせる

## 実装phase

### Phase 1: Scaffold and contracts

- current official `starlight/tailwind` templateから`website/`をscaffoldする
- package manager/runtime/lockfileを固定する
- Tailwind CSS v4、Vite plugin、Starlight compatibility layerとtheme tokenを設定する
- root English localeを明示する
- sidebar、content schema、base URL strategyを設定する
- dev/build/preview scriptを用意する
- default Starlight pageを削除する

完了条件: placeholder一枚ではなく、正規route構造がproduction buildできる。

### Phase 2: Core journey

- homepage
- requirements
- installation
- quickstart
- daily workflow
- credential safety content

完了条件: fresh user flowをproduction previewでend-to-end reviewできる。

### Phase 3: Guides and reference

- 残りguide
- 全9 command page
- global option、configuration、filesystem、output reference

完了条件: CLI referenceが揃い、READMEの利用者向け情報がsiteで欠落しない。

### Phase 4: Troubleshooting and project docs

- host/project diagnostics
- safety refusals
- design principles
- development entry point
- cross-link review

完了条件: major error categoryから安全な次の観測行動へ辿れる。

### Phase 5: Handoff and release

- production originを確定する
- `.github/workflows/deploy-website.yml`でdeploymentを行い、Pagesのsource設定をownerが承認する
- root READMEをwebsiteへの入口へ短縮する別変更を準備する

完了条件: `website-requirements.md`の完了条件をすべて満たす。

### Phase 6: Japanese addition（将来）

- `ja` localeとsidebar translationを追加する
- priority pageから同じrelative pathで翻訳する
- Starlight UIのbuilt-in Japanese stringsを確認する
- custom strings、Pagefind、alternate URL、fallback noticeを検証する
- English route snapshotが不変であることを確認する

## Riskと対策

| Risk | 対策 |
| --- | --- |
| Japanese追加でEnglish URLが`/en/`へ移る | 初回からroot localeを明示しroute snapshotを持つ |
| custom designでStarlight updateが困難になる | component overrideを最小化しCSS token中心にする |
| GitHub Pages subpathでassetが壊れる | base-aware linkとproject-page preview testを使う |
| `lastUpdated`がshallow cloneで誤る | full historyを取得するか表示を無効化する |
| exampleへcredentialが混入する | placeholderを使い、content reviewで確認する |

## 実装開始前の確認gate

次を確認したらPhase 1を開始できる。

- 要件、情報設計、技術計画の3つのsite planがpage scopeとEnglish-first/Japanese-ready方針を正しく表している

production originはPhase 5まで保留できる。custom domainの未決定をlocal content実装のblockerにしない。

## 参照した公式資料

2026-08-03時点で、次のcurrent official behaviorを前提とする。

- [Starlight: root locale and fallback content](https://starlight.astro.build/guides/i18n/)
- [Starlight: sidebar, Pagefind, prerender, edit links](https://starlight.astro.build/reference/configuration/)
- [Starlight: splash template, hero, draft and metadata](https://starlight.astro.build/reference/frontmatter/)
- [Starlight: Tailwind CSS v4 compatibility](https://starlight.astro.build/guides/css-and-tailwind/)
- [Starlight: Pagefind search](https://starlight.astro.build/guides/site-search/)
- [Astro: GitHub Pages deployment](https://docs.astro.build/en/guides/deploy/github/)
