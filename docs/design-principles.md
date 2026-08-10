# sbxm設計原則

この文書は、個別の機能計画を横断して適用するsbxm共通の設計方針を定める。破壊前保護の
具体的なRust moduleは`src/support/protection`にあり、この文書は言語やfile構成に依存しない
判断基準を記録する。

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

## 破壊前保護は共通ゲートを通す

通常の`rebuild`と`destroy`は、呼び出し側が個別の検査を選ぶのではなく、同じ保護ゲートを通る。
ゲートは固定順序でworktree、Git操作、現在のStepで定義されたorigin回収可能性を観測し、既知の
危険状態を一件で打ち切らず評価へ集める。ゲートの外からprivateなcollectorを直接呼び出さない。

層Aのblockerが一件でもある場合、利用者への明示確認で安全にできるとは考えず、削除計画や確認を
始める前に拒否する。blockerが無い場合だけ、通常経路を先へ進める。`destroy --force`はこの
保護を意図的に迂回する別操作であり、通常のrecovery手順や診断のcopy commandとして案内しない。

層Aの拒否理由は原因ごとに安定したerror codeを持つ。tracked changes、untracked paths、Git
operation、upstreamやorigin回収可能性、管理外worktree、観測不能を一つの汎用errorへ丸めない。

## 観測不能は成功ではない

外部commandが起動できない、終了状態を解釈できない、出力が未知または不完全である、必要な
worktreeやrefを列挙できない、といった場合はcleanや不存在として扱わない。検査段階ごとの
diagnosticへ写像し、元のprogram、exit status、stderrなどのexternal causeを保持する。

特にGitのstructured outputは、成功終了していても既知の形式として検証できないrecordを受け入れ
ない。解釈できない状態では削除へ進まない。

## 診断は事実と対処を分ける

利用者が「何が危険か」「どこを確認するか」「何を済ませれば再実行できるか」を一つの診断から
追えるようにする。

- error codeは翻訳しない安定IDとして先頭に表示する
- 英日descriptionは危険の意味を一文で説明し、worktree、operation、ref、commit、件数などの
  可変値を文章へ連結しない
- 可変値は名前付き`Fact`として表示し、remote URL、credential、secret、file内容は保持・表示しない
- remediationの説明と、正確に組み立てられるread-only commandを分離する
- project IDから安全に組み立てられる`sbxm open <project>`または`sbxm status <project>`だけを
  command blockへ載せる
- `git clean`、`git reset --hard`、`git worktree remove`などの破壊commandや、保護を迂回するforce
  commandを自動実行・copy推奨しない
- blockerが複数ある場合は安定順序で全件を同じrunに表示する

## Stepの責務境界

Step 1（#79）は、通常のrebuild/destroyを共通ゲートへ接続し、層Aの原因別blocker、fail-closedな
観測、Facts、英日diagnostic、read-only remediationを成立させる。ここで実際にconsumerが無い
将来型を先行実装しない。

後続Stepは、同じゲートとassessmentを拡張する。

- #81はprotected removeとforced removeの実際の境界を閉じ、そこで使用するopaqueなpermit/bypass
  をconsumerと同時に導入する
- #82は層Bの損失収集、削除計画、明示確認、状態fingerprintを導入する
- #83はoriginの権威ある観測とcommit reachability分類を導入する

後続Stepは別の破壊可否判定や別のゲートを作らず、このStepの責務を上書きしない。
