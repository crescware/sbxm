# release

この文書は、`release.sh`が何を行い、どの順で行い、失敗したときに何を残すかを定める。

`release.sh`は、Apple Silicon macOS向けのbinaryをbuildし、tagを打ち、GitHub Releaseを
作成する。tagを打つところからRelease作成までを1つのcommandで行い、途中を手作業に委ねない。

## 前提

- macOS 14以降のApple Silicon機
- Xcode Command Line Tools — `codesign`と`file`を使う
- mise — toolchainは`mise install`で揃う。`mise run check`の実行にも使う
- gh CLI — 認証済みであること (`gh auth login`)

このscriptはmacOSでしか完走しない。arm64であること、`rustc`のhostが
`aarch64-apple-darwin`であることを検査し、満たさなければ止まる。

## 使い方

リリースするversionを`Cargo.toml`へ入れてcommitしておく。scriptは`Cargo.toml`の
`version`とtagのversionが一致することを要求する。working treeがcleanであることも要求する
ため、この変更はリリース前にcommitしておく。

```sh
# 1. versionを上げてcommitする
$EDITOR Cargo.toml
git commit -am "Release 0.0.1"

# 2. 何も書き込まずに最後まで通して見る
scripts/release/release.sh --dry-run --prerelease v0.0.1

# 3. tagを打ち、pushし、Releaseを作る
scripts/release/release.sh --prerelease v0.0.1
```

tagは事前に用意しない。scriptがHEADへ打ってoriginへpushする。

