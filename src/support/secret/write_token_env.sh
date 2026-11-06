# sbxm token environment write
set -eu
temporary=
cleanup='[ -z "$temporary" ] || rm -f -- "$temporary"'
trap "$cleanup" EXIT
trap "$cleanup; exit 143" HUP INT TERM
umask 077
temporary=$(trap '' HUP INT TERM; mktemp "${4}.sbxm-new.XXXXXXXXXX")
printf '%s\n' "$1" "$2" "$3" > "$temporary"
chmod 0644 "$temporary"
sync "$temporary"
mv -f -- "$temporary" "$4"
temporary=
sync "${4%/*}"
