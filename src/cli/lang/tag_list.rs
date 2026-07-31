use super::tags;

/// helpと診断へ並べる、受け付けるlocale tagの一覧。
pub(super) fn tag_list() -> String {
    tags().join(", ")
}
