# sbxm inside check
dir=$(mktemp -d) || exit 70
trap 'rm -rf "$dir"' EXIT
trap 'rm -rf "$dir"; exit 143' HUP INT TERM
i=0
while [ $# -gt 0 ]; do
  n=0
  for a in "$@"; do
    [ -n "$a" ] || break
    n=$((n + 1))
  done
  [ $n -gt 0 ] && [ $# -gt $n ] || exit 64
  cmd=
  j=1
  while [ $j -le $n ]; do
    cmd="$cmd \"\$$j\""
    j=$((j + 1))
  done
  eval "$cmd" </dev/null >"$dir/out" 2>"$dir/err"
  status=$?
  printf '%s %s %s %s\n' "$i" "$status" "$(($(wc -c <"$dir/out")))" "$(($(wc -c <"$dir/err")))"
  cat "$dir/out" "$dir/err"
  shift $((n + 1))
  i=$((i + 1))
done
