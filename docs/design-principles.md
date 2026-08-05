# sbxm設計原則

この文書は、個別の機能計画を横断して適用するsbxm共通の設計方針を定める。

## 曖昧さは危険側に倒す

sbxmは、外部状態、永続状態、利用者の意図を一意に確認できない場合、安全であると推測して
mutationを続行しない。

具体的には、次を共通方針とする。

- 複数の解釈が成立する入力や状態を、名前、path、cwd、類似した値から推測して補完しない
- 正本同士の不一致を、いずれか一方が正しいものとして暗黙に修復しない
- ownershipまたは対応関係を確認できない既存成果物を、名前が一致するだけでadopt、上書き、
  移動、削除しない
- 外部状態を観測できないことを、存在しない、一致する、または安全であることと同一視しない
- 安全なmutation条件を満たさない場合はmutation前に拒否し、観測できた事実をerrorとして示す
- 自動修復を提供する場合は、通常操作の推測として実行せず、対象、根拠、変更範囲を明示した
  専用workflowとして設計する

この方針は利便性のために個別機能から暗黙に緩和しない。例外が必要な場合は、その機能の仕様で
曖昧性のない判定条件と許可するmutationを明記する。

## 破壊前保護は一つの契約に固定する（Issue #78 Step 1〜5）

`rebuild`と`destroy`の全ての通常経路は、`src/support/protection`が持つ共通の保護ゲートを通る。
呼び出し側は個別の検査を選ばない。

保護は回復可能性に基づく二層で構成する。

- **層A（拒否）**: 追跡対象ファイルの未commit変更、無視対象でない未追跡file、進行中のGit
  操作、管理外worktree（rebuildのみ）、originから回収できると証明できないcommitのいずれか
  1件でもあれば、確認を求めず削除しない。原因ごとに`ProtectionBlocker`のvariantと固有の
  `ErrorId`を持ち、共通の`UnsavedWork`のようなIDへ丸めない。
- **層B（明示確認）**: 無視対象のpath、originから回収できるcommitを指すローカルrefの名前、
  destroy対象の管理外worktreeの存在のように、commit自体は失わないが自動復元できない情報は、
  削除計画へ全件示し、対象sandbox名の完全一致入力を得た場合だけ削除を許可する。

状態を観測できない場合は、安全と推測せず層Aと同様に拒否する（`WorktreeInventoryUnobservable`
等の専用ID）。

`gate::assess`が観測を固定順序で行い、`gate::authorize`が層Aの通過だけを確認して不透明な
`ProtectionPermit`を発行する。`destroy --force`だけが、`ProtectionPermit`とは相互変換できない
別の型`ForceBypass`（`force_bypass::force_destroy`)で保護ゲートを迂回する。この迂回はarchitecture
testが唯一の呼び出し箇所であることを確認する。

層Bの収集・削除計画・明示確認・remove直前の再評価はIssue #82（Step 4）が、originの権威ある
観測と到達可能性の共通化はIssue #83（Step 5）が追加する。いずれも本節の型・層・fail-closedの
定義を拡張するのであり、別の分類やゲートを新設しない。
