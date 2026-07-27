# 表示文字列のresource

利用者向け文字列はすべてこのdirectoryのFTL resourceが持つ。実装側に利用者向けの
literalを置かない。

規約はこのfileが1箇所で持ち、FTL resourceは内容だけを持つ。resourceへコメントと
見出しを書かない。規約をresourceへ書くと、言語の数だけ同じ規約を維持することになる。

## このdirectoryの構成

```text
locales/
├── README.md
├── en.ftl          # 正本locale
└── ja.ftl
```

file名は`<tag>.ftl`とする。`<tag>`は`--lang`とconfigの`language`が受け付ける値であり、
`src/i18n.rs`の`DEFINITIONS`が持つtagと一致させる。

## 言語を増やす手順

1. `locales/<tag>.ftl`を追加し、正本localeと同じID集合を同じplaceholderで訳す
2. `src/i18n.rs`の`Locale`へvariantを、`DEFINITIONS`へ行を足す

これ以外のfileは編集しない。実装、test、ほかのlocale resourceは、この2箇所からの
導出だけを見る。

## 正本locale

`en`を正本localeとする。正本localeは次を兼ねる。

- message IDとplaceholder集合の正本
- localeを決められない場合のfallback
- 翻訳しない値が書かれている言語

正本localeは`Locale::SOURCE`が指す。実装が特定の言語を名指しで分岐することはない。

## 規約

### 全localeが同じID集合を同じplaceholderで持つ

正本localeが定義するIDを過不足なく定義し、各messageが参照するplaceholder集合も
一致させる。`tests/ftl.rs`が検証する。

### message IDはkebab-case

### 翻訳しないもの

enum（状態値）、path、command名、option名、exit status、外部commandのstdoutと
stderr。これらは正本localeの語のまま出力するため、resourceへliteralとして現れない。

### `locale-name`

その言語の名称を「自称表記 / 正本localeの語」で書く。読めない書記系が選択肢に並んでも
識別できるようにする。両者が一致する場合は自称表記だけでよい。

```text
日本語 / Japanese
English
```

各resourceは自分の名称だけを持つ。ほかの言語の名称は持たない。

### 正本locale以外の診断label

「訳語 (正本localeの語)」の形式とする。利用者が正本localeの用語で検索できるように
するため、括弧内は訳さない。

```text
設定 (Config)
項目 (ITEM)
```

### 状態値の凡例

状態値を翻訳しないため、正本locale以外は`legend-*`で各値の説明を持つ。凡例は値を
繰り返すのではなく、値の意味を書く。

```text
legend-ready = 期待した状態で利用できる
```

正本localeの`legend-*`は出力されない。値そのものが正本localeの語であるため、
`Reporter::render_legend`が正本localeでは凡例を出さない。ID集合を一致させるために
定義だけは置く。

## message IDのグループと並び順

resourceは次の順で並べる。見出しを書かない代わりに、この順序を守る。

1. locale metadata（`locale-name`）
2. CLI全体
3. Subcommand
4. 引数と使い方のerror
5. 案件の識別子
6. Global設定
7. 永続化とlock
8. 案件metadata
9. 外部command
10. Docker Sandboxes互換性
11. host環境の診断
12. 対処方法
13. security
14. `init`
15. `status --global`
