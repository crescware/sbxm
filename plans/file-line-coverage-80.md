# file別の行coverageを80%以上にする計画

## この文書の位置づけ

この文書は、通常のcoverage taskへ`--fail-under-file-lines 80`を追加し、coverage対象の
本番fileをすべて行coverage 80%以上にするまでの手順を定める。test追加、production codeの
変更、coverage gateの変更自体は、この計画branchでは行わない。

基準値を36%などの現在最低値へ合わせる移行はしない。導入するfile別floorは最初から80%と
し、既存の不足を80%以上へ返済してからgateを有効にする。

## 決定事項

- file別の初期基準は行coverage 80%とする。
- 現在の全体基準、行90%・関数90%・Region 88%は維持する。
- `--fail-under-file-lines 80`を全体基準の補助ではなく、すべての本番fileが満たす必須条件と
  する。
- production fileのallowlistや個別除外は設けない。
- 小さいfileも例外にしない。例えば実行可能行が5行なら4行以上、4行以下なら実質100%が
  必要になることを受け入れる。
- 実行可能行を持たずcoverage reportに現れないtrait宣言等は、file別floorの対象外である。
- `tests/`、`src/testing/`、`fake/`、`*_test*.rs`を除外する現在の母集団規約は変えない。
- test支援codeを本番fileへ戻す、除外regexを広げる、到達済みの無意味なcodeを足す、といった
  分母・分子の操作で80%を満たさない。
- 80%を達成したあとは、別計画で90%への引き上げとfile別no-regressionを扱う。

## なぜ全体基準だけでは足りないか

全体比率は、よくtestされたfileの行で別fileの未到達行を相殺できる。全体の行coverageが90%を
超えていても、0%や一桁%のfileを混在させられる。file別floorは、各fileがそれ自身のtestを
持つことを要求し、この相殺を止める。

一方、file別floorだけでは、80%を超えたfileの劣化や、変更行だけの未到達を完全には止めない。
そのため全体基準を残し、80%導入後はdiff coverageまたはfile別no-regressionを次のratchetと
する。

## 現在の参考baseline

PR #48のhead `c04da7d`で、固定済みの`cargo-llvm-cov 0.8.7`を使って測った参考値である。
計画branchは`origin/main`から分けるため、この数値は実装開始時に必ず取り直す。

| 行coverage | 80%未満のfile数 |
| --- | ---: |
| 0–39.99% | 4 |
| 40–49.99% | 9 |
| 50–59.99% | 9 |
| 60–69.99% | 9 |
| 70–79.99% | 21 |
| **合計** | **52** |

90%未満は90 fileである。80%導入の完了までは、90%未満のうち80%以上のfileを今回の返済対象に
含めない。

## 完了条件

次のすべてを満たしたとき、80%導入を完了とする。

1. coverage対象としてreportに現れる全production fileの行coverageが80%以上である。
2. `mise.toml`のcoverage taskに`--fail-under-file-lines 80`が入っている。
3. 全体の`--fail-under-lines 90`、`--fail-under-functions 90`、
   `--fail-under-regions 88`が維持されている。
4. architecture testがfile別基準80と既存のcoverage除外規約を固定し、基準の引き下げと余計な
   除外を拒否する。
5. `mise run check`が同一commitで3回連続して成功し、各fileの対象行数・到達行数が一致する。
6. 追加したtestが行を踏むことだけでなく、戻り値、diagnostic、error伝播、mutation後の状態の
   いずれかを仕様として検査している。
7. 通常checkがCIのrequired checkとして実行され、localで任意に実行するだけのgateではない。
8. baseline用の一時的なratchetを導入した場合、80% gateと役割が重複する部分を削除する。

## 測定方法

### 1. 母集団を固定する

実装開始前に、次を確認する。

- PR #48までのcoverage母集団修正とprompt分離がmerge済みである。
- command runner等の非決定的なcoverageが解消済みである。
- `tests/module_boundaries.rs`が、本番buildのfile集合とcoverage対象file集合の一致を検査して
  いる。
- Rust、`cargo-llvm-cov`、feature集合、除外regexがrepositoryで固定されている。

### 2. machine-readableなbaselineを作る

通常testを1回実行したあと、JSON reportを生成する。

```sh
cargo llvm-cov report \
  --json \
  --output-path target/file-coverage.json \
  --ignore-filename-regex '(^|/)tests/|(^|/)src/testing/|(^|/)fake/|[^/]*_test[^/]*\.rs$'
```

