use crate::commands::repair::RepairOutput;
use crate::design::Document;
use crate::hash::short_hex;
use crate::msg;

/// `repair`の完了、またはmutation不要の結果を表示する。
pub fn document(output: &RepairOutput) -> Document {
    let message = if output.changed {
        msg!(
            "repair-done",
            project = output.project,
            sandbox = output.sandbox,
            generation = short_hex(&output.target_generation)
        )
    } else {
        msg!(
            "repair-no-change",
            project = output.project,
            sandbox = output.sandbox,
            generation = short_hex(&output.target_generation)
        )
    };
    Document::new().summary(message)
}
