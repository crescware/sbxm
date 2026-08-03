# sbxm公式サイト保守タスク

## 位置づけ

この文書は、`website/`のホームページと利用者向けドキュメントが完成した後に、別の保守ブランチで
検討するタスクだけをまとめる。現在のwebsite実装に専用の検証script、CI workflow、外部link監視を
追加することは、この文書の作成時点では行わない。

サイト本体の構成、英語content、Tailwind、Astro build、将来の日本語localeは
[`website-technical-plan.md`](./website-technical-plan.md)で扱う。

## 保守ブランチの原則

- ホームページの見た目と利用者向けcontentを保守タスクで複雑にしない
- Rustの既存checkとwebsiteのbuildを同じscriptへ混ぜない
- 自動化が必要になった場合も、大量の無型`.mjs`を先に増やさない
- scriptを追加する場合はTypeScript module（`.mts`）を第一候補にし、入力・出力・失敗理由を明示する
- machine-readableなsourceを正本とし、website全体の文章を生成しない
- networkへ依存する検査と、ローカルで再現できる検査を分離する

## 保守タスク

### 1. CLI order and reference drift（必須）

`sbxm --help` の `Commands:` blockを正本として、websiteのCLI referenceを照合する`.mts` taskを必ず
実装する。Rust側では [`src/commands/specs.rs`](../src/commands/specs.rs) のvector順がhelpの表示順を
決めているため、sourceと生成されたhelpの不一致も同じ保守taskで報告する。

このtaskは次のすべてを確認し、不一致時は非zeroで終了してmergeを止める。

- `sbxm --help` のsubcommand集合と順番が `website/src/config/sidebar.ts` のCLI項目と一致する
- `src/commands/specs.rs` の順番と `sbxm --help` の順番が一致する
- 全subcommandに対応するreference pageがある
- reference indexに全subcommandへのlinkがある
- synopsisのrequired/optional引数、short option、value nameが一致する
- websiteだけに存在する未知のcommand/optionがない

失敗時はCLI側の順番、sidebar側の順番、最初の差分を表示する。commandの説明、example、安全上の
注意は自動生成せず、人がreviewする。CLI追加・削除・並び替え、reference追加、sidebar変更を含む
pull requestでは、このtaskを必須jobとして実行する。

### 2. Source value drift

requirements pageに表示する最低要件、Docker Sandboxes CLI minimum、worktree range、install commandを
production sourceの限定された定数と照合する。Rust sourceを脆い正規表現で全面parseしない。

### 3. Build route and metadata review

production buildのartifactを対象に、次を確認する。

- 想定routeと404 pageが生成される
- base path、canonical、sitemap、favicon、social metadataが壊れていない
- Pagefind indexが生成され、検索対象にuser-facing documentが含まれる
- English routeが日本語locale追加時に`/en/`へ移動しない

### 4. Link and accessibility review

- internal link、anchor、asset referenceを検査する
- external linkはscheduledまたはmanual jobで検査し、404/410とtimeout、DNS、rate limitを区別する
- representative routeをkeyboard、screen reader、mobile幅、light/dark themeで確認する
- transientな外部link failureでcontentを自動変更しない

### 5. Secret and artifact review

- sampleに本物のtoken、private key、個人pathが入っていない
- build artifactにsecret patternやabsolute workspace pathが残っていない
- external script、tracking pixel、remote fontが意図せず追加されていない

### 6. CI / deployment integration

ownerがworkflow追加を承認した後に、次を別PRで実装する。

1. `website/`でmiseのNode/pnpmを用意する
2. frozen lockfileでdependencyをinstallする
3. `mise exec -- pnpm build`を実行する
4. 必要なlink、accessibility、secret検査を独立jobとして追加する
5. deploy jobだけにPagesとOIDC permissionを与える

Rustの`mise run check`とは独立したjob名にする。GitHub Pagesのdeploy workflowを追加するまでは、
website実装は静的buildまでに留める。

## 着手条件

- homepageと必須ドキュメントのcontentが公開候補として固まっている
- sourceとwebsiteのownershipが明確になっている
- ownerが保守ブランチのscopeとCI実行時間を承認している
- 追加する検査が、手書きの大型fixtureや大量の重複contentを要求しない

## 完了条件

- 保守taskはwebsite本体から独立してreview、rollbackできる
- `.mts` taskの失敗時に対象page、source、差分が分かる
- CLI order and reference drift taskが、help・Rust source・sidebarの不一致をmerge前に検出する
- network検査の不安定性がPRのbuild結果を隠さない
- Japanese locale追加後もEnglish route、検索、metadataの扱いが明確である
