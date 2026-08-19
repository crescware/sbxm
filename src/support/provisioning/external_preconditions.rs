/// custom secretとDocker Engineの、read-onlyな事前条件を確認済みという証跡。
///
/// `provision`はこれをconsumeすることでしか進めない。呼び出し側がこの証跡を1回の
/// 確認だけから得るため、`provision`が内部で同じ外部callを二重に発行することもない。
pub(crate) struct ExternalPreconditions(pub(super) ());
