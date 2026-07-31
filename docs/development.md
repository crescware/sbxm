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

## 検証

変更を提案する前に、次の3つをすべて通す。

```sh
cargo test
cargo clippy --all-targets
cargo fmt --check
```

満たすべき状態は次のとおり。

- testが全件成功する
- clippyのwarningが0件である
- `cargo fmt --check`が差分を報告しない

CLIの公開契約を変えた場合は`tests/snapshots/cli-surface.txt`が差分として現れる。意図した
変更であることを確認したうえで、`SBXM_UPDATE_SNAPSHOTS=1`で記録を更新し、その差分をreviewに
含める。
