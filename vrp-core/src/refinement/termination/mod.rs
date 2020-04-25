//! Metaheuristic termination logic.

use crate::refinement::RefinementContext;

/// A trait which specifies criteria when metaheuristic should stop searching for improved solution.
pub trait Termination {
    /// Returns true if termination condition is met.
    fn is_termination(&self, refinement_ctx: &mut RefinementContext) -> bool;
}

mod max_generation;
pub use self::max_generation::MaxGeneration;

mod max_time;
pub use self::max_time::MaxTime;

/// A trait which encapsulates multiple termination criteria.
pub struct CompositeTermination {
    terminations: Vec<Box<dyn Termination>>,
}

impl CompositeTermination {
    /// Creates a new instance of [`CompositeTermination`].
    pub fn new(terminations: Vec<Box<dyn Termination>>) -> Self {
        Self { terminations }
    }
}

impl Termination for CompositeTermination {
    fn is_termination(&self, refinement_ctx: &mut RefinementContext) -> bool {
        self.terminations.iter().any(|t| t.is_termination(refinement_ctx))
    }
}
