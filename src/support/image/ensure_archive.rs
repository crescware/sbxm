use std::fs;

use crate::archive;

use crate::boundary::host::HostEnvironment;
use crate::design::Fact;
use crate::design::{ProgressSink, Warning};
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

    let result = (|| {
        progress.step(msg!("progress-saving-archive"));
        docker::save(host, &image.name, &temporary, progress)?;
        archive::verify_holds_image(&temporary, &image.name, &image.labels)
            .map_err(|error| docker::diagnose_failure(host, error))?;
        let image_ids = archive::read_image_ids(&temporary)
            .map_err(|error| docker::diagnose_failure(host, error))?;
        paths::atomic_rename_into_place(&temporary, &target)?;
        TransientArchive::new(target, image_ids)
    })();
    match result {
        Ok(archive) => Ok(archive),
        Err(error) => {
            // producerが非ゼロで終わった場合も、大きな書きかけを次回まで残さない。
            // 開始前に既存の一時archiveを除去済みであり、この実行が作ったpathだけを扱う。
            let cleanup = remove_partial(&temporary);
            Err(with_cleanup(error, &temporary, cleanup, progress))
        }
    }
}

fn remove_partial(path: &std::path::Path) -> std::io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => fs::remove_file(path),
        Ok(_) => Err(std::io::Error::other(
            "the partial archive is no longer a regular file",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn with_cleanup(
    error: Error,
    path: &std::path::Path,
    cleanup: std::io::Result<()>,
    progress: &mut dyn ProgressSink,
) -> Error {
    let Err(cause) = cleanup else {
        return error;
    };
    match error {
        Error::Canceled => {
            progress.warn(
                Warning::text(msg!("warning-archive-cleanup-failed"))
                    .fact(Fact::path(&paths::display(path)))
                    .fact(Fact::cause(&cause.to_string())),
            );
            Error::Canceled
        }
        Error::Diagnostics(diagnostics) => Error::Diagnostics(
            diagnostics
                .into_iter()
                .map(|diagnostic| {
                    diagnostic
                        .fact(Fact::path(&paths::display(path)))
                        .fact(Fact::cause(&cause.to_string()))
                })
                .collect(),
        ),
    }
}

#[cfg(test)]
#[path = "ensure_archive_test.rs"]
mod ensure_archive_test;
