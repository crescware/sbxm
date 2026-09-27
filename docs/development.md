# sbxm開発環境

この文書は、sbxmをsourceからbuildして検証するまでの手順を定める。

sbxmの開発は、何も入っていないsandboxから始まることを常態とする。そのためhost側に何かが
既に入っていることを前提にせず、cloneした状態から必要なものをすべて導入できるようにする。

## 前提

導入を求めるのはmiseだけとする。

- 導入手順は<https://mise.jdx.dev/>を参照する
- Rustもzigもmiseが導入するため、個別に用意しない
- root権限を必要としない

## toolchainの導入

repository rootで次を実行する。

```sh
mise install
```

`mise.toml`が宣言するtoolchainが揃う。

- `rust` — `Cargo.toml`の`rust-version`と同じ版に固定する
- `zig` — linkerとして使う

`clippy`と`rustfmt`はrustの導入に含まれるため、componentを個別に指定しない。

## zigをlinkerに使う理由

Linux向けの`rustc`はlinkerを同梱せず、リンク工程を外部の`cc`へ渡す。glibc向けのリンクには
起動用objectとlibcの開発用fileも要る。どちらも通常はOSのC toolchainが提供する。

C toolchainが無いsandboxでは、この不足によりbuildがリンク工程で失敗する。

zigはclangに加えてglibc向けの起動用objectとheaderを同梱するため、この不足を単体で満たす。
`.cargo/config.toml`は、Linuxの2つのtargetに対して`.cargo/zig-cc`をlinkerとして指定する。
`.cargo/zig-cc`は`zig cc`を呼ぶだけのwrapperであり、cargoがlinkerに単一の実行fileしか
渡せないために置く。

macOSはこの指定の対象にしない。zigでmacOS向けにリンクするにはmacOS SDKを別途要するため、
既存のC toolchainをそのまま使う。

## buildとローカル実行

toolchainを導入したrepository rootで、release binaryをbuildする。

```sh
cargo build --release
./target/release/sbxm --help
```

sourceから直接実行する場合は`cargo run --`の後ろにsbxmの引数を渡す。
hostの要件とDocker Sandboxes環境を変更せずに診断するには、次を実行する。

```sh
cargo run -- status --global
```

`status --global`、`open`、`repair`などの実環境を使う操作は、READMEに記載したmacOS 14以降、
Docker Desktop、Docker Sandboxes CLIが揃ったhostで実行する。Linuxではcompileとtestを実行できるが、
Sandboxへの接続はできない。

## 検証

変更を提案する前に、次を通す。

```sh
mise run check
```

このtaskは整形・lint・test・coverageを順に確認し、1つでも満たさなければそこで止まる。満たす
べき状態は次のとおり。

- `cargo fmt --all -- --check`が差分を報告しない
- clippyのwarningが0件である
- testが全件成功する
- coverageが最低基準を下回らない

整形だけを当てる場合は次を実行する。

```sh
mise run fmt
```

CLIの公開契約を変えた場合は`tests/snapshots/cli-surface.txt`が差分として現れる。意図した
変更であることを確認したうえで、`SBXM_UPDATE_SNAPSHOTS=1`で記録を更新し、その差分をreviewに
含める。

## coverageの母集団

coverageが数えるのは本番codeだけとする。testとtest支援codeは経路を踏ませるために書いたもの
であり、必ず高いcoverageを示す。本番codeと同じ母集団へ入れると、本番codeの不足を相殺して
最低基準を通してしまう。

数えないのは次の4か所とする。

- `tests/` — 統合test
- `src/testing/` — moduleを跨いで使うfixture
- `fake/` — 1つのmoduleの中だけで使うtest支援code
- file名に`_test`を含むcode — unit test

test buildでしか組み立たないcodeは、この4か所のいずれかへ置く。moduleだけでなくitemと
その内部のstatementやexpressionも同じ規約に従う。llvm-covはfile単位でしか母集団を外せない
ため、`Ui::capture`のようなtest専用の構築関数を本番fileへ書くと、それだけで母集団へ入る。

private fieldへ触れる構築関数も本番fileへ置く理由にはならない。子moduleは親が宣言した
ものへ届き、inherent implはcrateのどのmoduleにも書けるため、対象の隣の`_test` fileへ
`impl`ごと置ける。

