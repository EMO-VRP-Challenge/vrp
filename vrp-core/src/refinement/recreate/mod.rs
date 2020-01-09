//! Contains logic to build a feasible solution from partially ruined one.

use crate::construction::states::InsertionContext;

/// A trait which specifies logic to produce a new feasible solution from partial one.
pub trait Recreate {
    /// Recreates a new solution from the given.
    fn run(&self, refinement_ctx: &RefinementContext, insertion_ctx: InsertionContext) -> InsertionContext;
}

mod recreate_with_cheapest;

pub use self::recreate_with_cheapest::RecreateWithCheapest;

mod recreate_with_gaps;

pub use self::recreate_with_gaps::RecreateWithGaps;
use crate::refinement::recreate::recreate_with_blinks::RecreateWithBlinks;
use crate::refinement::RefinementContext;

mod recreate_with_blinks;

/// Provides the way to run one of multiple recreate methods.
pub struct CompositeRecreate {
    recreates: Vec<Box<dyn Recreate>>,
    weights: Vec<usize>,
}

impl Default for CompositeRecreate {
    fn default() -> Self {
        Self::new(vec![
            (Box::new(RecreateWithCheapest::default()), 100),
            (Box::new(RecreateWithBlinks::<i32>::default()), 50),
            (Box::new(RecreateWithGaps::default()), 10),
        ])
    }
}

impl CompositeRecreate {
    pub fn new(recreates: Vec<(Box<dyn Recreate>, usize)>) -> Self {
        let mut recreates = recreates;
        recreates.sort_by(|(_, a), (_, b)| b.cmp(&a));

        let weights = recreates.iter().map(|(_, weight)| *weight).collect();
        Self { recreates: recreates.into_iter().map(|(recreate, _)| recreate).collect(), weights }
    }
}

impl Recreate for CompositeRecreate {
    fn run(&self, refinement_ctx: &RefinementContext, insertion_ctx: InsertionContext) -> InsertionContext {
        // NOTE always use recreate method with the larger weight for the initial generation
        let index = if refinement_ctx.generation == 1 { 0 } else { insertion_ctx.random.weighted(self.weights.iter()) };
        self.recreates.get(index).unwrap().run(refinement_ctx, insertion_ctx)
    }
}
