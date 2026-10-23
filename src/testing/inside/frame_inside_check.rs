use std::fmt::Write as _;

/// commandごとの答えを、`INSIDE_CHECK`が並べるのと同じ形にする。
///
/// fakeの答えはstdoutだけを持つ。stderrは空として並べる。
pub fn frame_inside_check(answers: &[(i32, String)]) -> String {
    let mut framed = String::new();
    for (index, (code, stdout)) in answers.iter().enumerate() {
        // Stringへの書き込みは失敗しない。
        let _ = write!(framed, "{index} {code} {} 0\n{stdout}", stdout.len());
    }
    framed
}
