# sbxm

案件ごとのDocker Sandboxを構築、接続、診断、破棄するRust製CLI。

`sbxm`はDocker Sandboxesのラッパー兼オーケストレーターであり、成果物の作成元を所有・追跡する
systemではない。metadata、Sandbox、workspace、image、Git repository、worktreeが誰によって
作成されたかを利用可否の条件にしない。手作業または別toolで作成された状態も、validation規則を
満たす場合は同じ状態として受け入れる。

- 方向性: [plans/docker-sandbox-automation-mvp.md](../plans/docker-sandbox-automation-mvp.md)
- Phase別仕様: [plans/specs/](../plans/specs/)

## 実装状況

MVPは4 Phaseに分けて実装する。現在はPhase 1まで。

| Phase | 範囲 | 状態 |
|---|---|---|
| 1 | 共通基盤、`init`、`status --global` | 実装済み |
| 2 | `add`、`sync-files` | 未着手 |
| 3 | `open`、`stop`、`ls`、`status <project>` | 未着手 |
| 4 | `rebuild`、`destroy`、E2E検証 | 未着手 |

9 commandすべてがparserへ登録済みであり、helpとcommand固有の引数validationは全commandで
動作する。未実装のcommandは、引数validationを通過したあとに`not-implemented`で終了する。

## 対象環境

- macOS Sonoma 14以降、Apple silicon Mac
- Docker Desktop、Docker Sandboxes CLI 0.37.0以上
- GitHub repository、GitHub CLI
- Remote SSH対応editor

Docker Sandboxes CLIはEarly Accessであり、「0.37.0以上なら無条件に動く」とは扱わない。
使用する外部commandとstructured outputは、対象versionで採取したfixtureを契約とする。

## 使い方

```sh
sbxm [--lang <ja|en>] init
sbxm [--lang <ja|en>] init --base-path <PATH> --git-user-name <NAME> --git-user-email <EMAIL>
sbxm [--lang <ja|en>] status --global
```

`--lang`はsubcommandの前後どちらでも指定できる。scriptやpipeから機械的に利用する場合は
`--lang en`を指定する。日本語modeのstdoutは機械可読な出力契約としない。

exit codeは3つだけを使う。

| Code | 意味 |
|---:|---|
| `0` | 成功、または仕様で成功と定めたno-op |
| `1` | 引数不正、通常キャンセル、前提不足、設定・状態不正、外部command失敗、安全上の拒否 |
| `130` | Ctrl-CまたはEscによる対話キャンセル |

失敗理由はexit codeで分類せず、翻訳しない安定した英語error IDと、選択言語による説明で示す。

## Build と test

```sh
cargo build
cargo test
```

CLIの公開契約（command名、option名、value name、arity、並び順）は`tests/snapshots/cli-surface.txt`
に記録する。翻訳文を含まないため、言語を増やしても変わらない。契約を意図的に変更した場合だけ、
差分を確認してから更新する。

```sh
SBXM_UPDATE_SNAPSHOTS=1 cargo test --bin sbxm
```

## 表示言語

利用者向け文字列はすべて[locales/](../locales/)のFTL resourceが持つ。言語を増やすときに触るのは、
resource 1枚と`src/i18n.rs`のlocale定義表1行だけとする。規約は
[locales/README.md](../locales/README.md)が持つ。

## Docker Sandboxes CLI fixtureは未採取

`compatibility.toml`の`validated_cli_versions`は空である。この状態のsbxmは、検出した
Docker Sandboxes CLIのversionを解釈できないものとして扱い、`sbx`の出力に依存する検査を
`sbx-fixtures-not-collected`で停止する。`sbxm status --global`のDocker Sandboxes、Login、
Network policy、Remote SSH、Daemon、Session inspectionは、fixtureを採取するまで`error`と
なる。

これは未実装ではなく、「外部状態を観測できない場合に推測した状態を返さない」という原則を、
fixture未採取の状態へそのまま適用した結果である。採取手順は
[tests/fixtures/sbx/README.md](../tests/fixtures/sbx/README.md)を参照する。

採取には対象Mac（macOS 14以降のApple silicon、Docker Desktop、Docker Sandboxes CLI）が
必要であり、Phase 1のPRを作成した環境では満たせなかった。

## 設定

`~/.sbxm/config.toml`（`0600`、`~/.sbxm`は`0700`）。

```toml
version = 1
language = "ja"
base_path = "/Users/example/Projects"

[git]
user_name = "Example User"
user_email = "user@example.com"

[[files]]
source = "/Users/example/.config/example/config.toml"
destination = ".config/example/config.toml"
```

- token、secret、runtime状態を保存しない
- `files`はhost上の通常fileをSandbox内の`agent` homeからの相対pathへ配置する宣言であり、
  credential、token、秘密鍵には使用しない。それらにはDocker Sandboxesのsecret機能を使う
- 不正なconfigをsbxmが自動修復・上書きすることはない。直接編集して次回実行で再検証する
