use crate::testing::recorded_output::RecordedOutput;

use super::*;

const HIDDEN: [&str; 2] = ["To connect to this sandbox", "sbx run"];

fn shown(chunks: &[&[u8]]) -> String {
    let mut recorded = RecordedOutput::new();
    {
        let mut hiding = HidingLines::new(&mut recorded, &HIDDEN);
        for chunk in chunks {
            hiding.relay(chunk);
        }
        hiding.finished();
    }
    recorded.text()
}

#[test]
fn the_lines_that_name_another_way_in_are_left_out() {
    let shown = shown(&[
        b"Created sandbox 'example'\n",
        b"  Agent: shell\n\nTo connect to this sandbox, run:\n  sbx run --name example\n",
    ]);

    assert_eq!(shown, "Created sandbox 'example'\n  Agent: shell\n");
}

#[test]
fn a_hidden_line_that_arrives_one_byte_at_a_time_is_still_recognised() {
    // 中継はbyteが届いた順に流れる。行の切れ目がchunkの切れ目と一致するとは限らない。
    let line = b"To connect to this sandbox, run:\nkept\n";
    let chunks: Vec<&[u8]> = line.chunks(1).collect();

    assert_eq!(shown(&chunks), "kept\n");
}

#[test]
fn a_carriage_return_ends_a_line_so_progress_does_not_wait_for_a_newline() {
    // 同じ行を書き換える進捗表示は改行を出さない。復帰文字を行の終わりとして扱わないと、
    // 進捗は次の改行まで溜まり、動いていることが見えなくなる。
    assert_eq!(
        shown(&[b"Receiving objects:  50%\rReceiving objects: 100%\r"]),
        "Receiving objects:  50%\rReceiving objects: 100%\r",
        "each rewrite of the line arrives on its own"
    );

    // 行の切れ目が復帰文字であっても、見せない行の判断は行ごとに決まる。
    assert_eq!(shown(&[b"sbx run --name example\rkept\n"]), "kept\n");
}

#[test]
fn a_blank_line_left_behind_by_a_hidden_line_is_not_shown_either() {
    // 落とした行の跡に空行だけが残ると、sbxmの行との境界が二重になる。
    let shown = shown(&[b"kept\n", b"\n", b"sbx run --name example\n"]);

    assert_eq!(shown, "kept\n");
}

#[test]
fn a_blank_line_between_two_shown_lines_stays_where_the_tool_put_it() {
    let shown = shown(&[b"first\n\nsecond\n"]);

    assert_eq!(shown, "first\n\nsecond\n");
}

#[test]
fn a_command_with_nothing_to_hide_relays_every_line() {
    let mut recorded = RecordedOutput::new();
    {
        let mut hiding = HidingLines::new(&mut recorded, &[]);
        hiding.relay(b"To connect to this sandbox, run:\n");
        hiding.hand_over();
        hiding.finished();
    }

    assert_eq!(recorded.text(), "To connect to this sandbox, run:\n");
    assert_eq!(recorded.handed_over, 1, "the hand-over is passed along");
    assert_eq!(recorded.finished, 1, "the end is passed along once");
}
