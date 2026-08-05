/// sbxm以外のtoolが端末へ出す出力の受け口。
///
/// sbxmの行と外部toolの行の境界に空行を1つ置くのは、外部commandを起動する側ではなく
/// 描画側の仕事である。境界をどこに置くかを起動側が決めると、commandごとに規則が
/// 分かれ、どのcommandでも同じ見え方をするという保証がなくなる。
///
/// `relay`が1度も呼ばれなければ境界も置かない。何も出さなかった外部toolのために
/// 空行だけが残ることを避ける。
pub trait ExternalOutput {
    /// 外部toolが出したbyteを端末へ渡す。
    fn relay(&mut self, bytes: &[u8]);

    /// 中継せず端末そのものを渡すことを知らせる。
    ///
    /// 対話processは何を書くか観測できないため、境界の空行を先に置く。
    fn hand_over(&mut self);

    /// 外部toolの出力が終わったことを知らせる。
    fn finished(&mut self);
}
