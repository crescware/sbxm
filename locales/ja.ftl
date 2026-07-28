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
error-sandbox-name-duplicated = Sandbox一覧に { $sandbox } という名前のSandboxが複数あるため、案件と対応付けられません。
error-invalid-branch-name = { $value } はbranch名として使用できません: { $detail }
error-target-configuration-mismatch = { $project } は { $stored } として構築するよう登録されていますが、この実行は { $requested } を指定しています。
error-rebuild-intent-pending = { $project } は再構築の途中であるため、初回構築を継続できません。

error-host-clone-unusable = { $path } のcloneはこの案件には使用できません: { $detail }
error-image-collision = image { $image } は既に存在し、別の内容を宣言しているため、この世代はその名前を使えません。{ $detail }
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
error-selection-unresolved = 選択された { $index } は、候補 { $count } 件のいずれでもありません。
error-sandbox-still-running = Sandbox { $sandbox } は停止を要求したあとも起動したままです。
error-unsaved-work-uncommitted = { $target } にcommitされていない変更があります。
error-unsaved-work-in-progress = { $target } はGit操作の途中です ({ $operation })。
error-unsaved-work-no-upstream = { $target } がcheckoutしているbranchにupstreamがないため、commitはSandboxの中にしかありません。
error-unsaved-work-unpushed = { $target } には、{ $upstream } が持っていないcommitが { $count } 件あります。
error-unsaved-work-unreachable = { $target } のdetached HEADには、remoteのどのbranchからも到達できないcommitがあります。
error-worktree-outside-repository = worktree { $path } は共有repository { $root } の外にあるため、この案件の成果物として扱えません。
error-managed-worktree-missing = 案件はmanaged worktree { $path } を宣言していますが、Sandbox内のGitの一覧にありません。
error-worktrees-not-observed = { $root } の下にworktreeが1件もないため、Sandboxが持つ内容を判定できません。
error-unmanaged-worktree-present = worktree { $path } は目標構成に含まれず、再構築では元の配置を再現できません。
error-sandbox-still-present = Sandbox { $sandbox } は削除後も一覧に残っています。
error-rebuild-generation-missing = { $project } の再構築は世代 { $target } に固定されていますが、その成果物も一致するDockerfileもありません。現在のDockerfileは { $observed } です。
error-destroy-not-confirmed = Sandbox名が { $sandbox } と完全一致しなかったため、何も削除しませんでした。
error-sandbox-check-unobservable = Sandbox内の { $subject } に対する検査が、結果を示さないまま { $exit_status } で終了したため、判定できません。
error-global-scope-unobservable = host環境を読み取れないため、この案件の一部を検査できませんでした。
error-project-not-managed = { $project } は管理対象の案件ではありません。
error-sandbox-not-created = { $project } は登録済みですが、Sandbox { $sandbox } はまだ存在しません。
error-sandbox-not-running = Sandbox { $sandbox } は { $observed } です。このcommandはrunningのSandboxだけを対象とします。
warning-dockerfile-changed-during-rebuild = { $project } のDockerfileは、再構築が世代を固定したあとに変更されました。この実行は固定済みの世代を適用しています。現在のDockerfileを適用するには { $command } をもう一度実行してください。
warning-lock-file-left-behind = 案件の管理は解除しましたが、lock file { $path } を削除できませんでした: { $detail }
warning-dockerfile-changed-during-build = { $project } のDockerfileが初回構築の途中で変わったため、開始時の世代のまま構築を完了しました。現在の内容を反映するには { $command } を実行してください。
error-project-path-unexpected-type = { $path } は { $observed } ですが、sbxmはそこに { $expected } を必要とします。
error-project-path-unreadable = 案件のpath { $path } を読み取れません: { $detail }

