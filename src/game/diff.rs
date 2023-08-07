use super::GameExt;

pub trait GetDiffExt {
    type Diff;
    type Error;

    /// Get component version diff
    fn get_diff(&self) -> Result<Self::Diff, Self::Error>;
}

pub trait DiffExt {
    /// Install diff to the game
    fn install(&self, game: &impl GameExt);
}
