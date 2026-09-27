/// Sandboxの中で、stdinで受け取ったbyte列を宣言fileとして置く手順。
///
/// 受け取ったbyte列はrootだけが読める一時fileへ書き、digestが宣言fileと一致した場合だけ
/// `agent`所有の`pending`へ写し、renameで`destination`を置き換える。一時fileは成否に
/// かかわらず消す。引数は`$1`が`destination`、`$2`が`pending`、`$3`が期待するdigest。
/// 受け取ったbyte列が一致しなければ`TRANSFER_INCOMPLETE`で終わり、何も置き換えない。
///
/// HUP・INT・TERMをどの時点で受けても、一時fileを残さない。受け方を置けないSIGKILLは除く。
///
/// - 一時fileを作る前に、消す手順とsignalの受け方を置く。dashはsignalで終わるshellのEXIT
///   trapを走らせないため、signalは`exit`へ変えてEXIT trapを通す。`staged`を空で始めるのは、
///   `set -u`の下でもtrapが名前を読め、まだ何も作っていないと分かるためである。
/// - trapを置いたshellは、`$(...)`の途中で届いたsignalを代入が終わってから処理するため、
///   作った一時fileの名前を失わない。ただしprocess group全体へ送られたsignalは`mktemp`にも
///   届き、fileを作ってから名前を書くまでに`mktemp`が終わると、誰も名前を知らないfileが
///   残る。そこで置換の中だけでsignalを無視する。無視はexecを越えて`mktemp`へ引き継がれる。
///   shell自身は無視しないため、同じsignalを捨てずに代入のあとで処理する。無視を置く前に
///   置換が終わらされた場合は、まだ何も作っていない。
/// - 消す手順は、まずsignalを無視する。消している途中のsignalは、trapの`exit`で`rm`を
///   飛ばすか、`rm`そのものを終わらせるためである。EXIT trapが無視を置くより先にsignalが
///   届くと、signalのtrapが割り込み、その`exit`でEXIT trapの残りを飛ばす。signalのtrapも
///   自分で消してから終わる。
pub(super) const PLACE_FROM_STDIN: &str = r#"set -eu
staged=
remove='trap "" HUP INT TERM; [ -z "$staged" ] || rm -f "$staged"'
trap "$remove" EXIT
trap "$remove; exit 143" HUP INT TERM
umask 077
staged=$(trap '' HUP INT TERM; mktemp)
cat > "$staged"
received=$(sha256sum "$staged")
[ "${received%% *}" = "$3" ] || exit 65
install -o agent -g agent -m 0600 "$staged" "$2"
mv -f "$2" "$1""#;
