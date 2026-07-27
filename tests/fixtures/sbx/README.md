# Docker Sandboxes CLI fixtures

Docker Sandboxes CLIはEarly Accessであり、参照資料の現在内容よりも、対象Macで採取して
commitしたexact-version fixtureを実装上の契約とする。

## このdirectoryの構成

```text
tests/fixtures/sbx/
├── README.md
├── collect.sh          # 対象Mac上でfixtureを採取するscript
├── synthetic/          # 実機出力ではない合成データ
└── <exact-version>/    # 実機で採取したfixture（未採取）
```

## 現在の状態: 実機fixtureは未採取

`compatibility.toml`の`validated_cli_versions`は空である。この状態のsbxmは、検出した
Docker Sandboxes CLIのversionを解釈できないものとして扱い、`sbx`の出力に依存する検査を
`sbx-fixtures-not-collected`で停止する。read-only診断もmutationも行わない。

これは未実装ではなく、方向性文書の「外部状態を観測できない場合に推測した状態を返さない」と、
Phase 1仕様の「安全性に必要な出力を解釈できないversionではmutationを行わない」を、
fixture未採取の状態へそのまま適用した結果である。

実機採取には、macOS Sonoma 14以降のApple silicon Mac、Docker Desktop、Docker Sandboxes
CLI 0.37.0以上が必要であり、Phase 1のPRを作成した環境では満たせなかった。

## `synthetic/`の位置付け

`synthetic/`のJSONは、対象CLIから採取した出力**ではない**。parserが

- 必須fieldを欠く出力をparse不能として扱うこと
- 未知のstateを既知値へ丸めないこと
- 現在のnetwork policyを一意に特定できない出力を拒否すること

を検証するためだけに置いた合成データである。これらをversion契約の根拠にしない。
実機fixtureの代わりとして`validated_cli_versions`へversionを追加してはならない。

## 採取手順

対象Mac上で次を実行する。

```sh
tests/fixtures/sbx/collect.sh
```

scriptは`sbx version`から検出したexact versionのdirectoryを作り、Phase 1で必要な
read-only出力を保存する。採取後は同じPRで次をすべて行う。

1. `<exact-version>/`のfixtureをcommitする
2. `compatibility.toml`の`validated_cli_versions`へそのexact versionを追加する
3. `src/compatibility.rs`のparserを、採取した実際のschemaへ合わせて厳密化する
4. `this_build_has_no_validated_versions_yet` testを、採取したversionを固定するtestへ差し替える
5. 代表的失敗とparse不能のtestを、実際の出力に基づいて追加する

`sbx ls --json`のschemaが変わった場合は`ls_json_fixture_version`も上げる。

## Phase 1で採取するfixture

| File | 取得元 |
|---|---|
| `version.txt` | `sbx version` |
| `help.txt` | `sbx --help` |
| `ls-empty.json` | Sandboxが0件の`sbx ls --json` |
| `ls-running.json` | 起動中Sandboxがある`sbx ls --json` |
| `ls-stopped.json` | 停止中Sandboxがある`sbx ls --json` |
| `daemon-status-running.json` | daemon起動中の`sbx daemon status` |
| `daemon-status-stopped.json` | daemon停止中の`sbx daemon status` |
| `policy-ls-balanced.json` | network policyが`Balanced`の`sbx policy ls` |
| `policy-ls-unsupported.json` | `Balanced`以外を選択した`sbx policy ls` |

## 後続Phaseで採取するfixture

各workflowの実装PRで、そのworkflowが使用するsubcommandの`--help`、読み取るstructured
output、secret存在確認のread-only出力、create・exec・stop・rm・Template操作の正常と
代表的失敗のexit statusを同時に追加する。

Sandbox mutationを安全と判定する受入testは、`daemon-security.md`にdaemon安全性probeの
結果を記録するまで未完了のままとする。probeの内容はPhase 1仕様の12章を正本とする。

## Redaction

fixtureをcommitする前に、Mac user名、token、公開鍵、実案件のrepository名、host pathを
置換する。`collect.sh`はhome directoryのpathを`/Users/example`へ置換するが、置換結果は
必ず目視で確認する。
