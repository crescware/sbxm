use crate::support::image;
use crate::testing::host::FakeSbx;
use crate::testing::project::Registered;

/// imageがまだ存在しないhost。一覧は答えるが、1件も返さない。
pub fn without_image(host: FakeSbx, project: &Registered) -> FakeSbx {
    let image = image::image_name(
        &project.sandbox,
        &project.metadata.provisioning.dockerfile_sha256,
    );
    host.answering(&format!("image ls --quiet {image}"), 0, "")
}
