use std::sync::mpsc::{self, Receiver, Sender};

use crate::config::ConfigLocation;
use crate::diagnostics::{Msg, Result};

use super::{Candidate, ProjectPrompt, candidates, labels, no_managed_projects, unresolved};

/// 案件ごとのmetadata最大値をpromptの裏で読む。
///
/// prompt表示をmetadata読み込みで止めないため、案件が初めて表示対象になった時点で
/// threadへ処理を渡す。計算結果はpromptの各描画前にpollされ、未完了の案件は引き続き
/// 設定上限を使う。threadはprompt終了後も現在の読み込みだけを完了して自然に終了する。
struct MetadataMaximums {
    candidates: Vec<Candidate>,
    started: Vec<bool>,
    maximums: Vec<Option<u32>>,
    sender: Sender<(usize, Option<u32>)>,
    receiver: Receiver<(usize, Option<u32>)>,
}

impl MetadataMaximums {
    fn new(candidates: &[Candidate]) -> MetadataMaximums {
        let (sender, receiver) = mpsc::channel();
        MetadataMaximums {
            candidates: candidates.to_vec(),
            started: vec![false; candidates.len()],
            maximums: vec![None; candidates.len()],
            sender,
            receiver,
        }
    }

    fn poll(&mut self, project: usize) -> Option<u32> {
        while let Ok((index, maximum)) = self.receiver.try_recv() {
            if let Some(slot) = self.maximums.get_mut(index) {
                *slot = maximum;
            }
        }

        if let Some(started) = self.started.get_mut(project)
            && !*started
        {
            *started = true;
            let candidate = self.candidates[project].clone();
            let sender = self.sender.clone();
            let _ = std::thread::Builder::new()
                .name("sbxm-open-maximum".to_owned())
                .spawn(move || {
                    let maximum = candidate.reload().ok().map(|metadata| {
                        metadata.provisioning.requested_worktrees.saturating_sub(1)
                    });
                    let _ = sender.send((project, maximum));
                });
        }

        self.maximums.get(project).copied().flatten()
    }
}

/// 案件とworktree indexを1画面で選ぶ。
///
/// promptにはregistryの表示情報だけを渡す。metadataの最大値を読むとprompt表示が遅れる
/// ため、indexは呼び出し側が渡す楽観的な上限で受け付け、計算結果を裏から反映する。
/// 確定後にもlock済みmetadataでclampする。
pub fn open(
    location: &ConfigLocation,
    heading: &Msg,
    prompt: &mut dyn ProjectPrompt,
    maximum_index: u32,
) -> Result<(Candidate, u32)> {
    let mut candidates = candidates(location)?;
    if candidates.is_empty() {
        return Err(no_managed_projects());
    }
    let labels = labels(&candidates);
    let mut metadata_maximums = MetadataMaximums::new(&candidates);
    let mut maximums = |project| metadata_maximums.poll(project);
    let (project, index) =
        prompt.select_open_with_maximums(heading, &labels, maximum_index, &mut maximums)?;
    if project >= candidates.len() {
        return Err(unresolved(project, candidates.len()));
    }
    Ok((candidates.remove(project), index))
}

#[cfg(test)]
#[path = "open_test.rs"]
mod open_test;
