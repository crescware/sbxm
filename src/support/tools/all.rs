use super::{Claude, Codex, Gh, Mise, Tool};

/// 並びはこの1箇所だけが持つ。probeも、checkboxも、Dockerfileのmarkerもここから引く。
pub const ALL: [&dyn Tool; 4] = [&Gh, &Mise, &Claude, &Codex];
