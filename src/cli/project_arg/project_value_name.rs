/// 案件を指す位置引数のvalue name。
///
/// 値の形ではなく、値の名前を示す。形は`add`が表示するIDそのものであり、helpの
/// 説明文が併記する。登録の入力はclone URLであるため、ここへ`owner/repository`と
/// だけ書くと、その値がどこから来るのかがhelpから読めない。
///
/// clapはrequiredなvalueを`<>`、optionalなvalueを`[]`で囲む。どちらの表示でも読めるよう、
/// value name自体には囲み記号を含めない。
pub const PROJECT_VALUE_NAME: &str = "project-id";
