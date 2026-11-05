# sbxm inside check
memory=
if [ "$1" = -m ]; then
  memory=1
  shift
  export LC_ALL=C
else
  dir=$(mktemp -d) || exit 70
  trap 'rm -rf "$dir"' EXIT
  trap 'rm -rf "$dir"; exit 143' HUP INT TERM
fi
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
  if [ -n "$memory" ]; then
    # textだけの検査はpipeとshellのmemoryで受ける。末尾の改行も失わない。
    text=$(eval "$cmd" </dev/null 2>&1; status=$?; printf .; exit "$status")
    status=$?
    text=${text%.}
  else
    eval "$cmd" </dev/null >"$dir/out" 2>"$dir/err"
    status=$?
  fi
  if [ -n "$timed" ]; then
    t1=$(date +%s%N)
    case "$t0$t1" in
      *[!0-9]*) ;;
      *) elapsed=$((t1 - t0)); [ "$elapsed" -ge 0 ] || elapsed=0 ;;
    esac
  fi
  if [ -n "$memory" ]; then
    if [ "$status" -eq 0 ]; then
      printf '%s %s %s 0 %s\n' "$i" "$status" "${#text}" "$elapsed"
    else
      printf '%s %s 0 %s %s\n' "$i" "$status" "${#text}" "$elapsed"
    fi
    printf '%s' "$text"
  else
    printf '%s %s %s %s %s\n' "$i" "$status" "$(($(wc -c <"$dir/out")))" "$(($(wc -c <"$dir/err")))" "$elapsed"
    cat "$dir/out" "$dir/err"
  fi
  shift $((n + 1))
  i=$((i + 1))
done
