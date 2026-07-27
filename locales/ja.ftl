locale-name = 日本語 / Japanese

cli-about = 案件ごとのDocker Sandboxを構築、接続、診断、破棄します。
cli-heading-usage = 使い方 (Usage):
cli-heading-commands = command (Commands):
cli-heading-options = option (Options):
cli-heading-arguments = 引数 (Arguments):
cli-lang-help = この実行の表示言語 ({ $supported })
cli-help-help = helpを表示する
cli-version-help = versionを表示する

cli-init-about = sbxmのglobal設定を作成します
cli-init-base-path-help = 案件directoryを置くdirectoryのabsolute path
cli-init-git-user-name-help = Sandbox内へ設定するGitのuser.name
cli-init-git-user-email-help = Sandbox内へ設定するGitのuser.email

cli-add-about = GitHub repositoryを管理対象へ登録してSandboxを構築します
cli-add-project-help = 対象案件をowner/repository形式で指定します
cli-add-worktrees-help = 作成するmanaged worktreeの数 (1〜32)
cli-add-detach-help = detached modeで全managed worktreeの起点にするremote branch

cli-sync-files-about = global設定で宣言したfileをrunning Sandboxへ再配置します
cli-sync-files-project-help = 対象案件をowner/repository形式で指定します

cli-rebuild-about = 編集したDockerfileをSandbox再作成によって適用します
cli-rebuild-project-help = 対象案件をowner/repository形式で指定します

cli-open-about = 必要ならSandboxを起動し、SSHで接続します
cli-open-project-help = 対象案件をowner/repository形式で指定します

cli-stop-about = 起動中のSandboxを停止します
cli-stop-project-help = 対象案件をowner/repository形式で指定します

cli-ls-about = 管理案件とSandboxの状態を一覧します

cli-status-about = host環境または1案件をread-onlyで診断します
cli-status-project-help = 対象案件をowner/repository形式で指定します
cli-status-global-help = 案件ではなくhostとglobal環境を診断します

cli-destroy-about = 対象案件のSandboxとsbxmの管理情報を破棄します
cli-destroy-project-help = 対象案件をowner/repository形式で指定します
cli-destroy-force-help = データ保護検査とactive session検査を省略して削除します

error-invalid-arguments = 引数を解釈できませんでした。
error-unknown-argument = 未知の引数です: { $argument }
error-invalid-value = { $argument } に値 { $value } は指定できません。
error-missing-required-argument = 必須の引数が不足しています: { $argument }
error-missing-subcommand = commandの指定が必要です。
error-unknown-subcommand = 未知のcommandです: { $subcommand }
error-conflicting-arguments = 次の引数は同時に指定できません: { $arguments }
error-invalid-lang = 表示言語に値 { $value } は指定できません。指定できる値: { $supported }
error-init-incomplete-options = 対話modeでは次のoptionをすべて省略し、option modeではすべて指定します。不足: { $missing }
error-worktrees-out-of-range = managed worktreeの数は { $minimum } 以上 { $maximum } 以下です。指定値: { $value }
error-worktrees-require-detach = managed worktreeを2個以上作る場合は起点branchの明示が必要です。
error-project-argument-required = 対話端末ではない実行では、{ $command } にowner/repositoryの完全指定が必要です。
error-status-scope-required = global環境か1案件のどちらか一方だけを指定してください。
error-not-implemented = { $command } はこのbuildでは未実装です。

usage-hint = { $usage }

error-invalid-project-id = { $value } はowner/repository形式として正しくありません。
error-reserved-repository-name = { $value } は予約語のためrepository名に使用できません。

error-config-missing = global設定が { $path } に見つかりません。
error-config-unreadable = { $path } のglobal設定を読み取れません: { $detail }
error-config-invalid-syntax = { $path } のglobal設定がTOMLとして不正です: { $detail }
error-config-unknown-version = { $path } のglobal設定はversion { $version } ですが、このbuildが対応するのは { $supported } です。
error-config-missing-field = { $path } のglobal設定に必須項目 { $field } がありません。
error-config-invalid-value = { $path } の項目 { $field } の値が不正です: { $detail }
error-base-path-not-absolute = base path { $path } がabsolute pathではありません。
error-base-path-not-directory = base path { $path } は存在しますがdirectoryではありません。
error-base-path-not-writable = base path { $path } へ現在の利用者が書き込めません。
error-file-declaration-invalid-source = 宣言file { $index } のsource { $source } が不正です: { $detail }
error-file-declaration-invalid-destination = 宣言file { $index } のdestination { $destination } が不正です: { $detail }
warning-config-unknown-key = { $path } の未知のkey { $key } を無視しました。

