use std::fs;

use crate::archive;

use crate::command::HostEnvironment;
use crate::design::Fact;
use crate::design::ProgressSink;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::hash::short_hex;
use crate::msg;
use crate::paths::{self, PathScope, ProjectPaths};
use crate::support::docker;

use super::{BuiltImage, TransientArchive};

/// `sbx template load`にだけ使う、短命なTemplate archiveを作る。
///
/// archive工程へ到達するたびに新しく生成する。既存archiveの再利用による性能最適化は
/// MVPの対象外であり、いま検証したimageと同じものであることを毎回確かめる。
/// 正式なarchiveは、一時archiveの生成と検証が終わるまで変更しない。呼び出し側は
/// 返した[`TransientArchive`]を`template::ensure`にだけ使い、load後に片付ける。
pub fn ensure_archive(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    image: &BuiltImage,
    dockerfile_sha256: &str,
    progress: &mut dyn ProgressSink,
) -> Result<TransientArchive> {
    let generation = short_hex(dockerfile_sha256);
    let temporary = paths.template_archive_temp(generation);
    let target = paths.template_archive(generation);

    // `.cache`は登録時に作るが、以後に手動で消されていても`docker image save`の
    // `--output`は親directoryを自動で作らない。書き込みの直前に自己修復する。
    // 既存directoryのpermissionまでは問わない。ここで問うのは存在の有無だけである。
    paths::ensure_directory(&paths.cache_dir())?;

    // project lockを保持しているため、残っている一時fileは中断した実行の跡である。
    if paths::regular_file_exists(&temporary, PathScope::ProjectPath)? {
        fs::remove_file(&temporary).map_err(|error| {
            Error::single(
                Diagnostic::new(
                    ErrorId::AtomicWriteFailed,
                    msg!("error-atomic-write-failed"),
                )
                .fact(Fact::path(&paths::display(&temporary)))
                .fact(Fact::cause(&error.to_string())),
            )
        })?;
    }

    progress.step(msg!("progress-saving-archive"));
    docker::save(host, &image.name, &temporary, progress)?;

    archive::verify_holds_image(&temporary, &image.name, &image.labels)
        .map_err(|error| docker::diagnose_failure(host, error))?;
    paths::atomic_rename_into_place(&temporary, &target)?;
    Ok(TransientArchive::new(target))
}
