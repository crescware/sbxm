# release

この文書は、`release.sh`が何を行い、どの順で行い、失敗したときに何を残すかを定める。

`release.sh`は、Apple Silicon macOS向けのbinaryをbuildし、tagを打ち、GitHub Releaseを
作成する。tagを打つところからRelease作成までを1つのcommandで行い、途中を手作業に委ねない。

## 前提

- macOS 14以降のApple Silicon機
- Xcode Command Line Tools — `codesign`と`file`を使う
- mise — toolchainは`mise install`で揃う
- gh CLI — 認証済みであること (`gh auth login`)

このscriptはmacOSでしか完走しない。arm64であること、`rustc`のhostが
`aarch64-apple-darwin`であることを検査し、満たさなければ止まる。

## 使い方

releaseするversionを`Cargo.toml`へ入れてcommitしておく。scriptは`Cargo.toml`の
`version`とtagのversionが一致することを要求する。working treeがcleanであることも要求する
ため、この変更はrelease前にcommitしておく。

```sh
# 1. versionを上げてcommitする
$EDITOR Cargo.toml
git commit -am "Release 0.0.1"

# 2. 何も書き込まずに最後まで通して見る
scripts/release/release.sh --dry-run v0.0.1

# 3. tagを打ち、pushし、Releaseを作る
scripts/release/release.sh v0.0.1
```

tagは事前に用意しない。scriptがHEADへ打ってoriginへpushする。

optionはtagの前後どちらでも受ける。`release.sh v0.0.1 --dry-run`と書いてもdry runになる。
optionが黙って無視されて本番releaseが作られることはない。

## 実行順

書き込みは最後にまとめる。remoteから見える操作は、それ以外がすべて通ってから行う。

1. 検査 — clean tree、tagをHEADへ打てるか、ghの認証、同名Releaseの不在、`Cargo.toml`の
   versionとtagの一致、build結果へ影響するenv varの不在、host architecture、`rustc`のhost
2. build — `cargo build --release --locked`
3. 署名 — ad-hoc署名を付け、`codesign --verify`と`codesign -dv`で検証する
4. 検証 — `file`でarm64のMach-Oか、`sbxm --version`がrelease versionと一致するか
5. package — `dist/sbxm-aarch64-apple-darwin.tar.gz`を作り、直下が`sbxm`だけか確認する
6. 記録 — Git commit SHA、`rustc -vV`、`cargo -V`、`sw_vers`、`shasum -a 256`をrelease
   notesへ書く
7. publish — annotated tagをHEADへ打ち、originへpushし、`gh release create`する

tagを最後に打つのは、releaseを名付ける操作を、それを検証する工程の後ろへ置くためである。
先にtagを打つと、buildを一度も通していないcommitに対してtagがoriginへ出てしまう。

## 失敗したときに何が残るか

| 落ちた工程 | 残るもの |
| --- | --- |
| 検査・build・署名・package・記録 | tagはlocalにもoriginにも作られない |
| tagのpush | scriptが作ったlocal tagを消してから止まる |
| Releaseの作成 | tagはpush済みで残る。戻すためのcommandを表示して止まる |

Releaseの作成だけが落ちた場合、remoteのtagを黙って消しにはいかない。消すかどうかは
状況によるため、次の2つを表示して止まる。

```sh
git push origin :refs/tags/v0.0.1
git tag -d v0.0.1
```

同じtagでの再実行は通る。HEADを指すtagが既にあればそれを使い回すため、Releaseの作成だけが
落ちたときはそのまま実行し直せる。HEAD以外を指すtagがある場合は、localとoriginのどちらで
あっても、そのtagが指すcommitを示して拒否する。

## dry run

`--dry-run`は書き込む操作だけを行わない。tagを打たず、pushせず、Releaseも作らない。それ
以外はすべて本番と同じ手順で走る。使い捨てのrepositoryを用意する必要はなく、本物の
repositoryに対してそのまま実行できる。

- uploadするはずだったassetとrelease notesを`dist/`へ残すため、中身を確認できる
- 実行するはずだった`git tag`、`git push`、`gh release create`をそのまま表示する

publishの前提条件は、dry runでは即座に落とさず、警告として記録して最後にまとめて報告する。
tagがまだ無いtreeからでも最後まで通せなければ、予行の意味がないためである。

```
2 things would block the real release:
  - working tree is not clean; commit or stash before releasing
  - GitHub Release v0.0.1 already exists
the build itself succeeded; only these preconditions are unmet.
```

前提条件が1つでも欠けていればexit codeは非0になる。それぞれに解消するcommandを添える。
まとめて報告する対象は次の4つとする。

- working treeがcleanであること
- tagがHEAD以外を指していないこと (localとorigin)
- ghが認証済みであること
- 同名のGitHub Releaseが無いこと

生成物が正しいかどうかを決める検査は、dry runでも本番と同じく即座に落とす。`Cargo.toml`と
tagのversion一致、build-affecting env varの不在、host architecture、`rustc`のhost triple、
署名、arm64 Mach-O、`sbxm --version`、archiveの中身がこれにあたる。前提条件は「出してよいか」
を決め、これらは「出すものが正しいか」を決める。後者が崩れているtreeは、予行としても通さない。

## 生成物

`dist/`へ置く。repository直下へ置くと、次回実行のclean tree検査が前回の生成物を誤検知する。
`dist/`は`.gitignore`の対象とする。

- `dist/sbxm-aarch64-apple-darwin.tar.gz` — release asset。直下に`sbxm`だけを含む
- `dist/release-notes.md` — Releaseの本文。checksumとbuild provenanceを書く

archiveは`COPYFILE_DISABLE=1`のもとで作る。macOSがAppleDouble file (`._*`) やresource
forkをarchiveへ混ぜないようにする。

## 付けないoption

- `--prerelease` — 正式版だけを対象にする
- `--clobber` — 既存assetを誤って上書きしない

`gh release create`には`--verify-tag`を付ける。直前にpushしているため通るが、取り違えの
最後の歯止めとして残す。

## build結果へ影響するenv var

次のいずれかが設定されていれば止まる。再現性のないbinaryを出荷しないよう、releaseは常に
素のtoolchain設定で行う。

`RUSTFLAGS`、`CARGO_ENCODED_RUSTFLAGS`、`CARGO_BUILD_RUSTFLAGS`、`CARGO_BUILD_TARGET`、
`CARGO_TARGET_DIR`、`RUSTC`、`RUSTC_WRAPPER`、`RUSTC_BOOTSTRAP`、`CC`、`CFLAGS`、
`LDFLAGS`、`SDKROOT`、`MACOSX_DEPLOYMENT_TARGET`、`SOURCE_DATE_EPOCH`
