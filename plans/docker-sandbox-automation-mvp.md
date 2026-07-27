# sbxm MVP 方向性

## 1. この文書の役割

この文書は、`sbxm` MVPの目的、境界、共通の安全原則、公開CLI、全体状態モデルを定める。個々の処理手順、外部commandとの契約、再実行規則、test caseはPhase別仕様を正本とする。

- [Phase 1: 共通基盤、`init`、global `status`](specs/phase-1-foundation-init.md)
- [Phase 2: `add`](specs/phase-2-add.md)
- [Phase 3: `open`、`stop`、`ls`、`status`](specs/phase-3-daily-operations.md)
- [Phase 4: `rm`とE2E検証](specs/phase-4-remove-validation.md)

本文とPhase別仕様が矛盾する場合は、本文の安全原則を優先し、実装前に文書を修正する。実装で矛盾を吸収しない。

## 2. 目的

`sbxm`は、Codex・Claude Code向けDocker Sandboxの案件別セットアップと日常操作を自動化するRust製CLIである。

初版では汎用的なSandbox管理基盤を目指さず、既存運用を安全かつ再現可能に実行する。Docker imageのbuild、Templateのload、Sandbox内Git設定などは内部工程とし、公開CLIは利用者が達成したい目的に対応させる。

実際の案件でMVPを一巡した後に、公開command、設定項目、対応環境を拡張する。

## 3. 設計原則

- 初心者には、選択言語による状態説明、危険性、具体的な対処方法を提供する
- script向けには、安定した英語enum、exit code、引数指定時の非対話動作を提供する
- 対象を省略した人間には、既定選択のない安全な対話選択を提供する
- current directoryから操作対象を推測しない
- 外部commandのstderrは原文を保持し、その前に選択言語による説明を表示する
- 既存ファイル、Sandbox、repositoryを暗黙に削除または上書きしない
- 外部状態を観測できない場合に推測した状態を返さない
- 同じrepository内のagentとworktreeは共同作業単位であり、security境界ではない
- 異なるrepositoryは別Sandboxへ隔離する
- managed worktreeと、Agentなどが作るunmanaged worktreeを区別する
- SandboxへホストのSSH Agent、SSH秘密鍵、Docker socketを渡さない
- secret値をargument、log、設定ファイルへ保存しない

### 3.1 TTYと非TTY

- projectを対象とするcommandは、非TTYではproject引数の完全指定を必須とする
- 非TTYで対象を省略した場合は、config、metadata、filesystem、外部状態を読む前にexit code `2`で終了する
- 非TTYではcurrent directory、候補数、過去の選択から対象を推測または自動選択しない
- TTYでは、仕様で許可されたcommandに限り対象省略時の選択promptを表示する
- 引数が完全指定された非TTY操作は対話確認を要求せず、指定対象だけを処理する
- `ls`、`status --global`などprojectを対象としないcommandにはproject引数を要求しない
- `init`の非TTY規則はPhase 1仕様に従う

## 4. MVPの範囲

### 4.1 対象

- macOS Sonoma 14以降
- Apple silicon Mac
- GitHub repository
- Docker Desktop
- Docker Sandboxes CLI
- GitHub CLI
- Remote SSH対応editor
- 日本語と英語
- 1 GitHub repositoryにつき1 project directory、1 Sandbox
- 1 Sandbox内で1 bare Git repositoryと複数worktreeを共有

Docker Sandboxes CLIは0.37.0以上を要件とする。ただしEarly Accessで変更され得るため、「0.37.0以上なら無条件に動く」とは扱わない。Phase 1で互換性probeを実装し、Phase 2着手前に、検証済みexact version、使用command、JSON fixtureをrepositoryへ固定する。未検証versionではmutationを行わない。

### 4.2 対象外

- Linux、Windows、Intel Mac
- GitLabなどGitHub以外のhosting
- 同一repositoryを複数Sandboxへ分離するinstance機能
- host側project全体の自動削除
- Dockerfileの自動再build
- worktree追加・削除専用command
- secret値の入力代行または保存
- Codex・Claude Codeの対話login自動化
- repository由来の`mise trust`、`mise install`の自動実行
- CPU、memory設定
- port、exportの独自wrapper

## 5. 公開CLI

```text
sbxm [--lang <ja|en>] init
sbxm [--lang <ja|en>] add <owner>/<repository> [--worktrees <N>] [--detach <BRANCH>]
sbxm [--lang <ja|en>] open [<owner>/<repository>]
sbxm [--lang <ja|en>] stop [<owner>/<repository>...]
sbxm [--lang <ja|en>] ls
sbxm [--lang <ja|en>] status --global
sbxm [--lang <ja|en>] status <owner>/<repository>
sbxm [--lang <ja|en>] rm [-f|--force] [<owner>/<repository>]
```

