use core::construction::constraints::*;
use core::construction::states::{RouteContext, SolutionContext};
use core::models::problem::Job;
use std::slice::Iter;
use std::sync::Arc;

pub struct ExtraCostModule {
    constraints: Vec<ConstraintVariant>,
    keys: Vec<i32>,
}

impl Default for ExtraCostModule {
    fn default() -> Self {
        Self {
            constraints: vec![ConstraintVariant::SoftRoute(Arc::new(ExtraCostSoftRouteConstraint {}))],
            keys: vec![],
        }
    }
}

impl ConstraintModule for ExtraCostModule {
    fn accept_route_state(&self, _ctx: &mut RouteContext) {}

    fn accept_solution_state(&self, _ctx: &mut SolutionContext) {}

    fn state_keys(&self) -> Iter<i32> {
        self.keys.iter()
    }

    fn get_constraints(&self) -> Iter<ConstraintVariant> {
        self.constraints.iter()
    }
}

struct ExtraCostSoftRouteConstraint {}

impl SoftRouteConstraint for ExtraCostSoftRouteConstraint {
    fn estimate_job(&self, ctx: &RouteContext, _job: &Arc<Job>) -> f64 {
        let route = ctx.route.read().unwrap();
        if route.tour.job_count() == 0 {
            route.actor.vehicle.costs.fixed
        } else {
            0.
        }
    }
}