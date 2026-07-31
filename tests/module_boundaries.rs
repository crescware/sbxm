//! `src`のmodule境界を検査する。
//!
//! ファイル名と公開する責務を一致させるため、Rust sourceをASTとして読み、top-levelで
//! 非privateなitemを1つに制限する。公開関数を置くファイルは、関数名とfile stemも一致
//! させる。`mod.rs`はmoduleの入口なので名前の一致は求めないが、実装itemを置かず、
//! moduleの組み立てとre-exportに限定するため、同じ1公開itemの規則を適用する。

use std::fs;
use std::path::{Path, PathBuf};

use syn::{File, Item, Visibility};

struct Source {
    relative: String,
    path: PathBuf,
    syntax: File,
}

#[derive(Debug)]
struct Export {
    name: String,
    expected_stem: String,
    reexport: bool,
}

#[test]
fn source_files_have_one_named_public_subject() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut paths = Vec::new();
    collect_sources(&root.join("src"), &mut paths)?;
    paths.sort();

    let mut violations = Vec::new();
    for path in paths {
        let relative = path
            .strip_prefix(root)
            .map_err(|error| format!("{} is outside the repository: {error}", path.display()))?
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        let text = fs::read_to_string(&path)?;
        let syntax = syn::parse_file(&text)?;
        let source = Source {
            relative,
            path,
            syntax,
        };
        check_source(&source, &mut violations);
    }

    assert!(
        violations.is_empty(),
        "source files violate the one-public-subject rule:\n{}",
        violations.join("\n")
    );
    Ok(())
}

fn collect_sources(
    directory: &Path,
    found: &mut Vec<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_sources(&path, found)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
    Ok(())
}

fn check_source(source: &Source, violations: &mut Vec<String>) {
    let exports = public_items(&source.syntax);
    let is_module = file_stem(&source.path) == "mod";
    if is_module {
        let implementations: Vec<String> = source
            .syntax
            .items
            .iter()
            .filter(|item| !matches!(item, Item::Mod(_) | Item::Use(_)))
            .map(item_name)
            .collect();
        if !implementations.is_empty() {
            violations.push(format!(
                "{}: mod.rs may contain only module declarations and use items; found {}",
                source.relative,
                implementations.join(", ")
            ));
        }
        return;
    }

    if exports.len() > 1 {
        let names = if exports.is_empty() {
            "no public subject".to_string()
        } else {
            exports
                .iter()
                .map(|export| export.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };
        violations.push(format!(
            "{}: expected one public subject, found {names}",
            source.relative
        ));
        return;
    }

    if exports.is_empty() {
        return;
    }

    let export = &exports[0];
    if export.reexport {
        violations.push(format!(
            "{}: public re-exports belong in mod.rs",
            source.relative
        ));
    } else if file_stem(&source.path) != export.expected_stem {
        violations.push(format!(
            "{}: `{}` belongs in {}.rs",
            source.relative, export.name, export.expected_stem
        ));
    }
}

fn public_items(file: &File) -> Vec<Export> {
    file.items
        .iter()
        .filter_map(|item| match item {
            Item::Const(item) if visible(&item.vis) => Some(Export {
                name: item.ident.to_string(),
                expected_stem: snake_case(&item.ident.to_string()),
                reexport: false,
            }),
            Item::Enum(item) if visible(&item.vis) => Some(Export {
                name: item.ident.to_string(),
                expected_stem: snake_case(&item.ident.to_string()),
                reexport: false,
            }),
            Item::ExternCrate(item) if visible(&item.vis) => Some(Export {
                name: item.ident.to_string(),
                expected_stem: snake_case(&item.ident.to_string()),
                reexport: false,
            }),
            Item::Fn(item) if visible(&item.vis) => Some(Export {
                name: item.sig.ident.to_string(),
                expected_stem: item.sig.ident.to_string(),
                reexport: false,
            }),
            Item::Mod(item) if visible(&item.vis) => Some(Export {
                name: item.ident.to_string(),
                expected_stem: snake_case(&item.ident.to_string()),
                reexport: false,
            }),
            Item::Static(item) if visible(&item.vis) => Some(Export {
                name: item.ident.to_string(),
                expected_stem: snake_case(&item.ident.to_string()),
                reexport: false,
            }),
            Item::Struct(item) if visible(&item.vis) => Some(Export {
                name: item.ident.to_string(),
                expected_stem: snake_case(&item.ident.to_string()),
                reexport: false,
            }),
            Item::Trait(item) if visible(&item.vis) => Some(Export {
                name: item.ident.to_string(),
                expected_stem: snake_case(&item.ident.to_string()),
                reexport: false,
            }),
            Item::TraitAlias(item) if visible(&item.vis) => Some(Export {
                name: item.ident.to_string(),
                expected_stem: snake_case(&item.ident.to_string()),
                reexport: false,
            }),
            Item::Type(item) if visible(&item.vis) => Some(Export {
                name: item.ident.to_string(),
                expected_stem: snake_case(&item.ident.to_string()),
                reexport: false,
            }),
            Item::Union(item) if visible(&item.vis) => Some(Export {
                name: item.ident.to_string(),
                expected_stem: snake_case(&item.ident.to_string()),
                reexport: false,
            }),
            Item::Use(item) if visible(&item.vis) => Some(Export {
                name: "pub use".to_string(),
                expected_stem: String::new(),
                reexport: true,
            }),
            _ => None,
        })
        .collect()
}

fn visible(visibility: &Visibility) -> bool {
    !matches!(visibility, Visibility::Inherited)
}

fn item_name(item: &Item) -> String {
    match item {
        Item::Const(item) => format!("const {}", item.ident),
        Item::Enum(item) => format!("enum {}", item.ident),
        Item::ExternCrate(item) => format!("extern crate {}", item.ident),
        Item::Fn(item) => format!("fn {}", item.sig.ident),
        Item::ForeignMod(_) => "extern block".to_string(),
        Item::Impl(_) => "impl block".to_string(),
        Item::Macro(_) => "macro".to_string(),
        Item::Mod(item) => format!("mod {}", item.ident),
        Item::Static(item) => format!("static {}", item.ident),
        Item::Struct(item) => format!("struct {}", item.ident),
        Item::Trait(item) => format!("trait {}", item.ident),
        Item::TraitAlias(item) => format!("trait alias {}", item.ident),
        Item::Type(item) => format!("type {}", item.ident),
        Item::Union(item) => format!("union {}", item.ident),
        Item::Use(_) => "use".to_string(),
        Item::Verbatim(_) => "verbatim item".to_string(),
        _ => "item".to_string(),
    }
}

fn file_stem(path: &Path) -> &str {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("")
}

fn snake_case(name: &str) -> String {
    let mut result = String::with_capacity(name.len());
    for (index, character) in name.chars().enumerate() {
        if character.is_ascii_uppercase() && index > 0 {
            result.push('_');
        }
        result.push(character.to_ascii_lowercase());
    }
    result
}
