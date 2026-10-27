# sbxm inside check
dir=$(mktemp -d) || exit 70
trap 'rm -rf "$dir"' EXIT
trap 'rm -rf "$dir"; exit 143' HUP INT TERM
timed=
if [ "$1" = -t ]; then
  timed=1
  shift
fi
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
  elapsed=-
  [ -z "$timed" ] || t0=$(date +%s%N)
  eval "$cmd" </dev/null >"$dir/out" 2>"$dir/err"
  status=$?
  if [ -n "$timed" ]; then
    t1=$(date +%s%N)
    case "$t0$t1" in
      *[!0-9]*) ;;
      *) elapsed=$((t1 - t0)); [ "$elapsed" -ge 0 ] || elapsed=0 ;;
    esac
  fi
  printf '%s %s %s %s %s\n' "$i" "$status" "$(($(wc -c <"$dir/out")))" "$(($(wc -c <"$dir/err")))" "$elapsed"
  cat "$dir/out" "$dir/err"
  shift $((n + 1))
  i=$((i + 1))
done