`create`、`setup`、`start`、`shell`、`destroy`など、内部工程や下位toolの語彙は公開commandにしない。

### 5.1 対象指定

- 引数あり: `<owner>/<repository>`を完全指定し、案件選択promptを出さない
- TTYで引数なし: metadataから候補を作り、必ずpromptを表示する
- 非TTYで引数なし: 対象を探索せずusage errorとする
- promptはstdinから読み、stderrへ表示する。両方がTTYでなければusage errorとする
- `open`と`rm`は単一選択、`stop`は0件以上の複数選択とする
- `status`は`--global`（短縮形`-g`）または`<owner>/<repository>`のどちらか一方を必須とし、案件選択promptを出さない
- `rm --force`はTTYかどうかにかかわらずproject引数の完全指定を必須とする
- promptに既定選択を設けない
- EscまたはCtrl-Cは何も変更せず、exit code `130`で終了する
- `stop`で0件を確定した場合だけ、何も変更せずexit code `0`とする

## 6. 識別子とpath

### 6.1 Project ID

入力は`<owner>/<repository>`の1個のslashを持つ文字列とする。

- owner: `[A-Za-z0-9](?:[A-Za-z0-9-]{0,37}[A-Za-z0-9])?`
- repository: `[A-Za-z0-9._-]{1,100}`
- `.`、`..`はrepository名として拒否する
- 比較用canonical project IDはASCII lowercaseの`owner/repository`とする
- 表示にはmetadataへ保存したGitHub上のowner、repository表記を使用する
- canonical project IDが一致するmetadataを複数検出した場合はconflict errorとし、mutationしない

### 6.2 Sandbox名

Sandbox名はcanonical project IDから決定的に導出する。

1. canonical project IDの`/`を`-`へ変え、`[a-z0-9-]`以外を`-`へ置換する
2. 連続する`-`を1個へ畳み、前後の`-`を除く
3. UTF-8のcanonical project IDに対するSHA-256先頭12桁のlowercase hexを求める
4. `sbxm-<slug>-<hash>`が63 byte以内になるようslugの末尾を切る

同じcanonical project IDは常に同じ名前となり、異なるIDは通常hashで区別する。hash prefixの理論上の衝突も考慮し、導出した名前が別canonical IDを持つ既存metadataと一致する場合、または帰属を証明できない既存Sandboxと一致する場合はname collision errorとし、mutationしない。

### 6.3 Host path

```text
<base-path>/<owner-lower>/<repository-lower>.project/
├── <repository-lower>/
└── .sbx/
    ├── sbxm.toml
    ├── Dockerfile
    ├── exports/
    └── .cache/
        └── template.tar
```

- `base_path`はabsolute、既存または作成可能、symlink解決後も利用者が指定したroot配下であること
- path構築には`PathBuf`を使う
- ownerとrepositoryのlowercase化により、case-insensitive filesystem上の重複を防ぐ
- metadata探索は`<base-path>/*/*.project/.sbx/sbxm.toml`だけを対象とし、symlinkを追跡しない

### 6.4 Sandbox内path

```text
/home/agent/work/<repository-lower>/
├── .git/
├── <repository-lower>.tree-0/
└── <repository-lower>.tree-1/
```

`.git`はbare repositoryであり、親directory自体はworktreeではない。

## 7. 設定とmetadata

### 7.1 Global config

`~/.sbxm/config.toml`:

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

- `~/.sbxm`は`0700`
- configは`0600`
- token、secret、runtime状態を保存しない
- 未知のtop-level keyはversion 1ではwarning、未知の必須構造や未知versionはerror
- `files`はhost上の通常fileをSandbox内の`agent` homeからの相対pathへ配置する宣言
- `source`はabsolute path、`destination`はabsolute pathと`..`を含まないrelative pathとする
- credential、token、秘密鍵の転送には`files`を使わず、Docker Sandboxesのsecret機能を使用する

### 7.2 Project metadata

`<project-root>/.sbx/sbxm.toml`:

```toml
version = 1
owner = "example-org"
repository = "example-repo"
canonical_id = "example-org/example-repo"

[provisioning]
mode = "detached"
start_ref = "develop"
requested_worktrees = 3

[[worktrees.managed]]
path = "example-repo.tree-0"
created_from = "refs/remotes/origin/develop"
```

