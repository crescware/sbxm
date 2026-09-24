/// Sandboxの中で、stdinで受け取ったbundleを`$1`へ置く手順。引数は`$2`が期待するdigest。
///
/// 受け取ったbyte列は置き場所と同じdirectoryの一時fileへ書く。bundleは大きくなりうる
/// ため、別のfile systemへ書いてから写し直さない。digestが一致した場合だけrenameで
/// 置き換え、一致しなければ`TRANSFER_INCOMPLETE`で終わる。一時fileは成否にかかわらず
/// 残さない。
pub(super) const PLACE_BUNDLE: &str = r#"set -eu
umask 077
destination=$1
mkdir -p "${destination%/*}"
staged=$(mktemp "${destination%/*}/.receiving.XXXXXX")
trap 'rm -f "$staged"' EXIT
cat > "$staged"
received=$(sha256sum "$staged")
[ "${received%% *}" = "$2" ] || exit 65
mv -f "$staged" "$destination""#;