各fileについて、少なくとも次を記録する。

- production相対path
- executable lines
- covered lines
- line coverage percent
- missing lines

同一commitで3回測り、fileごとの分母と分子が一致しなければ返済を始めない。揺れるtestまたは
process lifecycleを先に修正する。

### 3. 返済中の逆戻りを止める

全52 fileを直すまでは80% gateを有効にできない。その期間に不足を増やさないため、一時的な
baseline ratchetをCIへ置く。

- baseline時点で80%以上のfileは80%未満へ落とせない。
- baseline時点で80%未満のfileは、covered / executableの比率を下げられない。
- 新規production fileは追加時点から80%以上を必須とする。
- 変更したbacklog fileは、そのPRで80%以上へ上げる。
- 80%未満のfile総数を増やさない。

ratchetはJSON report同士を数値で比較する。表示文字列へ依存せず、percentの丸め値ではなく
covered linesとexecutable linesから比較する。

## 返済対象の分類

coverageの低い順だけでPRを分けない。同じ依存境界と同じ失敗意味を持つfileをまとめ、次の
順序で扱う。

### A. 端末・外部processを必要としない純粋処理

CLI値の変換、configのserialization、i18n、表示幅、diagnostic mapping等から始める。

- 入力と出力のtable testを優先する。
- error値はID、exit code、保持するcontextまで検査する。
- 既存のprivate itemへ届くtestは、規約どおり`*_test.rs`の子moduleへ置く。
- 小さいfileは80%に丸めるのではなく、意味のある全分岐を100%へ近づける。

### B. composition rootとstatus probe

`commands/*/exec.rs`、status check、host queryを扱う。

- `HostEnvironment`、prompt、`Ui::capture`、既存fakeを使い、実hostへ触れずに分岐を固定する。
- locale設定、promptの有無、reportされるerror、hostへ渡すcommandを検査する。
- successだけでなく、settings読込失敗、host失敗、部分的な観測結果を含める。
- 依存を関数内で組み立てていてtestできない場合だけ、composition rootへ押し出す。

### C. filesystem、atomic operation、lock

directory、atomic replace、lock、project pathを扱う。

- `tempfile`上で成功、既存file、symlink、permission、競合、途中失敗を分けてtestする。
- destructive mutationは、失敗値だけでなく、元file、temporary file、lock、registry等の最終状態を
  検査する。
- cleanup errorを無視する場合は、その意味を明記してtestする。
- OS固有で通常testに置けない契約は、対象platformのacceptance taskへ分ける。

### D. image、repository、sandbox、command lifecycle

外部commandの出力を読むfileを扱う。

- fake hostで正常出力、非zero status、不正UTF-8、不正形式、read failureを固定する。
- timeout、signal、reader、pipeの終了契約は、process treeが残らないところまで検査する。
- 外部toolの出力変更を空値や既定値へ落とす場合は、意図したfallbackかerrorかを決める。
- 実Docker、git、`sbx`が必要な契約は通常coverageと混ぜず、実環境acceptanceへ残す。ただし
  production adapter自体のerror伝播は通常testで80%以上へ到達させる。

### E. 実terminal adapter

`design/prompt/real_terminal.rs`を扱う。

- 非TTYのread/write pairで観測できる委譲とerror伝播は通常testで固定する。
- 実TTYの高さ、打鍵、cursor visibility、文字・行消去の効果はPTY testで固定する。
- line coverageだけのためにterminal escape sequenceのcrate内部実装を複製しない。
- PTY testを通常coverageへ含められない場合は、adapterを意味のある単位へ分け、通常test可能な
  部分だけで80%を満たす。production fileのcoverage除外は行わない。

## 実装PRの切り方

1つの巨大PRで52 fileを変更しない。次の制約で小さくmergeする。

- 1 PRは1 subsystem、または同じerror semanticsを持つ数fileまでとする。
- 変更したbacklog fileはそのPRで80%以上にする。途中の60%や70%を完成扱いにしない。
- production codeの構造変更とtest追加を同じPRに含める場合、構造変更の理由をcoverage値では
  なく依存境界またはerror semanticsで説明する。
- 各PR本文に、対象fileの前後のcovered / executable lines、追加した仕様、残る未到達理由を
  記録する。
- 各PRで全体90/90/88と一時ratchetを通す。
- subsystem間で独立なPRは並行可能だが、同じfakeやcomposition rootを変更するPRは順序を
  固定する。

