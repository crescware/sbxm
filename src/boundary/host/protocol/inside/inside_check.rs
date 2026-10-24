/// Sandboxの中で、受け取ったcommandを順に走らせ、それぞれの結果を1つの出力に並べる手順。
///
/// 引数は、commandのargvを空の引数で区切って並べたものである（`inside_check_arguments`が
/// 作る）。argvに空の引数は現れないため、空の引数は区切りにしか読めない。各commandは
/// `eval`へ`"$1" "$2" ...`の形で渡し、shellが引数を分け直すことはない。stdinは繋がない。
///
/// 1つのcommandの結果は、`<番号> <終了status> <stdoutのbyte数> <stderrのbyte数> <所要時間>`
/// の見出し1行と、そのbyte数ぶんのstdout、stderrである。stdoutもstderrもfileへ受けてから
/// 長さを数えて流すため、末尾の改行もNULも、command自身が書いたとおりに届く。
/// 見出しの番号は0から数え、受け取った順と一致する。
///
/// 所要時間は、最初の引数が`-t`のときだけ計り、nano秒の整数で示す。計らないとき、または
/// shellの`date`がnano秒を出せないときは`-`である。計るのは`--verbose`の実行だけであり、
/// 時刻を読む2回の起動をcommandごとに増やす代償は、求めた実行だけが払う。commandの
/// argvはprogram名から始まるため、先頭の`-t`がcommandと紛れることはない。
///
/// 終了statusは、commandごとの見出しが持つ。手順そのものは、引数の形が崩れていれば
/// `64`、一時directoryを作れなければ`70`で終わり、どちらも何も並べない。
///
/// 本文は`inside_check.sh`が持つ。`tests/inside_check.rs`が同じfileをこのhostのshellで
/// 走らせ、並べ方が`parse_inside_check`の読み方と一致することを確かめる。
pub const INSIDE_CHECK: &str = include_str!("inside_check.sh");
