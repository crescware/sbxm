/// hostのrepositoryへsbxmが保存した先端を示す表記。
///
/// originのref名と取り違えないよう`host:`を前に付ける。保存した先端をoriginの観測へ
/// 足すどの経路も、同じ表記を使う。
pub(super) fn host_label(reference: &str) -> String {
    format!("host:{reference}")
}