## 段階

### 手順0. prerequisiteを完了させる

1. PR #48をmergeする。
2. coverage非決定性が残っていないことを3回計測で確認する。
3. `mise run check`をCI required checkにする。未導入なら、coverage返済より先に別PRで行う。
4. baselineを取り直し、80%未満fileの確定一覧を作る。

完了条件: 同一commitの3 reportが一致し、CIが通常checkを強制している。

### 手順1. 一時ratchetを入れる

1. file別JSON baselineをrepositoryへ記録する。
2. 既存fileの劣化、新規fileの80%未満、backlog総数の増加を拒否するcheckを追加する。
3. 比較処理自体のtestを持つ。
4. 全体90/90/88はそのまま通す。

完了条件: 80% gate導入前でもcoverage debtを増やせない。

### 手順2. 40%未満の4 fileを80%以上にする

最初の参考対象は次の4 fileである。

| file | covered / executable | 行coverage |
| --- | ---: | ---: |
| `src/config/serialized.rs` | 3 / 11 | 27.27% |
| `src/paths/atomic/temp_path_for.rs` | 8 / 26 | 30.77% |
| `src/cli/help/format.rs` | 3 / 8 | 37.50% |
| `src/paths/directory/ensure_directory.rs` | 10 / 26 | 38.46% |

完了条件: 4 fileがそれぞれ80%以上で、失敗経路と最終状態がtestされている。

### 手順3. 40–59.99%の18 fileを80%以上にする

status probe、command lifecycle、config読込、image、atomic operation、prompt terminal adapterを、
前述の分類単位で分割する。

完了条件: 60%未満のproduction fileが0件になる。

### 手順4. 60–69.99%の9 fileを80%以上にする

config load、repository worktree、status、exec等を扱う。到達済みのhappy pathを増やすのではなく、
未到達のerrorと境界値を仕様として固定する。

完了条件: 70%未満のproduction fileが0件になる。

### 手順5. 70–79.99%の21 fileを80%以上にする

80%直下のfileは、1–数行を踏むだけになりやすい。未到達行を機械的に呼ぶのではなく、その行が
表す分岐、error、fallbackをtest名とassertionに対応させる。

完了条件: JSON report上、80%未満のproduction fileが0件になる。

### 手順6. file別80% gateを有効にする

`mise.toml`を次の形へ変更する。

```toml
[tasks.coverage]
run = '''
cargo llvm-cov \
  --workspace \
  --all-features \
  --summary-only \
  --ignore-filename-regex '(^|/)tests/|(^|/)src/testing/|(^|/)fake/|[^/]*_test[^/]*\.rs$' \
  --fail-under-lines 90 \
  --fail-under-functions 90 \
  --fail-under-regions 88 \
  --fail-under-file-lines 80
'''
```

同じPRでarchitecture testを追加し、次を固定する。

- `--fail-under-file-lines`が存在する。
- 値が80未満ではない。
- 同じoptionを後ろで上書きしていない。
- coverage除外regexが既存の4規約と一致する。
- 全体基準90/90/88が維持されている。

完了条件: `mise run check`が3回成功し、意図的に80%未満のfixtureをreportへ入れた検証ではgateが
対象file名を示して失敗する。

### 手順7. 一時ratchetを整理する

1. 80%未満を管理するだけのbaseline一覧を削除する。
2. 新規・変更fileのno-regressionに引き続き価値がある部分は残す。
3. 最終分布と、90%へ上げるためのbacklogを記録する。

完了条件: 一時的な二重管理がなく、80% floorと全体90/90/88がCIで強制される。

## 参考backlog

以下はPR #48 head `c04da7d`での参考値であり、実装開始時の再計測結果を正とする。

