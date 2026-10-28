# sbxm

開発環境と検証の手順は`docs/development.md`にある。変更を提案する前に`mise run check`を通す。

## 文章

Markdownの改行は段落の終わりにだけ置く。表示幅に合わせた折り返しは表示側が行うものであり、文字数を数えて文の途中で改行しない。英語も日本語も同じである。`mise run markdown`がこれを検査し、`mise run fmt`が段落の途中の改行を前の行へ結合する。

## script

repositoryに残す開発用のscriptは、Rustで`xtask` crateのtaskとして書き、`cargo xtask <task>`で呼ぶ。本番のcodeと同じlint、test、coverageの規律に従う。shell script、Python、TypeScriptなど、ほかの言語でscriptを新しく足さない。sbxmがSandboxやhostへ送って`sh`で実行するscriptは製品の一部であり、この規約の対象ではない。
