set -eu
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
mv -f "$2" "$1"