`--prerelease`か`--stable`は必ず渡す。versionからは推測しない。詳細は
[prereleaseかどうか](#prereleaseかどうか)に書く。

optionはtagの前後どちらでも受ける。`release.sh v0.0.1 --dry-run`と書いてもdry runになる。
optionが黙って無視されて本番のリリースが作られることはない。

## 実行順

書き込みは最後にまとめる。remoteから見える操作は、それ以外がすべて通ってから行う。

1. 検査 — clean tree、HEADが既定branchに入っているか、tagをHEADへ打てるか、ghの認証、
   GitHubへの到達、同名Releaseの不在、`Cargo.toml`のversionとtagの一致、build結果へ影響する
   env varの不在、host architecture、`rustc`のhost
2. `mise run check` — fmt、lint、macOS向けcompile、test、coverage
3. build — `cargo build --release --locked`
4. 署名 — ad-hoc署名を付け、`codesign --verify`と`codesign -dv`で検証する
5. 検証 — `file`でarm64のMach-Oか、`sbxm --version`がリリースするversionと一致するか
6. package — `dist/sbxm-aarch64-apple-darwin.tar.gz`を作り、直下が`sbxm`だけか確認する
7. 記録 — Git commit SHA、`rustc -vV`、`cargo -V`、`sw_vers`、`shasum -a 256`を
   リリースノートへ書く
8. publish — annotated tagをHEADへ打ち、originへpushし、`gh release create`する

安い検査を先に置き、時間のかかる`mise run check`をその後にする。tagの綴りを間違えただけの
実行を、testの完走まで待たせない。

tagを最後に打つのは、リリースを名付ける操作を、それを検証する工程の後ろへ置くためである。
先にtagを打つと、buildを一度も通していないcommitに対してtagがoriginへ出てしまう。

## リリースするtreeを自分で検査する

buildの前に`mise run check`を通す。通らなければそこで止まり、tagもbuildもRelease作成も
行わない。dry runでも同じく止まる。

検査を他所の結果に委ねない。どこかで検査が通ったかどうかはこのscriptから観測できず、通った
はずだという推測にしかならない。何をリリースしてよいかの判断を推測の上に置かない。

`mise run check`はmacOS向けにcompileできることも確かめる。Linuxで書いてLinuxで検査した
変更が、macOSでだけ通らないという状態を、releaseまで持ち越さないためである。

## リリースは既定branchから切る

HEADが既定branch (`main`) に入っていなければ拒否する。入っているかは、`origin`のHEADが指す
branchの先端をfetchし、そこからHEADへ辿れるかで判定する。

featureブランチのcommitへtagを打つと、そのbranchをsquash mergeまたはrebaseした後、tagは
どのbranchからも辿れないcommitを指したまま残る。tagが指す限りそのcommitは消えないため、
「このversionのsourceはどれか」がhistoryのどこからも答えられない状態になる。

merge前に試したい場合は`--dry-run`を使う。dry runはこれをblockerとして報告するが、build
から署名、packageまでは最後まで通すため、merge前でも成果物を確認できる。

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

- uploadするはずだったassetとリリースノートを`dist/`へ残すため、中身を確認できる
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
まとめて報告する対象は次の6つとする。

- working treeがcleanであること
- HEADが既定branchに入っていること
- tagがHEAD以外を指していないこと (localとorigin)
- originへ到達できること
- ghが認証済みであること、GitHubへ到達できること
- 同名のGitHub Releaseが無いこと

observeできないことを、無いことと同一視しない。次の2つは別の答えとして扱う。

- `git ls-remote`は、tagが無いときも、originへ届かないときも空を返す
- `gh release view`は、Releaseが無いときも、APIへ届かないときも失敗する

どちらも、届かないことを「無い」と読めば、networkが切れているだけの状態を「衝突は無い」と
答えてしまう。届くことを先に確かめ、確かめられなければその旨をblockerとする。

生成物が正しいかどうかを決める検査は、dry runでも本番と同じく即座に落とす。`Cargo.toml`と
tagのversion一致、build-affecting env varの不在、host architecture、`rustc`のhost triple、
署名、arm64 Mach-O、`sbxm --version`、archiveの中身がこれにあたる。前提条件は「出して
よいか」を決め、これらは「出すものが正しいか」を決める。後者が崩れているtreeは、予行と
しても通さない。

## 生成物

`dist/`へ置く。repository直下へ置くと、次回実行のclean tree検査が前回の生成物を誤検知する。
`dist/`は`.gitignore`の対象とする。

実行の冒頭で`dist/`を消す。どこで落ちても、`dist/`には今回の実行が作ったものしか無い状態を
保つ。途中で落ちた実行の後に前回の生成物が残っていると、それを今回のものと読んでしまう。

- `dist/sbxm-aarch64-apple-darwin.tar.gz` — リリース資産。直下に`sbxm`だけを含む
- `dist/release-notes.md` — Releaseの本文。checksumとbuild provenanceを書く

archiveは`COPYFILE_DISABLE=1`のもとで作る。macOSがAppleDouble file (`._*`) やresource
forkをarchiveへ混ぜないようにする。

## 署名をad-hocとする理由

現状はad-hoc署名 (`codesign --sign -`) とする。これは省略できる工程ではない。Apple Silicon
はsignatureを持たないbinaryを実行しないため、配布するbinaryには最低限どれかの署名が要る。

ad-hoc署名は「誰が署名したか」を持たない。そのためGatekeeperの「開発元を確認できません」
は消えない。これを消すにはApple Developer Program (年間$99 USD) への加入が要り、加入して
初めて次の2つが手に入る。片方だけでは足りない。

- Developer ID Application証明書 — 氏名とTeam IDが入った、身元付きの署名証明書
- Notarizationサービス — 署名済みbinaryをAppleへ送り、malware検査済みのticketを受け取る

無料のApple IDで取れるのは手元の実機で動かす開発用証明書であり、App Store外への配布には
使えない。

### それでもad-hocで足りると判断した根拠

Gatekeeperの警告が出るのは、fileに`com.apple.quarantine`が付いている場合だけとする。この
属性を付けるのはbrowser、mail、AirDropであり、`curl`、`wget`、Homebrewは付けない。

| 入手方法 | 警告 |
| --- | --- |
| `brew install` | 出ない |
| Releasesページからbrowserで直接download | 出る |

配布はHomebrew tapを想定している。その経路の利用者はGatekeeperの警告に触れないため、
Developer IDへ移っても利用者の体験は変わらない。年$99と実装の手間に対して得るものが無い。

### 見直す条件

時期ではなく、Releasesページからbinaryを直接downloadする利用者が主になったときとする。
その層にはHomebrewの経路が効かないため、そこで初めて費用に見合う。

移行は後からで構わない。過去のReleaseへ手を入れる必要はなく、ad-hoc署名のversionを使って
いる利用者が公証済みのversionへ上がるときも、特別な操作は要らない。年会費であるため、必要
になってから加入した方が総額も少ない。

### 移行するときに変わるもの

```sh
# 署名 — 身元付きにする。hardened runtimeとtimestampは公証の必須条件
codesign --force --sign "Developer ID Application: ... (TEAMID)" \
  --options runtime --timestamp "$bin_path"

# 公証 — Appleへ送って結果を待つ
xcrun notarytool submit "$archive" --apple-id ... --team-id ... --password ... --wait
```

このとき、配布形式そのものを見直す必要が出る。`xcrun stapler`はticketを`.app`、`.dmg`、
`.pkg`にしか貼れず、tar.gzの中の裸の実行fileには貼れない。貼れない場合、Gatekeeperは初回
実行時にAppleへonlineで問い合わせる。offlineでも通るようにするなら`.pkg`か`.dmg`へ変える。

## 誰がリリースできるか

このscriptは権限を持たない。実行者の資格情報で`git`と`gh`を呼ぶだけとする。

tagのpushと`gh release create`は、どちらもrepositoryへのwrite権限を要する。権限を持たない
者がこのscriptを実行しても、その2箇所で止まる。誰がリリースできるかはGitHub側の設定が
決めるのであり、このscriptが決めるのではない。

repositoryがpublicであるため、fork先に対してこのscriptを実行することは誰にでもできる。その
場合の成果物はfork先のReleaseとして出るのであり、上流には影響しない。

write権限を持つ者を増やすときは、その全員がtagを打ってReleaseを作れるようになる。tagの作成
者を絞るなら、collaboratorを追加する前にtag protectionのrulesetを入れる。

## prereleaseかどうか

`--prerelease`か`--stable`のどちらかを必ず渡す。省略すると、何も書かずに拒否する。

```sh
scripts/release/release.sh --prerelease v0.0.1
scripts/release/release.sh --stable v1.0.0
```

versionからは推測しない。`0.0.1`という並びは未完成を示唆するが、示唆は宣言ではない。version
はこの判断の正本ではなく、たまたま似た形をした別の値である。

推測を既定にすると、間違えたときに黙って通る。リリースが完成品かどうかは、GitHubが
「Latest」として指すかどうかを決め、`/releases/latest`が答えるかどうかを決める。取り違えた
まま公開すると、未完成のものが最新の正式版として案内される。この判断は、それを知っている
者が明示的に述べるものとする。

両方を同時に渡すと落ちる。どちらか一方に解釈せず、矛盾として扱う。

どちらを選んだかは実行時に表示する。dry runでも表示し、`gh release create`へ`--prerelease`
が付くかどうかをそのまま確認できる。

```
==> publishing as a prerelease (--prerelease)
```

## 付けないoption

- `--clobber` — 既存assetを誤って上書きしない

`gh release create`には`--verify-tag`を付ける。直前にpushしているため通るが、取り違えの
最後の歯止めとして残す。

## build結果へ影響するenv var

次のいずれかが設定されていれば止まる。再現性のないbinaryを出荷しないよう、リリースは常に
素のtoolchain設定で行う。

`RUSTFLAGS`、`CARGO_ENCODED_RUSTFLAGS`、`CARGO_BUILD_RUSTFLAGS`、`CARGO_BUILD_TARGET`、
`CARGO_TARGET_DIR`、`RUSTC`、`RUSTC_WRAPPER`、`RUSTC_BOOTSTRAP`、`CC`、`CFLAGS`、
`LDFLAGS`、`SDKROOT`、`MACOSX_DEPLOYMENT_TARGET`、`SOURCE_DATE_EPOCH`