error-atomic-write-failed = { $path } への書き込みに失敗しました: { $detail }
error-cleanup-failed = { $path } を削除できませんでした: { $detail }
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
error-remote-ssh-unconfigured = sshに { $host } 向けのproxy設定がないため、SandboxへSSHで接続できません。
error-remote-ssh-unobservable = Sandbox向けのSSH設定を読み取れません: { $detail }
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
remediation-diagnose-project = { $command } を実行すると現在の状態を確認できます。
remediation-run-global-status = { $command } を実行するとhost環境を診断できます。
remediation-remote-ssh-unconfigured = このhostでDocker SandboxesのRemote SSH連携を設定してから、もう一度実行してください。
remediation-unsaved-work = 残す変更はcommitしてpushし、不要なfileは削除してから、もう一度実行してください。
remediation-worktree-outside-repository = そのpathの内容はご自身で確認してください。sbxmは説明のつかないworktreeを削除しません。
remediation-managed-worktree-missing = { $command } を実行してSandboxの現在の内容を確認し、差を解消してから削除してください。
remediation-worktrees-not-observed = { $command } を実行してSandboxの現在の内容を確認してください。sbxmは読み取れないrepositoryを操作しません。
remediation-unmanaged-worktree-present = そのworktreeの作業を保存したうえで、Sandbox内のgitで削除してから、もう一度rebuildを実行してください。
remediation-rebuild-generation-missing = その世代のDockerfileを復元してから、もう一度 rebuild を実行してください。復元できない場合、{ $command } はhost cloneとDockerfileを残したままSandboxと管理情報を削除します。
remediation-destroy-force = 内部を確認せずに削除するには { $command } を実行してください。データ保護検査とactive session検査は省略されます。
remediation-run-rebuild = { $command } を実行して再構築を完了してください。
remediation-target-configuration-mismatch = 保存済みの目標構成で続けるには { $command } をoptionなしで実行し、別の構成で作り直すには先に案件を破棄してください。
remediation-image-collision = { $image } の内容をご自身で確認し、不要であることを確かめてから削除または改名してください。sbxmは自分がbuildしていないimageを上書きしません。
remediation-cleanup-failed = ほかに必要とするものがないことを確かめてから { $path } をご自身で削除し、もう一度実行してください。
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

security-config-owner-description = { $path } の所有者はuser ID { $observed } で、現在の利用者は { $expected } です。ほかのaccountが所有する設定fileは、いつでも置き換えられる可能性があります。
security-config-owner-remediation = { $path } を退避してsbxmに作り直させるか、自分が所有するfileをこのpathへ戻してください。

security-config-dir-permission-description = { $path } のmodeは { $observed } で、所有者以外にも権限があります。そこへ置くlockと設定を観測または置換される可能性があります。
security-config-dir-permission-remediation = chmod { $expected } { $path } を実行し、directoryの所有者が自分であることを確認してください。

security-config-dir-symlink-description = { $path } はsymbolic linkです。lockと設定が意図しないdirectoryへ作られます。
security-config-dir-symlink-remediation = { $path } を自分が所有するdirectoryへ置き換えてください。

security-config-dir-owner-description = { $path } の所有者はuser ID { $observed } で、現在の利用者は { $expected } です。lockと設定が、ほかのaccountの管理下にあるdirectoryへ置かれます。
security-config-dir-owner-remediation = { $path } を退避してsbxmに作り直させるか、自分が所有するdirectoryをこのpathへ戻してください。

security-project-path-symlink-description = { $path } はsymbolic linkです。追跡すると案件directoryの外へfileを作成または置き換えるため、sbxmは追跡しません。
security-project-path-symlink-remediation = { $path } を自分が所有する通常fileまたはdirectoryへ置き換えてから、もう一度実行してください。

security-project-file-permission-description = { $path } のmodeは { $observed } で、所有者以外にも権限があります。この機械上のほかのaccountが読み書きできるfileをsbxmは使用しません。
security-project-file-permission-remediation = chmod { $expected } { $path } を実行し、fileの所有者が自分であることを確認してください。

security-project-path-owner-description = { $path } の所有者はuser ID { $observed } で、現在の利用者は { $expected } です。modeにかかわらず、ほかのaccountが所有するpathの上に案件を構築しません。
security-project-path-owner-remediation = { $path } を退避してsbxmに作り直させるか、自分が所有するpathをこのpathへ戻してください。