error-metadata-unreadable = { $path } の案件metadataを読み取れません: { $detail }
error-metadata-invalid-syntax = { $path } の案件metadataがTOMLとして不正です: { $detail }
error-metadata-unknown-version = { $path } の案件metadataはversion { $version } ですが、このbuildが対応するのは { $supported } です。
error-metadata-missing-field = { $path } の案件metadataに必須項目 { $field } がありません。
error-metadata-invalid-value = { $path } の項目 { $field } の値が不正です: { $detail }
error-metadata-path-mismatch = { $path } のmetadataは { $canonical_id } を宣言しており、本来の場所は { $expected } です。
error-metadata-duplicate-project = { $canonical_id } を複数の案件directoryが宣言しています: { $paths }
error-sandbox-name-collision = Sandbox名 { $sandbox } が複数の案件から導出されています: { $projects }
error-invalid-branch-name = { $value } はbranch名として使用できません: { $detail }
error-target-configuration-mismatch = { $project } は { $stored } として構築するよう登録されていますが、この実行は { $requested } を指定しています。
error-rebuild-intent-pending = { $project } は再構築の途中であるため、初回構築を継続できません。

error-host-clone-unusable = { $path } のcloneはこの案件には使用できません: { $detail }
error-image-unusable = image { $image } はこの案件には使用できません: { $detail }
error-build-context-not-empty = build context { $path } に { $observed } 件のentryがあります。sbxmは空のcontextからだけbuildします。
warning-build-context-left-behind = 一時build context { $path } を削除できませんでした: { $detail }
error-archive-unusable = Template archive { $path } は使用できません: { $detail }
error-template-unusable = Template { $template } は使用できません: { $detail }
error-sandbox-unusable = Sandbox { $sandbox } はこの案件には使用できません: { $detail }
error-declared-file-unusable = 宣言file { $source } は配置できません: { $detail }
error-declared-file-conflict = { $destination } には別の内容があるため、{ $source } を配置しませんでした。
error-sandbox-identity-mismatch = { $sandbox } では { $key } が既に { $observed } であり、この案件が期待する値は { $expected } です。
error-github-secret-missing = Sandbox { $sandbox } に { $secret } secretがないため、repositoryへaccessできません。
error-sandbox-repository-unusable = Sandbox内の { $path } はこの案件には使用できません: { $detail }
error-start-ref-unresolved = { $project } のremoteに { $reference } がありません。
error-no-managed-projects = 選択できる管理案件がありません。
error-project-not-managed = { $project } は管理対象の案件ではありません。
error-sandbox-not-created = { $project } は登録済みですが、Sandbox { $sandbox } はまだ存在しません。
error-sandbox-not-running = Sandbox { $sandbox } は { $observed } です。このcommandはrunningのSandboxだけを対象とします。
warning-dockerfile-changed-during-build = { $project } のDockerfileが初回構築の途中で変わったため、開始時の世代のまま構築を完了しました。現在の内容を反映するには { $command } を実行してください。
error-project-path-unexpected-type = { $path } は { $observed } ですが、sbxmはそこに { $expected } を必要とします。
error-project-path-unreadable = 案件のpath { $path } を読み取れません: { $detail }

error-atomic-write-failed = { $path } への書き込みに失敗しました: { $detail }
error-temp-file-left-behind = 中断した実行の一時file { $path } が残っています。
error-target-appeared-concurrently = 作成中に { $path } が出現したため、何も上書きしませんでした。
error-target-changed-concurrently = 書き換え中に { $path } が別のfileへ差し替わったため、何も上書きしませんでした。
error-lock-timeout = ほかのsbxmの実行が { $path } のlockを保持しています。{ $seconds } 秒待機しました。
error-lock-unavailable = { $path } のlockを取得できません: { $detail }

error-external-command-not-found = command { $program } がPATH上に見つかりません。
error-external-command-spawn-failed = command { $program } を起動できません: { $detail }
error-external-command-failed = command { $program } が { $exit_status } で失敗しました。
error-external-command-timeout = command { $program } が { $seconds } 秒以内に終了しなかったため停止しました。
error-external-output-unparseable = { $program } の出力を解釈できません: { $detail }
warning-external-output-lossy = { $program } の { $stream } 出力がUTF-8として不正なため、置換文字を含む形へ変換しました。
external-invocation = 実行したcommand: { $program } { $args }
external-working-directory = 作業directory: { $path }
external-output-heading = { $program } の出力:

error-sbx-version-unparseable = { $observed } からDocker Sandboxes CLIのversionを判定できません。
error-sbx-version-below-minimum = Docker Sandboxes CLI { $observed } は要件の { $minimum } より古いversionです。
error-platform-unsupported = このbuildが対応するのは { $expected } です。観測値: { $observed }
error-platform-unobservable = platformを判定できません: { $detail }
error-host-command-missing = command { $command } がPATH上に見つかりません。
error-docker-unreachable = Docker daemonが応答しません: { $detail }
error-network-policy-mismatch = network policyは { $observed } ですが、このbuildが検証済みなのは { $expected } だけです。
error-network-policy-unobservable = network policyを読み取れません: { $detail }
error-daemon-unobservable = Docker Sandboxes daemonの状態を読み取れません: { $detail }
error-sbx-login-missing = このhostはDocker Sandboxesへloginしていません。
error-sbx-login-unobservable = このhostがDocker Sandboxesへloginしているかを読み取れません: { $detail }
error-daemon-session-active = { $sandboxes } にsessionが接続しているため、Docker Sandboxes daemonを変更しませんでした。
error-daemon-session-unobservable = このDocker Sandboxes versionは { $sandbox } へのsession接続を示さないため、daemonを変更しませんでした。
remediation-run-help = { $command } を実行すると指定できる引数を確認できます。
remediation-run-init = sbxm init を実行してglobal設定を作成してください。
remediation-host-clone-unusable = { $path } を確認し、退避するかoriginを直してから、もう一度実行してください。
remediation-daemon-session-active = 対象Sandboxへ接続しているshellとeditorを終了してから、もう一度実行してください。
remediation-daemon-session-unobservable = 接続中のsessionを示すversionのDocker Sandboxes CLIへ更新してから、もう一度実行してください。
remediation-declared-file-conflict = 両者を確認したうえで、sbxm sync-files を実行すると宣言fileでSandbox側を置き換えられます。
remediation-sandbox-identity-mismatch = 誰のSandboxかを確認してください。sbxmは設定済みの値を上書きしません。
remediation-github-secret-missing = 対象repositoryに限定したfine-grained personal access tokenを、Contents read/writeとMetadata readで発行してください。必要な場合だけPull requests、Issues、Actionsを追加します。{ $command } で登録してから、同じcommandをもう一度実行してください。
remediation-sandbox-repository-unusable = Sandbox内の { $path } を確認してください。sbxmは場所を空けるためにrepositoryやworktreeを削除しません。
remediation-start-ref-unresolved = GitHub上のbranch名を確認し、branchが存在する状態でもう一度実行してください。
remediation-project-not-managed = { $command } を実行すると登録して構築できます。
remediation-sandbox-not-created = { $command } を実行するとSandboxを構築できます。
remediation-sandbox-not-running = { $command } でSandboxを起動してから、もう一度実行してください。
remediation-sbx-login = { $command } を実行してloginを完了してから、もう一度実行してください。
remediation-no-managed-projects = { $command } を実行すると最初の案件を登録できます。
remediation-run-rebuild = { $command } を実行して再構築を完了してください。
remediation-target-configuration-mismatch = 保存済みの目標構成で続けるには { $command } をoptionなしで実行し、別の構成で作り直すには先に案件を破棄してください。
remediation-remove-temp-file = { $path } の内容を確認し、ほかの実行が使用していないことを確かめてから削除してください。
remediation-fix-config = { $path } を編集してからもう一度実行してください。
remediation-install-command = { $command } を導入し、PATH上に置いてください。
remediation-start-docker = Docker Desktopを起動し、engineがrunningになるまで待ってください。
remediation-network-policy = 続行する前にDocker Sandboxesのnetwork policyを { $expected } に設定してください。
remediation-wait-for-lock = ほかのsbxmの実行が終わるのを待ってから、もう一度実行してください。
security-config-permission-description = { $path } のmodeは { $observed } で、所有者以外にも権限があります。この機械上のほかのaccountが読み書きできる設定fileをsbxmは使用しません。
security-config-permission-remediation = chmod { $expected } { $path } を実行し、fileの所有者が自分であることを確認してください。

security-config-symlink-description = { $path } はsymbolic linkです。追跡するとsbxmの設定directory外のfileを読み書きする可能性があります。
security-config-symlink-remediation = { $path } を自分が所有する通常fileへ置き換えるか、実体の設定fileをこのpathへ戻してください。