- `provisioning`は進捗cacheではなく、利用者が要求した目標構成である
- `provisioning`は最初の外部mutation前にatomic writeする
- 再実行時の引数が保存済み構成と異なる場合はusage conflictとしてmutation前に拒否する
- `worktrees.managed`はmanaged用pathの永続的な宣言であり、各worktree作成成功直後にatomic writeで追記する
- `rm`後も宣言を保持し、再構築時には実体がまだ存在しない目標pathとして扱う
- runtime state、HEAD、dirty状態は保存せずGitと`sbx`から取得する

## 8. 状態モデル

### 8.1 管理状態

- `unmanaged`: 有効なproject metadataがない
- `registered`: metadataがあり、Sandboxが存在しない
- `running`: metadataと対応する起動中Sandboxがある
- `stopped`: metadataと対応する停止中Sandboxがある
- `inconsistent`: metadata、成果物、Sandboxの対応に矛盾がある

`inconsistent`では、読み取り専用診断以外のmutationを禁止する。

### 8.2 Command別状態遷移

| 現在状態 | `add` | `open` | `stop` | `rm` |
|---|---|---|---|---|
| `unmanaged` | 新規登録して構築 | 対象未登録error | 対象未登録error | 対象未登録error |
| `registered` | 保存済み目標構成で構築・再構築 | `add`を案内してerror | no-op成功 | no-op成功 |
| `stopped` | 成果物を検証してno-op成功 | 起動して接続 | no-op成功 | clean検証後に削除 |
| `running` | 成果物を検証してno-op成功 | そのまま接続 | 停止 | session停止・clean検証後に削除 |
| `inconsistent` | 診断付きerror | error | error | error |

`rm`後は`registered`になる。再構築は保存済みの目標構成を使って`add`を実行する。

## 9. 表示言語と出力

- 組み込みlocaleは`en`と`ja`
- すべての利用者向け文字列をFTL resourceから生成する
- 英語FTLをmessage IDの正本とする
- 組み込みlocaleの欠落、placeholder不一致、format失敗はtest failure
- enum、path、command、exit status、外部stdout/stderrは翻訳しない
- 日本語の診断labelは`日本語 (English)`とする
- 日本語出力の末尾には、実際に出現したenumだけの凡例を出す
- `--lang`、config、初回locale判定の優先順位はPhase 1仕様に従う
- stdoutは正常結果と機械的に利用可能なtable、stderrはprompt、warning、errorに使用する

## 10. Exit code

| Code | 意味 |
|---:|---|
| `0` | 成功、または仕様で成功と定めたno-op |
| `2` | CLI usage、入力値、保存済み目標構成との不一致 |
| `3` | 前提command、version、host環境の非対応 |
| `4` | configまたはmetadata不正、成果物の不整合 |
| `5` | 外部command失敗、外部状態を観測不能 |
| `6` | security条件を証明できない、または破壊操作を安全に実行不能 |
| `10` | 利用者が通常の確認でキャンセル |
| `130` | Ctrl-CまたはEscによる対話キャンセル |

外部commandのexit codeは`sbxm`のexit codeへ直接透過しない。原値は診断へ含める。

## 11. 実装順とreview gate

1. Phase 1で共通型、設定、metadata、i18n、command runner、互換性probe、`init`、`status --global`を実装する
2. Phase 1のDocker Sandboxes互換性fixtureとdaemon安全性probeをreviewし、Phase 2着手を承認する
3. Phase 2で`add`を実装する
4. Phase 3で日常操作を実装する
5. Phase 4で破棄操作とE2Eを実装する

各Phaseは、仕様内の自動testと受入条件を満たし、前Phaseのschemaと外部command fixtureが固定されるまで開始しない。

## 12. MVP利用後にreviewする論点

- 公開commandの語彙と粒度
- `ls`と`status`の責務分担
- 対話選択の操作速度
- 日本語labelとenum凡例の冗長さ
- `add`の中断理由と再開導線
- `open`後のworktree移動支援
- worktree追加・削除command
- repository単位の共有境界
- Dockerfile再build
- 案件別Git identity
- host側project全体の削除

このreviewまでは、公開command、設定項目、対応環境を増やさない。

## 13. 参照資料

- [Docker Sandboxes](https://docs.docker.com/ai/sandboxes/)
- [Docker Sandboxes architecture](https://docs.docker.com/ai/sandboxes/architecture/)
- [Docker Sandboxes credentials](https://docs.docker.com/ai/sandboxes/security/credentials/)
- [Docker Sandboxes troubleshooting](https://docs.docker.com/ai/sandboxes/troubleshooting/)
- [Docker Sandboxes release notes](https://docs.docker.com/ai/sandboxes/release-notes/)
- [Docker Sandboxes templates](https://docs.docker.com/ai/sandboxes/customize/templates/)

外部CLIの仕様は変更され得るため、参照資料の現在内容よりPhase 1でcommitするexact-version fixtureを実装上の契約とする。
