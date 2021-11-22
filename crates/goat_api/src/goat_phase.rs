use crate::PlayerIdx;

#[derive(Clone, Debug)]
pub struct GoatPhase {
    pub goat: PlayerIdx,
    pub noise: Option<usize>,
}

impl GoatPhase {
    pub fn new(goat: PlayerIdx) -> Self {
        Self { goat, noise: None }
    }
}