本番codeで数えないのは、`src/boundary/os/`のOS層だけとする。OS層はOSの呼び出しそのもので
あり、関数の本体は1つの呼び出し式に限られる。`?`のほかに分岐を置けないため、判断をOS層へ
移して基準を逃れることはできない。この形は`tests/architecture.rs`の
`the_os_layer_has_no_branches`が確かめる。OSについての仮定は、隣の`_test` fileが契約testと
して実OSで確かめ、coverageの数字では求めない。

`tests/module_boundaries.rs`が次を確認する。

- 数えるfileに、test buildでしか組み立たないcodeが無いこと
- `test`がitemとその内部のcodeの有無を1人で決めていること。`#[cfg(not(test))]`は数えられる
  のにtestが踏めず、`#[cfg(any(test, ...))]`は本番buildに存在するかどうかが決まらない
- crate rootから辿って、test buildでしか組み立たないfileとOS層だけが外れていること。除外を
  足して本番codeを母集団から外すことも落ちる
- `mise.toml`が渡す`--ignore-filename-regex`が、この4つとOS層だけを綴っていること

## 同じtreeは同じ値を返す

coverageは繰り返し測っても同じ値になることを前提とする。同じtreeで未到達行が動く場合、揺れて
いるのは測定ではなく実装である。testが偶然しか踏まない経路は、踏んだ回だけ到達済みになる。

基準を動かす前に、まず値を安定させる。踏ませたい経路は、競合ではなくtestから踏ませる。

## flakyになりうる要素

実行のたびに順序や時機が変わりうるものを、flakyになりうる要素と呼ぶ。実時間、threadの順序、
並行して動く子process、signal、書いた直後の実行可能file、file lockの取り合いである。完了まで
同期的に待つだけの外部command実行（`output()`）は、順序の競合がないため数えない。

要素を書いてよいfileと種類は、`tests/architecture.rs`の`FLAKY_ELEMENT_PLACES`が持つ。一覧に
無いfileに要素が現れても、一覧のfileに許した種類以外の要素が現れても落ちる。一覧のfileから
要素が消えたら、一覧から外すまで落ちる。一覧は減る方向にしか動かない。

検出はsourceを構文として読み、`use`が導入した名前を完全修飾pathへ戻してから判定する。何を
検出し何を見ないかは`tests/flaky_elements/mod.rs`の冒頭に書く。

要素は、判断のcodeが差し込みで受け取る。実物はOS層が持ち、testは台本どおりに答えるものを
渡す。OS層の実物を名指ししてよいのは、OS層の中と、`OS_LAYER_WIRING`に挙げた本番の配線だけで
ある。ほかの本番codeやtestが名指しすれば、実OSを動かす要素として数える。配線を通して実OSへ
届くtestは名指ししないため数えられない。そうしたtestは、lockなら期限を0にして1度だけ試す
ように、待たずに済む形で書く。

## flakyを疑ったとき

1つのtestを単独で繰り返しても、時機の競合はほとんど再現しない。CIで出る競合は、同じprocessの
中で千件を超えるtestがforkとexecをしながら走り、少ないCPUを取り合う条件で出る。

疑わしいときは、unit testのbinary全体を2 coreに固定し、16 threadで数回走らせる。Linuxでは
次のとおりである。

```sh
cargo test --bin sbxm --no-run
taskset -c 0,1 target/debug/deps/sbxm-<hash> --test-threads=16
```

1行目が`Executable unittests src/main.rs (...)`として示すpathを、2行目へ渡す。修正前のtree
（164c708）では、この条件でCIと同じ2件が落ちる。

これは診断の道具であり、CIでもreleaseでも回さない。何回通っても、flakyでないことの根拠には
ならない。flakyでないことは反復ではなく、flakyになりうる要素を含まない構造で言う。

## 最低基準

基準は`mise.toml`の`coverage` taskが1箇所で持つ。下回るとその時点で止まる。

現在の実測値はこの文書へ書かない。coverageは変更のたびに動くため、書けばその行は次の変更で
古くなる。値が要るときは`mise run coverage`が出す表を読む。

基準の行97%・関数97%・region 95%と、file単位の行90%は、今ある未到達分を当面は落とさずに置く
ための下限であり、testを書かなくてよい範囲を示すものではない。本番codeを足すときは、その経路を
踏むtestを同じ変更で足す。

割合を下限にする限り、新しく足した未到達codeは、同じ変更で足した到達済みcodeが押し上げた分に
隠れる。母集団からtest支援codeを外しても、この相殺そのものは消えていない。
