/// top-levelの`prefix`で始まる行だけを差し替える。無ければ`version`行の直後へ足す。
pub(super) fn replace_line(text: &str, prefix: &str, line: &str) -> String {
    let declares_key = text.lines().any(|source| source.starts_with(prefix));
    let mut out = String::with_capacity(text.len() + line.len() + 1);
    let mut written = false;
    for source in text.lines() {
        if declares_key {
            if !written && source.starts_with(prefix) {
                out.push_str(line);
                out.push('\n');
                written = true;
                continue;
            }
            out.push_str(source);
            out.push('\n');
            continue;
        }
        out.push_str(source);
        out.push('\n');
        if !written && source.starts_with("version:") {
            out.push_str(line);
            out.push('\n');
            written = true;
        }
    }
    if !written {
        out.push_str(line);
        out.push('\n');
    }
    out
}
