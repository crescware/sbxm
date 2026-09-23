/// Sandboxの中で、stdinで受け取ったbyte列を宣言fileとして置く手順。
///
/// 受け取ったbyte列はrootだけが読める一時fileへ書き、digestが宣言fileと一致した場合だけ
/// `agent`所有の`pending`へ写し、renameで`destination`を置き換える。一時fileは成否に
/// かかわらず消す。引数は`$1`が`destination`、`$2`が`pending`、`$3`が期待するdigest。
/// 受け取ったbyte列が一致しなければ`TRANSFER_INCOMPLETE`で終わり、何も置き換えない。
pub(super) const PLACE_FROM_STDIN: &str = r#"set -eu
umask 077
staged=$(mktemp)
trap 'rm -f "$staged"' EXIT
cat > "$staged"
received=$(sha256sum "$staged")
[ "${received%% *}" = "$3" ] || exit 65
install -o agent -g agent -m 0600 "$staged" "$2"
mv -f "$2" "$1""#;