security-config-dir-permission-description = { $path } のmodeは { $observed } で、所有者以外にも権限があります。そこへ置くlockと設定を観測または置換される可能性があります。
security-config-dir-permission-remediation = chmod { $expected } { $path } を実行し、directoryの所有者が自分であることを確認してください。

security-config-dir-symlink-description = { $path } はsymbolic linkです。lockと設定が意図しないdirectoryへ作られます。
security-config-dir-symlink-remediation = { $path } を自分が所有するdirectoryへ置き換えてください。

security-project-path-symlink-description = { $path } はsymbolic linkです。追跡すると案件directoryの外へfileを作成または置き換えるため、sbxmは追跡しません。
security-project-path-symlink-remediation = { $path } を自分が所有する通常fileまたはdirectoryへ置き換えてから、もう一度実行してください。

security-project-file-permission-description = { $path } のmodeは { $observed } で、所有者以外にも権限があります。この機械上のほかのaccountが読み書きできるfileをsbxmは使用しません。
security-project-file-permission-remediation = chmod { $expected } { $path } を実行し、fileの所有者が自分であることを確認してください。

security-base-path-escape-description = { $path } はsymbolic linkの解決後に { $resolved } となります。案件が指定したdirectoryの外に作られます。
security-base-path-escape-remediation = 解決後も意図したdirectory配下に収まるbase pathを指定してください。

init-already-initialized = sbxmは初期化済みです。設定fileは { $path } です。
init-created = global設定を { $path } に作成しました。
init-next-step = sbxm status --global を実行するとhost環境を診断できます。
init-prompt-language = 表示言語
init-prompt-base-path = 案件directoryを置くdirectoryのabsolute path
init-prompt-git-user-name = Sandbox内で使うGitのuser.name
init-prompt-git-user-email = Sandbox内で使うGitのuser.email
init-prompt-create-base-path = { $path } はまだ存在しません。作成しますか?
error-init-requires-tty = 対話modeの初期化には、標準入力と標準エラー出力の両方が端末である必要があります。
error-git-identity-invalid = Gitの { $field } の値は使用できません: { $detail }
detail-value-empty = 値が空です
detail-value-has-newline = 値に改行が含まれています

status-global-section = GLOBAL
status-column-item = 項目 (ITEM)
status-column-status = 状態 (STATUS)
status-item-config = 設定 (Config)
status-item-base-path = base path (Base path)
status-item-platform = platform (Platform)
status-item-git = Git
status-item-ssh = SSH
status-item-docker = Docker
status-item-docker-sandboxes = Docker Sandboxes
status-item-network-policy = network policy (Network policy)
status-item-daemon = daemon (Daemon)
add-field-project = 案件 (Project)
add-field-sandbox = Sandbox
add-field-creation-mode = 作成mode (Creation mode)
add-field-start-branch = 起点branch (Start branch)
add-field-managed-worktrees = managed worktree数 (Managed worktree count)
add-field-host-clone = host clone (Host clone)
add-field-sandbox-state = Sandboxの状態 (Sandbox state)
column-worktree = worktree (WORKTREE)
column-created-from = 作成元 (CREATED FROM)
column-head = HEAD
column-mode = mode (MODE)
column-file = file (FILE)
column-destination = 配置先 (DESTINATION)
column-result = 結果 (RESULT)
add-already-built = { $project } は構築済みのため、何も変更しませんでした。
add-mise-heading = 次のmanaged worktreeはmiseの設定を持ちます。sbxmはmiseを自動実行しません:
add-mise-hint = 使用する場合は、Sandbox内で mise trust と mise install を実行してください。
sync-files-done = { $project } の宣言file { $count } 件を { $sandbox } へ配置しました。
legend-attached = remoteをtrackingするbranch上のworktreeです
legend-detached = branchを持たないcommit上のworktreeです
legend-placed = Sandboxへ書き込みました
legend-unchanged = Sandboxに同じ内容が既にありました

status-item-login = Docker Sandboxesへのlogin (Docker Sandboxes login)
status-item-session-inspection = active session検査 (Active session inspection)
legend-heading = 状態値の凡例
legend-ready = 期待どおり利用できます
legend-missing = 存在しません
legend-error = 検証できないか、要件を満たしていません
legend-not-created = 案件は登録済みですが、Sandboxはまだ存在しません
legend-running = serviceが起動しています
legend-stopped = serviceは導入済みですが起動していません
