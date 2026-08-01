use std::fs;
use std::path::PathBuf;

use crate::archive;

use crate::command::{CommandSpec, HostEnvironment, TimeoutClass};
use crate::design::Fact;
use crate::design::ProgressSink;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::hash::short_hex;
use crate::msg;
use crate::paths::{self, PathScope, ProjectPaths};

use super::BuiltImage;

/// 世代別のTemplate archiveを作り直す。
///
/// archive工程へ到達するたびに新しく生成する。既存archiveの再利用による性能最適化は
/// MVPの対象外であり、いま検証したimageと同じものであることを毎回確かめる。
/// 正式なarchiveは、一時archiveの生成と検証が終わるまで変更しない。
pub fn ensure_archive(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    image: &BuiltImage,
    dockerfile_sha256: &str,
    progress: &mut dyn ProgressSink,
) -> Result<PathBuf> {
    let generation = short_hex(dockerfile_sha256);
    let temporary = paths.template_archive_temp(generation);
    let target = paths.template_archive(generation);

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
    let spec = CommandSpec::passthrough(
        "docker",
        &[
            "image",
            "save",
            &image.name,
            "--output",
            &paths::display(&temporary),
        ],
    )
    .timeout(TimeoutClass::ImageBuild);
    host.run(&spec)?.require_success()?;

    archive::verify_holds_image(&temporary, &image.name, &image.labels)?;
    paths::atomic_rename_into_place(&temporary, &target)?;
    Ok(target)
}