security-ssh-agent-exposed-description = { $sandbox } からhostのSSH Agentへ到達できます。Sandbox内のagentが利用者の鍵で署名できる状態です。
security-ssh-agent-exposed-remediation = Sandboxを停止し、sbxm で開き直すと、SSH Agentを渡さないdaemonで起動します。

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
ls-projects-section = 管理案件 (PROJECTS)
ls-unmanaged-section = 管理外のSandbox (UNMANAGED SANDBOXES)
column-project = 案件 (PROJECT)
column-sandbox = Sandbox
column-state = 状態 (STATE)
column-workspace = workspace (WORKSPACE)
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
files-secret-hint = 宣言fileは設定を運ぶためのもので、credentialのためのものではありません。token、secret、秘密鍵は宣言fileへ入れず、Docker Sandboxesのsecret機能でSandboxへ渡してください。
sync-files-done = { $project } の宣言file { $count } 件を { $sandbox } へ配置しました。
legend-attached = remoteをtrackingするbranch上のworktreeです
legend-detached = branchを持たないcommit上のworktreeです
legend-managed = 案件が宣言しているworktreeで、再構築で作り直されます
legend-unmanaged = Sandboxの中で作られたworktreeで、作り直す仕組みはありません
legend-pushed = upstream branchがここのcommitをすべて持っています
legend-reachable = remoteのいずれかのbranchからこのcommitへ到達できます
legend-placed = Sandboxへ書き込みました
legend-unchanged = Sandboxに同じ内容が既にありました

status-item-login = Docker Sandboxesへのlogin (Docker Sandboxes login)
status-item-session-inspection = active session検査 (Active session inspection)
open-connecting = { $project } のSandbox { $sandbox } へ接続します。
open-worktrees = このSandboxのmanaged worktree:

destroy-confirm-prompt = 削除を確認するため、Sandbox名を入力してください
destroy-removes = 削除対象:
destroy-keeps = 保持対象:
destroy-target-sandbox = Sandbox { $sandbox } とその内部のすべて
destroy-target-host-images = このhostでbuildしたimageと、そこからloadしたTemplate
destroy-target-secrets = Docker Sandboxesへ登録したsecret
destroy-force-notice = force modeではデータ保護検査とactive session検査を省略します。
destroy-done = { $project } の管理を解除しました。
destroy-re-register = 再登録するには次を実行します: { $command }

column-branch = branch (BRANCH)
column-remote = remote (REMOTE)
select-destroy-heading = どの案件を破棄しますか?
rebuild-unchanged = { $project } のDockerfileは適用済みの内容と同じため、何も変更しませんでした。
rebuild-applied = { $project } を再構築しました。{ $sandbox } は世代 { $generation } で動作します。

select-open-heading = どの案件を開きますか?
select-stop-heading = どの案件を停止しますか?

legend-stopped-now = この実行で停止しました
legend-not-stopped = この実行では停止していません。既に停止しているか、Sandboxがないか、先行する失敗のあとそのままにしました
legend-failed = 停止できませんでした
status-project-section = 案件 (PROJECT)
status-worktrees-section = worktree (WORKTREES)
status-column-value = 値 (VALUE)
status-item-metadata = metadata (Metadata)
status-item-project-root = 案件root (Project root)
status-item-host-clone = host clone (Host clone)
status-item-dockerfile = Dockerfile
status-item-image = image (Image)
status-item-template-archive = Template archive
status-item-sandbox = Sandbox
status-item-workspace = workspace (Workspace)
status-item-secret = GitHub secret
status-item-bare-repository = bare repository (Bare repository)
status-item-ssh-agent = SSH Agent
status-item-worktrees = worktree (Worktrees)
status-item-project = 案件 (Project)
column-path = path (PATH)
column-kind = 種別 (KIND)
legend-mismatch = 観測した状態が案件の宣言と一致しません
legend-changed = 適用後に変更されており、次の rebuild で反映されます
legend-clean = commitしていない変更はありません
legend-dirty = 変更があります
legend-not-exposed = hostのSSH AgentへSandboxから到達できません
legend-exposed = hostのSSH AgentへSandboxから到達できます
legend-not-applicable = 対象のSandboxがありません
legend-not-observed-stopped = Sandboxが停止中のため、起動せずに検査を省きました
status-item-remote-ssh = Remote SSH
legend-heading = 状態値の凡例
legend-ready = 期待どおり利用できます
legend-missing = 存在しません
legend-error = 検証できないか、要件を満たしていません
legend-not-created = 案件は登録済みですが、Sandboxはまだ存在しません
legend-sandbox-running = Sandboxが起動しています
legend-sandbox-stopped = Sandboxはありますが起動していません
legend-running = serviceが起動しています
legend-stopped = serviceは導入済みですが起動していません
