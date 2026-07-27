# sbxm MVP 方向性

## 1. この文書の役割

この文書は、`sbxm` MVPの目的、境界、共通の安全原則、公開CLI、全体状態モデルを定める。個々の処理手順、外部commandとの契約、再実行規則、test caseはPhase別仕様を正本とする。

- [Phase 1: 共通基盤、`init`、global `status`](specs/phase-1-foundation-init.md)
- [Phase 2: `add`と`sync-files`](specs/phase-2-add.md)
- [Phase 3: `open`、`stop`、`ls`、`status`](specs/phase-3-daily-operations.md)
- [Phase 4: `rebuild`、`destroy`とE2E検証](specs/phase-4-destroy-validation.md)

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

`sbxm`はDocker Sandboxesの便利なラッパー兼オーケストレーターであり、成果物の作成元を所有・追跡するsystemではない。metadata、Sandbox、workspace、image、Git repository、worktreeが誰によって作成されたかを利用可否の条件にしない。手作業または別toolで作成された状態も、`sbxm`のvalidation規則を満たす場合は同じ状態として受け入れる。

`status`は作成元ではなく現在の状態を診断する。mutation commandも`status`と同じvalidation規則に基づいて実行または拒否し、「sbxmが作った印」や作成履歴を追加の条件にしない。有効なmetadataを手作業で配置した場合、そのmetadataが示す対象を利用者がsbxmの管理対象として明示したものと扱う。

### 3.1 TTYと非TTY

- projectを対象とするcommandは、非TTYではproject引数の完全指定を必須とする
- 非TTYで対象を省略した場合は、config、metadata、filesystem、外部状態を読む前にexit code `1`で終了する
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

Docker Sandboxes CLIは0.37.0以上を要件とする。ただしEarly Accessで変更され得るため、「0.37.0以上なら無条件に動く」とは扱わない。各commandの実装時に、使用する外部command、structured output、代表的失敗を対象versionで確認し、parser fixtureとtestを同じ変更へ追加する。安全性に必要な出力を解釈できないversionではmutationを行わない。

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
sbxm init
sbxm --lang <ja|en> init --base-path <PATH> --git-user-name <NAME> --git-user-email <EMAIL>
sbxm [--lang <ja|en>] add <owner>/<repository> [--worktrees <N>] [--detach <BRANCH>]
sbxm [--lang <ja|en>] sync-files <owner>/<repository>
sbxm [--lang <ja|en>] rebuild <owner>/<repository>
sbxm [--lang <ja|en>] open [<owner>/<repository>]
sbxm [--lang <ja|en>] stop [<owner>/<repository>...]
sbxm [--lang <ja|en>] ls
sbxm [--lang <ja|en>] status --global
sbxm [--lang <ja|en>] status <owner>/<repository>
sbxm [--lang <ja|en>] destroy [-f|--force] [<owner>/<repository>]
```

公開commandは利用者が達成したい目的に基づいて命名し、下位toolのcommand構成をそのまま公開APIへ転写しない。内部工程や下位toolと同じ語彙になること自体は制約としない。

### 5.1 対象指定

- 引数あり: `<owner>/<repository>`を完全指定し、案件選択promptを出さない
- TTYで引数なし: metadataから候補を作り、必ずpromptを表示する
- 非TTYで引数なし: 対象を探索せずusage errorとする
- promptはstdinから読み、stderrへ表示する。両方がTTYでなければusage errorとする
- `open`と`destroy`は単一選択、`stop`は0件以上の複数選択とする
- `add`、`sync-files`、`rebuild`はproject引数の完全指定を必須とし、案件選択promptを出さない
- `status`は`--global`（短縮形`-g`）または`<owner>/<repository>`のどちらか一方を必須とし、案件選択promptを出さない
- `destroy --force`はTTYかどうかにかかわらずproject引数の完全指定を必須とする
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

同じcanonical project IDは常に同じ名前となり、異なるIDは通常hashで区別する。hash prefixの理論上の衝突も考慮し、導出した名前が別canonical IDを持つ既存metadataと一致する場合、または既存Sandboxの実状態が対象metadataから導出した期待状態と一致しない場合はname collision errorとし、mutationしない。

### 6.3 Host path

```text
<base-path>/<owner-lower>/<repository-lower>.project/
├── <repository-lower>/
└── .sbxm/
    ├── project.toml
    ├── project.lock
    ├── Dockerfile
    ├── exports/
    └── .cache/
        └── template-<dockerfile-sha256-first-12-hex>.tar
