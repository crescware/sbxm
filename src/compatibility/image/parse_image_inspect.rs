use crate::diagnostics::{Result, unparseable};
use crate::image_labels::{LabelDefect, labels_from_declared};

use crate::compatibility::json::string_field;

use super::ImageIdentity;

/// `docker image inspect <image>`のstructured outputをparseする。
///
/// 1件のimageを指すため、要素が1個の配列だけを受け付ける。labelを持たないimageは
/// 空のlabel集合として扱い、labelの不足は呼び出し側が判定する。
pub fn parse_image_inspect(output: &str) -> Result<ImageIdentity> {
    let document: serde_json::Value = serde_json::from_str(output)
        .map_err(|error| unparseable("docker image inspect", &error.to_string()))?;
    let items = document
        .as_array()
        .ok_or_else(|| unparseable("docker image inspect", "the document is not an array"))?;
    let [item] = items.as_slice() else {
        return Err(unparseable(
            "docker image inspect",
            &format!(
                "the document describes {} images instead of one",
                items.len()
            ),
        ));
    };
    let object = item
        .as_object()
        .ok_or_else(|| unparseable("docker image inspect", "the entry is not an object"))?;

    let id = string_field(object, "Id")
        .filter(|id| !id.is_empty())
        .ok_or_else(|| unparseable("docker image inspect", "the image has no Id"))?;

    let Some(config) = object.get("Config").and_then(|config| config.as_object()) else {
        return Err(unparseable(
            "docker image inspect",
            "the image has no Config section",
        ));
    };
    let labels = labels_from_declared(config.get("Labels")).map_err(|defect| match defect {
        LabelDefect::NotAnObject => unparseable(
            "docker image inspect",
            "Labels is neither an object nor null",
        ),
        LabelDefect::ValueNotAString(key) => unparseable(
            "docker image inspect",
            &format!("label {key} does not hold a string"),
        ),
    })?;

    Ok(ImageIdentity { id, labels })
}
