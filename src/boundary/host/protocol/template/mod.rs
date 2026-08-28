//! `sbx template ls --json`の解釈。

mod parse_template_list;
mod template_entry;

pub use parse_template_list::parse_template_list;
pub use template_entry::TemplateEntry;

#[cfg(test)]
#[path = "template_test.rs"]
mod template_test;