| 行coverage | covered / executable | file |
| ---: | ---: | --- |
| 27.27% | 3 / 11 | `src/config/serialized.rs` |
| 30.77% | 8 / 26 | `src/paths/atomic/temp_path_for.rs` |
| 37.50% | 3 / 8 | `src/cli/help/format.rs` |
| 38.46% | 10 / 26 | `src/paths/directory/ensure_directory.rs` |
| 40.00% | 12 / 30 | `src/commands/status/global/sandboxes/check_remote_ssh.rs` |
| 40.00% | 12 / 30 | `src/commands/status/global/service/check_login.rs` |
| 40.00% | 4 / 10 | `src/cli/project_arg/required_clone_url.rs` |
| 40.00% | 4 / 10 | `src/cli/project_arg/required_project.rs` |
| 45.45% | 15 / 33 | `src/commands/status/project/artifacts/check_dockerfile.rs` |
| 46.88% | 15 / 32 | `src/command/wait_with_limit.rs` |
| 47.06% | 8 / 17 | `src/support/generation/current_dockerfile_hash.rs` |
| 47.37% | 18 / 38 | `src/support/image/ephemeral_context.rs` |
| 47.73% | 21 / 44 | `src/commands/destroy/exec.rs` |
| 50.00% | 6 / 12 | `src/commands/status/project/artifacts/check_archive.rs` |
| 50.00% | 6 / 12 | `src/config/read_existing.rs` |
| 50.00% | 7 / 14 | `src/cli/diagnostics/context_string.rs` |
| 51.16% | 22 / 43 | `src/commands/open/exec.rs` |
| 53.33% | 8 / 15 | `src/design/width/is_wide.rs` |
| 53.57% | 15 / 28 | `src/design/prompt/real_terminal.rs` |
| 55.56% | 15 / 27 | `src/commands/status/project/artifacts/check_image.rs` |
| 56.67% | 17 / 30 | `src/paths/atomic/atomic_rename_into_place.rs` |
| 58.62% | 17 / 29 | `src/commands/status/global/service/check_daemon.rs` |
| 64.00% | 16 / 25 | `src/support/repository/worktree/provision_worktree.rs` |
| 65.38% | 17 / 26 | `src/paths/atomic/replaceable_identity.rs` |
| 66.67% | 14 / 21 | `src/commands/add/ask_language.rs` |
| 66.67% | 24 / 36 | `src/config/load.rs` |
| 66.67% | 4 / 6 | `src/commands/present/observed.rs` |
| 68.42% | 26 / 38 | `src/commands/status/project/repository/worktree_state.rs` |
| 68.97% | 20 / 29 | `src/commands/rebuild/exec.rs` |
| 68.97% | 20 / 29 | `src/support/repository/worktree/verify_mode.rs` |
| 69.57% | 16 / 23 | `src/commands/status/global/settings/check_state_directory.rs` |
| 70.00% | 7 / 10 | `src/i18n/format_failure.rs` |
| 70.59% | 12 / 17 | `src/commands/ls/exec.rs` |
| 70.59% | 24 / 34 | `src/paths/directory/ensure_private_dir.rs` |
| 71.43% | 15 / 21 | `src/support/status/status_value.rs` |
| 72.73% | 16 / 22 | `src/config/config_location.rs` |
| 73.08% | 19 / 26 | `src/commands/prepare/exec.rs` |
| 74.58% | 44 / 59 | `src/archive/read_entry.rs` |
| 74.58% | 44 / 59 | `src/paths/lock/acquire_exclusive_lock.rs` |
| 75.00% | 33 / 44 | `src/commands/status/global/service/check_network_policy.rs` |
| 75.00% | 6 / 8 | `src/i18n/shell_locale.rs` |
| 75.00% | 9 / 12 | `src/commands/status/print/global.rs` |
| 75.00% | 9 / 12 | `src/repository/provider.rs` |
| 75.68% | 28 / 37 | `src/config/save_git_identity.rs` |
| 75.93% | 41 / 54 | `src/commands/status/global/platform/check_platform.rs` |
| 76.32% | 29 / 38 | `src/support/image/ensure_archive.rs` |
| 76.47% | 13 / 17 | `src/commands/status/project/inside/check_secret.rs` |
| 76.67% | 23 / 30 | `src/commands/status/project/inside/check_sandbox.rs` |
| 76.92% | 10 / 13 | `src/paths/atomic/atomic_replace.rs` |
| 78.95% | 30 / 38 | `src/paths/project/project_parent.rs` |
| 78.95% | 45 / 57 | `src/cli/diagnostics/map.rs` |
| 79.55% | 35 / 44 | `src/commands/add/exec.rs` |

## 80%導入後

80%は最終品質ではなく、file間相殺を止める最初の強制基準である。導入後は次を別計画として
扱う。

1. 80%以上90%未満のfileをsubsystemごとに返済する。
2. `--fail-under-file-lines`を85、90へ引き上げる。
3. diff coverageまたはfile別no-regressionを恒久化する。
4. branch coverage / MC/DCを試験計測し、採用可否と理由を記録する。