```

- `base_path`はabsolute、既存または作成可能、symlink解決後も利用者が指定したroot配下であること
- path構築には`PathBuf`を使う
- ownerとrepositoryのlowercase化により、case-insensitive filesystem上の重複を防ぐ
- metadata探索は`<base-path>/*/*.project/.sbxm/project.toml`だけを対象とし、symlinkを追跡しない

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

`<project-root>/.sbxm/project.toml`:

```toml
version = 1
owner = "example-org"
repository = "example-repo"
canonical_id = "example-org/example-repo"

[provisioning]
mode = "detached"
start_ref = "develop"
requested_worktrees = 3
dockerfile_sha256 = "<sha256>"

[[worktrees.managed]]
path = "example-repo.tree-0"
created_from = "refs/remotes/origin/develop"

# `rebuild`のSandbox切替中だけ存在する
[rebuild]
target_dockerfile_sha256 = "<sha256>"
previous_dockerfile_sha256 = "<sha256>"
```

- `provisioning`は進捗cacheではなく、利用者が要求した目標構成である
- `provisioning.dockerfile_sha256`は初回構築中の採用世代、構築完了後は現在のSandboxへ適用済みのDockerfile hashである
- `provisioning`は最初の外部mutation前にatomic writeする
- `worktrees.managed`はmanaged用pathの永続的な宣言であり、各worktree作成成功直後にatomic writeで追記する
- metadataが存在し、構築が未完了の案件へ同じ目標構成で`add`を再実行すると構築を継続する
- metadataが存在し、構築が完了した案件への`add`はno-op成功とする
- 再実行した`add`のoptionが保存済み目標構成と異なる場合は、mutationせず目標構成不一致とする
- `rebuild`で新世代成果物を検証しSandbox切替へ進む直前に、適用予定のDockerfile hashをdurableなrebuild intentとしてmetadataへatomic writeする。Sandbox再作成と検証の成功後に適用済みhashを更新し、intentを削除する
- rebuild intent中はtarget hashとprevious hashを世代判定の正本とする。previous世代のSandboxからは安全検査後の削除、target世代からは未完了工程、不在からは作成工程を継続し、どちらでもない世代は変更しない
- intent記録後にDockerfileが変わっても利用者の編集を上書きしない。固定済みtarget成果物が健全ならintent世代を完成させ、現在のDockerfileとの差分は次の`rebuild`対象として残す
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

| 現在状態 | `add` | `sync-files` | `rebuild` | `open` | `stop` | `destroy` |
|---|---|---|---|---|---|---|
| `unmanaged` | 新規登録して構築 | 対象未登録error | 対象未登録error | 対象未登録error | 対象未登録error | 対象未登録error |
| `registered` | 保存済み目標構成で構築を継続 | Sandbox未作成error | rebuild intentがあれば再構築を継続、なければ`add`を案内 | `add`を案内してerror | no-op成功 | 管理情報を破棄して`unmanaged` |
| `stopped` | 構築済みとしてno-op成功 | 起動せず拒否し`open`を案内 | 内部状態を観測できないため拒否 | 起動して接続 | no-op成功 | 通常modeは拒否、force modeは削除 |
| `running` | 構築済みとしてno-op成功 | 宣言fileを再配置 | 安全検査後に再構築 | そのまま接続 | 停止 | 通常modeはsession・保存状態検証後、force modeは検証なしで削除 |
| `inconsistent` | 診断付きerror | 診断付きerror | 診断付きerror | error | error | error |

`add`は新規登録と中断した初回構築の継続を担当する。`sync-files`は現在のglobal configにある`[[files]]`だけをrunning Sandboxへ再配置し、Git、Dockerfile、image、Template、worktreeを変更しない。`rebuild`はDockerfile変更を既存案件へ適用するため、安全検査後にSandboxを再作成する。`destroy`後はproject metadataを削除して`unmanaged`になるため、再構築には新しい目標構成を指定して`add`を実行する。

## 9. 表示言語と出力

- 組み込みlocaleは`en`と`ja`
- すべての利用者向け文字列をFTL resourceから生成する
- 英語FTLをmessage IDの正本とする
- 組み込みlocaleの欠落、placeholder不一致、format失敗はtest failure
- enum、path、command、exit status、外部stdout/stderrは翻訳しない
- 日本語の診断labelは`日本語 (English)`とする
- 日本語modeは人が読むための出力とし、正常出力の末尾には、実際に出現したenumだけの凡例を正常出力の一部としてstdoutへ出す
- scriptやpipeから機械的に利用する場合は`--lang en`を指定する。日本語modeのstdoutは機械可読な出力契約としない
- `--lang`、config、初回locale判定の優先順位はPhase 1仕様に従う
- stdoutは正常結果に使用する。英語modeでは機械的に利用可能なtableを提供し、日本語modeではtableに日本語のenum凡例を加える。stderrはprompt、warning、errorに使用する

## 10. Exit code

| Code | 意味 |
|---:|---|
| `0` | 成功、または仕様で成功と定めたno-op |
| `1` | 引数不正、通常キャンセル、前提不足、設定・状態不正、外部command失敗、安全上の拒否を含む通常error |
| `130` | Ctrl-CまたはEscによる対話キャンセル |

CLI parserを含む内部libraryの既定exit codeを公開契約へ透過しない。helpとversionは`0`、parse errorは`1`とする。外部commandのexit codeも直接透過せず、原値を診断へ含める。

失敗理由はexit codeで分類せず、翻訳しない安定した英語error ID、選択言語による説明、対象、観測値、対処方法、必要な場合はredact済みの外部stderrで示す。将来scriptが失敗理由による分岐を必要とする場合は、exit codeを増やさずstructured outputを検討する。

## 11. 実装順と品質gate

1. PR 1 / Phase 1で共通型、設定、metadata、i18n、command runner、互換性probe、`init`、`status --global`を実装する
2. PR 2 / Phase 2で`add`と`sync-files`を実装する
3. PR 3 / Phase 3で日常操作を実装する
4. PR 4 / Phase 4で共通データ保護検査、`rebuild`、`destroy`とE2Eを実装する

Phase 1〜4はそれぞれ独立した1 PRとし、合計4 PRで実装する。各PRは、そのPhaseのRust実装、fixture、自動test、文書更新を含み、単独でreview可能な状態にする。Rustの型、module境界、error設計、外部command abstraction、CLIの操作感をPhaseごとにreviewし、その結果によって後続Phaseの設計と実装を調整できるようにする。

後続Phaseの調査やlocal実装は、必要な共通interfaceが利用可能になり、関連する既存testが成功していれば並行して進めてよい。ただし後続PRは前Phase PRのreview結果を取り込み、前Phase PRより先にmergeしない。fixtureとparser testは、それを使用するcommandのPRへ同時に追加・更新する。

品質gateは次とする。

- 各変更は、変更対象の自動testと既存testを成功させてから次へ進む
- 各Phase PRは、そのPhaseの受入条件と自動testを満たしてからreview依頼する
- 外部commandを新たに使用する変更は、対象versionのfixture、正常系、代表的失敗、parse不能testを含める
- mutation commandは、対象を一意に特定できない場合と安全性を証明できない場合の拒否testを含める
- Phase境界を越えた調査やlocal実装を許可するが、失敗test、未確認fixture、未解決のsecurity条件をPR完成扱いにしない
- MVP完成には全Phaseの自動test、専用test repositoryでのE2E、SSH Agent・Docker socket非露出の実機確認を必須とする
- 実案件での利用は、対象操作に対応する自動testと実機E2Eが成功した後に行う

## 12. MVP利用後にreviewする論点

- 公開commandの語彙と粒度
- `ls`と`status`の責務分担
- 対話選択の操作速度
- 日本語labelとenum凡例の冗長さ
- `add`の中断理由と同じcommandによる再開導線
- `sync-files`の利用頻度と命名
- `rebuild`の所要時間と失敗後の再開導線
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
