# Phase 1 実装仕様: 共通基盤と`init`

## 1. 目的と完了境界

Phase 1は、後続Phaseが判断を追加せず利用できる共通型、永続化、外部command実行、翻訳、対象解決、Docker Sandboxes互換性probeを実装し、`sbxm init`を完成させる。

Phase 1完了時点ではprojectやSandboxを作成しない。Phase 2の外部mutationは、本文書の互換性gateが承認されるまで開始しない。

## 2. 成果物

```text
Cargo.toml
src/
├── main.rs
├── cli.rs
├── config.rs
├── error.rs
├── i18n.rs
├── project.rs
├── paths.rs
├── command.rs
├── compatibility.rs
└── workflow/
    ├── mod.rs
    └── init.rs
locales/
├── en.ftl
└── ja.ftl
tests/
└── fixtures/
    └── sbx/<validated-version>/
```

主なdependencyは`clap`、`serde`、`toml`、`thiserror`、`dirs`、`fluent-bundle`、`unic-langid`、`dialoguer`、`sha2`、`serde_json`とする。dependency versionは実装PRでlock fileとともに固定する。

## 3. 共通domain型

stringを直接workflowへ渡さず、validation済みの型を使う。

```text
ProjectId {
  owner_display,
  repository_display,
  canonical_id
}

SandboxName(String)
AbsoluteBasePath(PathBuf)
Locale { En, Ja }
ManagedWorktreePath(PathBuf)
```

`ProjectId::parse`、Sandbox名導出、host path導出は方向性文書の規則を唯一の実装とする。

## 4. CLI parse

Phase 1で7 commandと全optionをparserへ登録する。未実装commandはparse後にlocalizedな`not implemented in this build`を返してexit code `3`とする。これによりhelpとusageの翻訳・snapshotをPhase 1で固定する。

validation順:

1. syntaxとoption関係
2. `--lang`
3. command固有の引数
4. config load
5. project解決
6. 外部command
7. mutation

`add --worktrees 0`、`add --worktrees >= 2`かつ`--detach`なしはconfigやfilesystemを読む前にexit code `2`とする。

## 5. Locale決定

優先順位:

1. 有効な`--lang ja|en`
2. 有効なglobal configの`language`
3. `init`実行時だけmacOS優先言語
4. shell locale
5. `en`

通常commandでconfigが存在しない場合は、`sbxm init`を案内してexit code `4`とする。error表示は3〜5で選んだbootstrap localeを使う。

macOS優先言語は`defaults read -g AppleLanguages`の出力をparseする。先頭が`ja`または`ja-*`なら、TTY上でJapanese / Englishを選択させる。その他はpromptなしで`en`とする。command失敗またはparse失敗時だけ`LC_ALL`、`LC_MESSAGES`、`LANG`の順にfallbackする。

`init`が非TTYで、言語、base path、Git identityのいずれかに対話入力が必要なら、何も作成せずexit code `2`とする。MVPには非対話`init` optionを設けない。

## 6. FTL契約

- message IDは意味と用途を表すkebab-case
- 英語と日本語のID集合およびplaceholder集合を完全一致させる
- help、usage、prompt、正常出力、warning、errorをFTLから生成する
- format失敗は対象message IDとlocaleを示してexit code `4`
- 外部stderrをFTL placeholderへ埋め込まず、localized説明とは別blockで出す
- security messageは`title`、`description`、`remediation`の3 IDを必須とする

testではFTL parse、ID一致、placeholder一致、全command help snapshot、代表的なerror snapshotを検証する。

## 7. Atomic file write

configとmetadataは次の手順で書く。

1. 同一directoryに`create_new`で一時fileを作る
2. 必要permissionを設定する
3. 全内容を書いて`sync_all`する
4. 既存targetがないことを再確認する
5. renameする
6. 親directoryを`sync_all`する

更新時は既存fileのpermissionとidentityを検証し、同一directoryの一時fileからatomic renameする。symlinkは拒否する。秘密情報を一時fileへ書かない。

processが中断した一時fileは次回起動時に自動削除せず、pathと安全な削除方法を表示してexit code `4`とする。

## 8. Config loadとvalidation

### 8.1 不在

- `init`: 新規作成へ進む
- その他: `sbxm init`を案内してexit code `4`

### 8.2 有効

`init`は保存済み値を再入力させない。host prerequisiteとSSH setupだけ再検査し、configを変更せず成功する。

### 8.3 無効

構文不正、未知version、必須値欠落、permission過剰、symlink、relative base pathはpathと原因を示してexit code `4`。`init`も自動修復・上書きしない。

`base_path`はstandardizeしたabsolute pathとして保存する。存在しなければ`init`が確認後に作成する。既存ならdirectoryであり、利用者がwrite可能であることを確認する。

## 9. Project metadata探索

- `base_path`直下のowner directoryと、その直下の`*.project/.sbx/sbxm.toml`だけを読む
- directory entryとmetadata fileのsymlinkは追跡しない
- すべてのmetadataをparseしてから結果を返す
- canonical ID重複、導出path不一致、Sandbox名衝突は一覧化してexit code `4`
- 1件の破損を無視して部分的な案件一覧を返さない
- 並び順はcanonical IDのbyte昇順

## 10. 外部command runner

runner input:

```text
program
args[]
cwd
environment policy
stdin policy
stdout policy
stderr policy
timeout class
```

規則:

- shellを介さない
- defaultで現在processのenvironmentを継承する
- security-sensitiveな`sbx`起動では`SSH_AUTH_SOCK`を必ず除外する
- secret値をargumentやdebug表示へ渡さない
- stdoutとstderrを別々にbyte列として保持し、lossy変換した場合はその事実を診断する
- timeout時はchildを終了し、command名とtimeoutを表示してexit code `5`
- testではfake executableをPATH先頭へ置き、program、args、cwd、environment、streamを記録する

timeout既定値:

| Class | Timeout |
|---|---:|
| probe | 10秒 |
| local filesystem/Git | 60秒 |
| image build/save | 30分 |
| Sandbox create/start/stop/rm | 10分 |
| interactive | timeoutなし |

## 11. Docker Sandboxes互換性gate

Docker Sandboxes CLIはEarly Accessである。Phase 1実装PRは、対象Macで次を採取しfixtureとしてcommitする。

- `sbx version`または同等command
- `sbx --help`
- 使用する各subcommandの`--help`
- `sbx ls --json`の0件、running、stopped fixture
- `sbx inspect`のfixture
- `sbx daemon status`のrunning、stopped fixture
- secret存在確認に使うread-only出力
- create、exec、stop、rm、Template操作の正常・代表的失敗exit status

互換性manifest:

```toml
schema_version = 1
validated_cli_versions = ["<exact-version>"]
ls_json_fixture_version = 1
```

runtimeではexact versionを検出する。

- 0.37.0未満: exit code `3`
- fixtureと一致するversion: 続行
- patch versionだけ異なる: read-only commandはwarning付きで許可、mutationはexit code `3`
- minor/majorまたはparse不能: exit code `3`

新version対応はfixture、parser test、manifestを更新するPRで行う。

## 12. Daemon安全性probe

Phase 2開始前に、次を実機で証明して結果を`tests/fixtures/sbx/<version>/daemon-security.md`へ記録する。

1. `SSH_AUTH_SOCK`ありで起動したdaemonがSandboxへagentを転送すること
2. `SSH_AUTH_SOCK`をunsetして`sbx daemon start --detach`したdaemonでは転送されないこと
3. `sbx daemon status`またはOS process情報から、現在のdaemon instanceを一意に識別できること
4. Docker Desktop再起動、Mac再起動、`sbx daemon stop/start`でinstance識別子が変わること
5. markerとinstance識別子を比較できること

一意なinstance識別子を取得できない場合、runtime marker方式は採用しない。その場合のMVP仕様は、mutationを伴う`add`と最初の`open`の前にdaemonを停止し、`SSH_AUTH_SOCK`をunsetして起動し直すこととする。別Sandboxのactive sessionがある場合は自動停止せずexit code `6`とし、利用者へ停止方法を案内する。

markerを採用できる場合:

- 保存先は`~/.sbxm/runtime/daemon.toml`
- directory `0700`、file `0600`
- `sbx`のexact version、daemon instance ID、起動時刻を保存
- file lock `~/.sbxm/runtime/daemon.lock`をdaemon操作全体でexclusive取得
- marker不在、不一致、parse失敗は安全と見なさない
- markerだけを根拠にせず、毎回現在instance IDと一致させる

## 13. `sbxm init`

### 13.1 事前状態

configがない場合だけ新規作成する。既存の有効configは再利用し、無効configは停止する。

### 13.2 処理順

1. bootstrap localeを決定する
2. configの有無と妥当性を確認する
3. `sw_vers`と`uname -m`でmacOS 14以上、arm64を確認する
4. `brew`、`docker`、`gh`、`sbx`の存在を確認する
5. Docker Client/Server疎通を確認する
6. Docker Sandboxes exact versionとcompatibility manifestを照合する
7. loginが必要なら`sbx login`をTTY接続で起動する
8. network policy状態をread-onlyで表示する。自動変更しない
9. Remote SSH機能の対応状況をfixtureに基づき確認し、必要な公式setup commandを実行する
10. configがなければlanguage、base path、Git name、Git emailを取得・検証する
11. configをatomic writeする
12. prerequisite結果を表示する

Homebrew installやnetwork policy変更は自動実行しない。正確な公式commandを表示してexit code `3`とし、再実行を案内する。

Git identityの既定候補はhostの`git config --global user.name`と`user.email`。候補を表示して明示確定させ、空文字と改行を拒否する。

### 13.3 再実行

- config作成前の失敗: hostに作った空の`~/.sbxm`以外を変更しない
- config作成後の再実行: 値を変更せずprerequisiteだけ再検査
- loginやsetupの利用者キャンセル: exit code `10`
- config変更: MVPでは直接編集し、次回load時にvalidationする

## 14. 自動test

- Project ID validation、case正規化、Sandbox名の衝突耐性
- path導出、symlink拒否、metadata重複
- configのround trip、unknown version、permission
- atomic writeの各中断点
- locale優先順位、bootstrap fallback
- FTL完全性とsnapshot
- CLI argument関係とmutation前validation
- TTY/non-TTY、Esc、Ctrl-C
- command runnerのenvironment、timeout、stream
- compatibility fixtureの全parser
- external command失敗時のexit code mapping

## 15. 受入条件

- 方向性文書の識別子、path、exit codeを共通型で表現できる
- `init`を新規・再実行・失敗後再実行できる
- configとmetadataの不正を自動修復しない
- 全利用者向け出力が日英で生成される
- 外部commandをshellなしで実行し、secretと`SSH_AUTH_SOCK`を規則どおり扱う
- 対応するDocker Sandboxes exact versionとJSON fixtureがreview済みである
- daemon安全性probeの結論が記録され、Phase 2が利用する方式が一意に決まっている
