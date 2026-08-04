# ReleaseからHomebrew tapのformula PRを作る計画

## 目的

[Issue #55](https://github.com/crescware/sbxm/issues/55)に従い、
`scripts/release/release.sh`がGitHub Releaseを作成した後、同じRelease assetのURLと
SHA-256を使って`crescware/homebrew-tap`の`Formula/sbxm.rb`を更新し、review用のPRを
作るところまで自動化する。

通常のリリースで人が行う作業は、sbxm側のversion変更をmainへ入れること、
`release.sh`を実行すること、sbxm側のversion変更PRと生成されたtap PRをmergeすることに
限る。tapの`main`へ直接commitまたはpushする機能は作らない。

## 現状

- `release.sh`はarchiveを作り、`record_provenance`内でSHA-256を計算し、tagのpush後に
  `gh release create`を実行する。
- `--dry-run`はtag、push、Release作成を省略する一方、archiveとrelease notesは`dist/`へ
  作り、予定していたpublish commandを表示する。
- `crescware/homebrew-tap`の`Formula/sbxm.rb`には、インデント2文字の`url`行と`sha256`行が
  それぞれ1行だけある。`v0.0.1`ではこの2行を人が更新した。
- tap repositoryには現在CI workflowがない。release script自身が、commit前に変更範囲と値を
  検証する必要がある。
- shell script用の永続的なtest harnessはない。release script追加時の検証は、fake commandと
  scratch Git repositoryを使ったend-to-end testとして手動実行された。

## Scope

### 含める

- Release assetのURLとSHA-256を1組のrelease metadataとして確定する
- script専用cacheへtapをcloneし、既存cloneは安全性を確認してfetchする
- `origin/main`から`chore/bump-sbxm-to-<tag>`を作る
- `Formula/sbxm.rb`の`url`と`sha256`だけを置換し、置換結果を検証する
- formulaだけをcommitしてbranchをpushし、非対話で`gh pr create`を実行する
- dry run、各失敗段階、再実行不能な部分に対する明確なreportを用意する
- release手順書と自動testを更新する

### 含めない

- tapの`main`への直接push、PRの自動merge、既存branchへのforce push
- formulaの`version`、依存関係、test blockなど、`url`と`sha256`以外の変更
- Homebrew本体のinstall、`brew install`、`brew audit`をrelease scriptの新しい前提にすること
- GitHub ActionsやGitHub Appへrelease処理を移すこと
- 一般化された任意のtap repository・formulaを更新する機能

## 完成時の処理順

remoteへ影響する境界は次の順に固定する。

1. 現行どおりsbxm repository、version、host、toolchainを検査する
2. `mise run check`、build、署名、archive検証を実行する
3. archiveのSHA-256を一度だけ計算し、release notesにも同じ値を書く
4. tagを作って`origin`へpushする
5. `gh release create`でReleaseとassetを公開する
6. 公開されたassetを`gh release view --json assets`で読み、名前、状態、URL、digestを検証する
7. ここで初めてtapのcloneまたはfetchを行う
8. `origin/main`からbranchを作り、formula候補を生成して検証する
9. formulaだけをcommitし、そのbranchだけをpushする
10. `gh pr create`で`main`向けPRを作り、PR URLを表示する

`create_github_release`または6のasset検証が失敗した場合、7以降を呼ばない。tapのcache、local
branch、remote branch、PRのいずれも作らない。

## Release metadataの正本

`record_provenance`の中だけに閉じているchecksum計算を、archive検証直後の独立した処理へ出す。

- `shasum -a 256`は1回だけ実行する
- 64文字の小文字16進数だけをcanonicalな`archive_sha256`として保持する
- release notesのchecksum行とformulaの`sha256`へ同じ変数を渡す
- formula側がrelease notesをparseしたり、同じarchiveを再度hashしたりしない
- asset名は既存の`ARCHIVE_NAME`を使い、文字列を別の場所へ複製しない

Release作成後は、同名assetがちょうど1件あり、`state`が`uploaded`、GitHubが返す
`digest`が`sha256:<archive_sha256>`であることを確認する。formulaの`url`には、そのassetの
`url`をそのまま使う。これにより、組み立てたURLの推測ではなく、今作ったReleaseが実際に公開した
assetをformulaが参照する。

dry runではReleaseが存在しないため、現在のsource repository名、tag、`ARCHIVE_NAME`から予定URLを
組み立て、local archiveのSHA-256と一緒にreportする。

## 公式repositoryとforkの境界

tapの自動更新対象は`crescware/homebrew-tap`に固定する。既存READMEが説明しているforkでの
Release作成を壊さず、かつforkのasset URLを公式tapへ誤って入れないため、現在のsource
repositoryを`gh repo view --json nameWithOwner`で明示的に取得する。

- `crescware/sbxm`での実行だけがtap更新を有効にする
- forkでの実行は従来どおりfork側のReleaseまで行い、tap更新をskipしたことを明示する
- dry runも同じ判定とし、公式repositoryでだけtap PRの予定を表示する
- repositoryのownerやtap名をremote URLの文字列から推測しない

この分岐と、公式tapへpushできるGitHub権限が追加で必要になることを
`scripts/release/README.md`へ記載する。

## Tap checkoutの管理

cloneの再利用を成立させるため、毎回消える`WORK_DIR`や`dist/`には置かない。macOS向けの
script専用cacheとして、次のdirectoryを使う。

```text
${XDG_CACHE_HOME:-$HOME/Library/Caches}/sbxm/release/homebrew-tap
```

このdirectoryはtap開発者の通常のcheckoutとは別物とし、release scriptだけが使う。

- directoryが無ければ、Release成功後に親directoryを作り、
  `gh repo clone crescware/homebrew-tap <tap-dir>`でcloneする
- directoryがあれば、Git working treeであること、repository rootがそのdirectory自身であること、
  `origin`が`crescware/homebrew-tap`であること、working treeとindexがcleanであることを確認する
- 確認後にだけ`git fetch --prune origin main`を行う
- pathが別用途で存在する、originを同定できない、dirtyである、fetchできない場合は、削除、reset、
  stashをせずに停止する
- branchのbaseはlocal `main`ではなく、取得した`origin/main`のcommitへ固定する

scriptはcache内の既存変更をownership不明の状態で上書きしない。正常終了後もcloneは残し、次回は
fetch経路を使う。branch名はtagごとに一意なので、過去のcleanなlocal branchが残っていても新しい
releaseのbaseには使わない。

## Branchと衝突の扱い

branch名はIssueの指定どおり、tagを含めて次の形にする。

```text
chore/bump-sbxm-to-v0.0.2
```

branchを作る前にlocal refと`origin`の同名refを別々に確認する。既に存在する場合は、どちらかを
正しいものと推測して再利用せず、commit SHAとcache pathを表示して停止する。branchの削除や
force pushは行わない。

新規branchは`origin/main`から作る。pushは`HEAD:refs/heads/<branch>`という明示的なrefspecを使い、
`main`をpush対象に含めない。

## Formulaの置換と検証

編集前に`Formula/sbxm.rb`が次の契約を満たすことを確認する。

- `url "..."`行がちょうど1行ある
- `sha256 "..."`行がちょうど1行あり、既存値も64文字の小文字16進数である
- `url`がsbxmのGitHub Release assetを指し、asset basenameが`ARCHIVE_NAME`と一致する
- checkoutはformula以外も含めてcleanである

元fileを直接in-place編集せず、`WORK_DIR`へcandidateを作る。`awk`などmacOS標準のtoolだけで、
一致した2行を期待する完全な行へ置き換え、その他の行はそのまま出力する。candidateに対して次を
すべて確認してからformulaへ反映する。

1. 新しい`url`行と`sha256`行がそれぞれちょうど1行ある
2. parseした値がRelease asset URLと`archive_sha256`へ完全一致する
3. 対象2行を除いた元fileとcandidateがbyte単位で一致する
4. Git diffの対象fileが`Formula/sbxm.rb`だけである
5. diffが2行削除・2行追加だけで、`git diff --check`が成功する
6. staged diffにも同じ条件が成立し、unstagedの変更が残っていない

1つでも満たさなければcommitもpushもしない。`brew`を新しい実行時依存にせず、変更範囲の狭さと
GitHub上のasset metadataとの一致をscript自身で保証する。

## Commit、push、PR

commitとPRのmetadataはtagから決定的に作る。

- commit/PR title: `Bump sbxm to 0.0.2`
- base: `main`
- head: `chore/bump-sbxm-to-v0.0.2`
- PR body: source Releaseへのlink、asset URL、SHA-256、自動生成であること

`gh pr create`には`--repo crescware/homebrew-tap`、`--base main`、`--head <branch>`、`--title`、
`--body-file`をすべて渡し、promptやcurrent directoryから値を補完させない。body fileは
`WORK_DIR`へ作る。成功時は返されたPR URLを最後のrelease summaryへ含める。

## Dry run

既存のdry runと同じく、archiveとrelease notesは`dist/`へ生成して検査できる状態を保つ。一方、
tapについてはread/writeを分けず、clone、cache作成、fetch、branch作成、formula編集、commit、push、
PR作成を一切実行しない。

`report_dry_run`へ次を追加する。

- 対象repositoryとformula path
- cloneまたは既存cloneのfetchを行う予定であること
- 作るbranch名とbase branch
- 設定予定の完全なasset URLとSHA-256
- commit message
- pushするrefspec
- 実行予定の非対話な`gh pr create` command

現在のRelease作成予定と同じreport内で、tap処理がRelease成功後にだけ行われることが読める順に
表示する。dry run前後でtap cacheとtap remote refsが変わらないことをtestする。

## 失敗時に残す状態と回復案内

tap処理はRelease公開後にしか始められないため、失敗しても作成済みReleaseやtagを自動で戻さない。
外部状態を推測してrollbackすると、公開済みassetを参照し始めた利用者と競合するためである。

| 失敗箇所 | 残る状態 | reportする内容 |
| --- | --- | --- |
| Release作成 | 現行どおりremote tagだけが残り得る。tapは未接触 | 既存のtag取消command |
| Release asset検証 | Releaseとtagが残る。tapは未接触 | Release URLと不一致内容 |
| tap clone/fetch/編集検証 | Releaseとtagが残る。tap remoteは不変 | cache pathと失敗した検証 |
| tap commit | Releaseとtag、local formula変更が残り得る | cache pathと`git status`確認案内 |
| tap push | Releaseとtag、local commitが残る。tap remote branchは通常不変 | branch名と再push command |
| PR作成 | Release、tag、tap remote branchが残る | 完全な`gh pr create`再実行command |

同じtagでrelease script全体を再実行すると、既存Releaseの検査で拒否される。tap段階の例外的な
失敗は、reportしたcacheとcommandから続ける方針とし、このIssueではReleaseをskipしてtapだけを
再開する新しいCLI optionは追加しない。

## 変更するfile

### `scripts/release/release.sh`

- repository、tap、cache、formulaに関する定数と状態を追加する
- checksum計算を`record_provenance`から分離し、同じ値を後続処理へ渡す
- Release assetのURL・digest検証を追加する
- tap checkout、branch衝突検査、formula candidate生成・検証、commit/push、PR作成を責務ごとの
  小さなfunctionへ分ける
- `publish`はRelease成功後にだけtap workflowを呼ぶ
- `report_dry_run`と成功・失敗reportを拡張する
- macOS同梱のBash 3.2と標準toolで動く範囲を維持する

### `scripts/release/README.md`

- 目的と実行順へtap PR作成を追加する
- tapへのwrite権限、cache location、fork時のskipを前提・権限節へ追加する
- dry runの表示内容、正常終了時の成果物、失敗時に残る状態と回復commandを更新する
- 人が行う通常のリリース手順をIssueの完了条件に合わせる

### `scripts/release/release_test.sh`（新規）

- fake commandとscratch source/tap repositoryを組み立てるBashのend-to-end testを置く
- networkや実際のGitHub repositoryへ接続せず、git ref、formula diff、command log、reportを検証する
- test用cacheは一時的な`XDG_CACHE_HOME`へ隔離する

### `mise.toml`

- `release-test` taskを追加し、`mise run check`へ含める
- test中のrelease scriptが呼ぶ`mise run check`はfake `mise`で記録・成功させ、task自身への再帰を
  起こさない

## 実装phase

### Phase 1: Release metadataを一本化する

1. archive SHA-256をmain flowで一度だけ計算する
2. release notesが渡されたdigestから従来と同じchecksum行を作るよう変更する
3. source repositoryの同定、公式tapを更新する条件、dry-run用asset URL生成を追加する
4. Release作成後のasset URL・digest照合を追加する

このphaseではtap repositoryへ書き込まない。既存Release生成のoutputと失敗時のtag挙動を回帰testで
固定する。

### Phase 2: Tapを安全にcheckoutする

1. script専用cacheのclone/fetchを実装する
2. existing path、origin identity、cleanliness、`origin/main`を検査する
3. local/remoteのbranch衝突を観測し、安全が確認できない状態では停止する
4. Release失敗とdry runからcheckout functionへ到達しないtestを先に通す

### Phase 3: Formulaをtransactionalに更新する

1. 元formulaの構造を検査する
2. candidate生成、値照合、非対象行のbyte比較を実装する
3. candidateを反映し、working treeとstaged diffの範囲を検査する
4. 欠落、重複、不正checksum、余分な変更を持つfixtureでcommit前に止まることを確認する

### Phase 4: Branchを公開してPRを作る

1. 決定的なcommit messageとPR bodyを生成する
2. formulaだけをcommitし、明示refspecでbranchをpushする
3. 完全指定の`gh pr create`を実行し、PR URLを最終reportへ渡す
4. push失敗とPR作成失敗の残存状態・回復案内をtestする

### Phase 5: 文書化とrelease rehearsal

1. release READMEの処理順、dry run、権限、cache、失敗表を実装と一致させる
2. `bash -n scripts/release/release.sh`と`mise run check`を通す
3. Apple Silicon macOSで公式repositoryに対するdry runを行い、tapに差分がないことを確認する
4. `v0.0.2`の実リリースで生成されたtap PRが、期待した2行だけを変更していることを確認する

## 自動test scenario

最低限、次をscratch source repository、bare source origin、bare tap origin、fake `gh`で固定する。

### 正常系

- 新規cacheではRelease作成後にcloneし、`origin/main`から指定branchを作る
- 既存cacheではcloneせずfetchし、更新された`origin/main`をbaseにする
- release notesとformulaに同じSHA-256が入り、formula URLがfake Release asset URLと一致する
- tapの`main` refは変わらず、指定branchにformulaだけのcommitが1つ増える
- `gh pr create`のrepo、base、head、title、bodyが完全指定される
- event logが`gh release create`、asset検証、tap checkout、tap push、PR作成の順になる

### Dry run

- tag、source remote、tap cache、tap remoteのいずれも変わらない
- `gh release create`、tap clone/fetch、commit、push、`gh pr create`が呼ばれない
- reportに予定URL、実checksum、branch、formula path、commit、PR commandが出る
- forkではtap更新をskipする理由が出て、公式tap名を使ったcommandが出ない

### Release境界

- `gh release create`が失敗したら、tap関係のcommandが一度も呼ばれない
- asset名、state、URL、digestのいずれかが期待と違えば、tap checkout前に停止する
- tap処理が失敗してもsourceのReleaseとtagを削除しない

### Formula guard

- `url`または`sha256`の欠落・重複を拒否する
- 既存`sha256`の形式が不正なら拒否する
- asset basenameが違う既存`url`を別のformulaとして拒否する
- candidateの値が期待値と違う場合を拒否する
- 対象2行以外の差分があれば拒否する
- staged fileがformula以外にもあれば拒否する

### Git・GitHubの失敗

- cacheがnon-Git、別origin、dirtyの各状態で既存内容を変更しない
- localまたはremoteの同名branchがあれば削除・再利用・force pushをしない
- fetch、commit、push、PR作成の各失敗を非0で返し、段階に合った回復案内を出す
- PR作成失敗後はremote branchが残り、表示されたcommandが同じbase/head/title/bodyを使う

## 完了条件

- `release.sh --dry-run ... v0.0.2`がtapを変更せず、予定するformulaの2値とPR操作を表示する
- 本番実行ではRelease成功後にだけtapを更新する
- 生成されたtap PRのdiffが`Formula/sbxm.rb`の`url`と`sha256`の2行だけである
- formulaのURLが作成済みRelease assetを指し、SHA-256がlocal archiveおよびGitHub asset digestと
  一致する
- tapの`main`はPR mergeまで変わらない
- 通常系でSHA-256の転記とtap repositoryの手操作が不要になる
- 異常系で、Releaseとtapのどこまでが作成されたか、および次に実行できるcommandが判別できる
- `bash -n scripts/release/release.sh`と`mise run check`が成功し、macOS上のdry runでもtapが不変である
