use std::sync::mpsc::{self, Receiver};

use crate::config::ConfigLocation;
use crate::diagnostics::{Msg, Result};
use crate::metadata::last_worktree_index;

use super::{Candidate, ProjectPrompt, candidates, labels, no_managed_projects, unresolved};

/// 案件ごとのmetadata最大値をpromptの裏で読む。
///
/// prompt表示をmetadata読み込みで止めないため、読み込みはthreadへ渡す。カーソルが
/// 当たるのを待たず、promptを開いた時点で全案件を先頭から順に読む。案件が多くても、
/// カーソルが届く前に結果が揃っていく。
///
/// 計算結果はpromptの各描画前にpollされ、未着の案件は範囲を持たないまま扱う。
/// promptが閉じて受け手が消えたら、残りは読まずに終える。
struct MetadataMaximums {
    maximums: Vec<Option<u32>>,
    receiver: Receiver<(usize, Option<u32>)>,
}

impl MetadataMaximums {
    fn new(candidates: &[Candidate]) -> MetadataMaximums {
        let (sender, receiver) = mpsc::channel();
        // 全件を読むので、ここでのcloneは表示されない案件のぶんを先に作る無駄にならない。
        let queue = candidates.to_vec();
        let _ = std::thread::Builder::new()
            .name("sbxm-open-maximum".to_owned())
            .spawn(move || {
                for (project, candidate) in queue.into_iter().enumerate() {
                    let maximum = candidate.reload().ok().map(|metadata| {
                        last_worktree_index(metadata.provisioning.requested_worktrees)
                    });
                    // promptが閉じれば受け手はいない。残りは読まずに終える。
                    if sender.send((project, maximum)).is_err() {
                        return;
                    }
                }
            });

        MetadataMaximums {
            maximums: vec![None; candidates.len()],
            receiver,
        }
    }

    fn poll(&mut self, project: usize) -> Option<u32> {
        while let Ok((index, maximum)) = self.receiver.try_recv() {
            if let Some(slot) = self.maximums.get_mut(index) {
                *slot = maximum;
            }
        }

        self.maximums.get(project).copied().flatten()
    }
}

/// 案件とworktree indexを1画面で選ぶ。
///
/// promptにはregistryの表示情報だけを渡す。metadataの最大値を読むとprompt表示が遅れる
/// ため、indexは呼び出し側が渡す天井まで受け付け、計算結果を裏から反映する。
/// 確定後にもlock済みmetadataでclampし、下げた場合は接続前にその差を見せる。
pub fn open(
    location: &ConfigLocation,
    heading: &Msg,
    prompt: &mut dyn ProjectPrompt,
    ceiling: u32,
) -> Result<(Candidate, u32)> {
    let mut candidates = candidates(location)?;
    if candidates.is_empty() {
        return Err(no_managed_projects());
    }
    let labels = labels(&candidates);
    let mut metadata_maximums = MetadataMaximums::new(&candidates);
    let mut maximums = |project| metadata_maximums.poll(project);
    let (project, index) = prompt.select_open(heading, &labels, ceiling, &mut maximums)?;
    if project >= candidates.len() {
        return Err(unresolved(project, candidates.len()));
    }
    Ok((candidates.remove(project), index))
}

#[cfg(test)]
#[path = "open_test.rs"]
mod open_test;
