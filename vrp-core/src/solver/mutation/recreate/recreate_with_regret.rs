use crate::construction::heuristics::*;
use crate::construction::heuristics::{InsertionContext, InsertionResult};
use crate::models::problem::Job;
use crate::solver::mutation::Recreate;
use crate::solver::RefinementContext;
use crate::utils::parallel_collect;
use std::cmp::Ordering::*;
use std::ops::Deref;

/// A recreate method which uses regret insertion approach.
pub struct RecreateWithRegret {
    job_selector: Box<dyn JobSelector + Send + Sync>,
    job_reducer: Box<dyn JobMapReducer + Send + Sync>,
}

impl Default for RecreateWithRegret {
    fn default() -> Self {
        RecreateWithRegret::new(2, 4)
    }
}

impl Recreate for RecreateWithRegret {
    fn run(&self, refinement_ctx: &mut RefinementContext, insertion_ctx: InsertionContext) -> InsertionContext {
        InsertionHeuristic::default().process(
            &self.job_selector,
            &self.job_reducer,
            insertion_ctx,
            &refinement_ctx.quota,
        )
    }
}

impl RecreateWithRegret {
    pub fn new(min: usize, max: usize) -> Self {
        Self {
            job_selector: Box::new(AllJobSelector::default()),
            job_reducer: Box::new(RegretJobMapReducer::new(min, max)),
        }
    }
}

struct RegretJobMapReducer {
    min: usize,
    max: usize,
}

impl RegretJobMapReducer {
    pub fn new(min: usize, max: usize) -> Self {
        Self { min, max }
    }
}

impl JobMapReducer for RegretJobMapReducer {
    fn reduce<'a>(
        &'a self,
        ctx: &'a InsertionContext,
        jobs: Vec<Job>,
        map: Box<dyn Fn(&Job) -> InsertionResult + Send + Sync + 'a>,
    ) -> InsertionResult {
        let mut results = parallel_collect(&jobs, |job| map.deref()(&job));

        results.sort_by(|a, b| match (a, b) {
            (InsertionResult::Success(a), InsertionResult::Success(b)) => a.cost.partial_cmp(&b.cost).unwrap_or(Less),
            (InsertionResult::Success(_), InsertionResult::Failure(_)) => Less,
            (InsertionResult::Failure(_), InsertionResult::Success(_)) => Greater,
            (InsertionResult::Failure(_), InsertionResult::Failure(_)) => Equal,
        });

        let regret_index =
            ctx.random.uniform_int(self.min as i32, self.max as i32).min(results.len() as i32) as usize - 1;

        let insertion_result = results
            .drain(regret_index..=regret_index)
            .next()
            .unwrap_or_else(|| panic!("Unexpected insertion results length"));

        insertion_result
    }
}
