# file別の行coverageを90%以上にする計画

## この文書の位置づけ

`plans/file-line-coverage-80.md`が定めた80% floorは、branch
`chore/plan-file-coverage-80`で導入まで終わっている。本文書は、そのbranchで続けて行う
90%への引き上げの残作業を定める。80%側の文書は履歴として残し、書き換えない。

## なぜ80で止めないか

80をagentへの目標として渡した結果、80や90に届いた時点で手が止まったfileが出た。floorは
下限であって目標ではない。書けるtestを書かずに下限で止めることは、下限の意味を目標へ
すり替えることである。

到達不能なcodeは、testを書く対象ではなく削除する対象である。floorを上げると削除以外に
通す手段が無くなるが、それは欠点ではなく、この引き上げで得たい結果そのものである。

## 決定事項

- coverage taskの全体floorを行97%、関数97%、region95%にし、file別行floorを90%にする。
  `--fail-under-file-lines`は90%を維持する。
- 判定の単位はfileとその契約であり、行ではない。行単位で埋めると行の形をしたtestになる。
- 未到達行が残るfileは、fileごとに1度だけ決める。未testの契約が残っているのか、死んだ
  codeなのか、実hostを要するのか。
- 死んだcodeは削除する。削除で失われる契約があるなら、それは「契約が出力されていない」と
  いうbugであり、codeを消さずに報告する。
- production fileのallowlistや個別除外は設けない。除外regexの4規約も変えない。

## cargo-llvm-covのfloorは「以下」で落ちる

`cargo-llvm-cov 0.8.7`の`--fail-under-file-lines`は、fileのcoverageが閾値と等しいときにも
失敗する。実測で確認した。

| 閾値 | 最小のfileが80.00%のときの結果 |
| --- | --- |
| 79.99 | 成功 |
| 80 | 失敗 |

したがって`--fail-under-file-lines 90`は「90%以上」ではなく「90%超」を要求する。ちょうど
90.00%のfileは通らない。返済の完了判定にはこの1点を織り込む。

## 現況

`537beb6`（80% floor導入まで）の時点で、434 fileを測って次のとおりである。

| 指標 | 値 |
| --- | ---: |
| 全体の行coverage | 96.59% |
| 全体の関数coverage | 95.33% |
| 全体のRegion coverage | 94.35% |
| 90.00%以下のfile | 33 |
| 未到達行を残すfile | 91 |
| 未到達行の合計 | 353 |

`mise.toml`は全体floorを行97%、関数97%、region95%、file別行floorを90%とする。architecture
testは各floorがこの下限を下回らず、同じoptionを2度渡していないことを検査する。

## WIPの扱い

本文書と同じcommitに入っていた`WIP`は、`git reset --soft`で未コミットへ戻した。64 fileの
変更はreviewし、production codeの変更は契約・error semanticsで説明できるものに絞った。
testとcoverageは現行の基準で通過している。変更は引き続き未コミットであり、subsystem単位
への整理は別途行う。

## 残手順

### 手順1. WIPを整理する

1. `WIP` commitの64 fileをreviewする。
2. production codeの変更は、依存境界またはerror semanticsで説明できるものだけ残す。
   coverage値を理由にした変更は落とす。
3. subsystem単位のcommitへ組み直す。

完了条件: `WIP`という名のcommitが残っていない。

### 手順2. 未到達をfileごとに決める

90.00%以下の33 fileを、fileが持つ契約の単位で埋める。90%を超えていて数行だけ残るfileは、
そのfileについて1度だけ、covered・削除・実host必須のいずれかを決める。

完了条件: 全fileが90.00%超であり、残った未到達行にfile単位の理由が付いている。

### 手順3. floorを確定する

`mise.toml`の全体floorを行97%、関数97%、region95%、file別行floorを90%にする。
architecture testの下限も同じ値へ書き換える。optionの削除、値の引き下げ、後ろでの上書きの
それぞれで落ちることを確認する。

完了条件: `mise run check`が通り、意図的に90%以下のfileをreportへ入れると落ちる。

### 手順4. 文書を合わせる

`plans/file-line-coverage-80.md`は履歴として残す。現行のfile別基準が90%、全体基準が
行97%・関数97%・region95%であることは本文書が持つ。

### 手順5. 繰り返しを確認する

`mise run check`を同一commitで3回連続して成功させ、各fileの対象行数・到達行数が一致する
ことを確認する。あわせて、branchの各commitが単体で通ることを確認する。

完了条件: 3 reportが一致し、branch上に落ちるcommitが無い。

## 別件として残す不具合

`src/paths/scope/path_scope.rs`の所有者診断は、説明文を`path`と`observed`だけで組み立てる。
`locales/en.ftl`と`locales/ja.ftl`の`security-*-owner-description`は`$expected`も参照する
ため、この診断は本文の代わりに`message-format-failed`を描画する。`expected`はremediationへ
しか渡していない。

`/tmp/docker-sandboxes`がroot所有である環境で`rebuild`を動かすと踏む。coverage引き上げとは
別の修正として扱う。
