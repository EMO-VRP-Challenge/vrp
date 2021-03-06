#[cfg(test)]
#[path = "../../tests/unit/constraints/breaks_test.rs"]
mod breaks_test;

use crate::constraints::*;
use std::collections::HashSet;
use std::slice::Iter;
use std::sync::Arc;
use vrp_core::construction::constraints::*;
use vrp_core::construction::heuristics::{ActivityContext, RouteContext, SolutionContext};
use vrp_core::models::common::{Schedule, TimeWindow, ValueDimension};
use vrp_core::models::problem::{Job, Single};
use vrp_core::models::solution::Activity;

/// Implements break functionality with variable location and time.
/// NOTE known issue: rescheduling departure might affect break with time offset.
pub struct BreakModule {
    conditional: ConditionalJobModule,
    constraints: Vec<ConstraintVariant>,
}

impl BreakModule {
    pub fn new(code: i32) -> Self {
        Self {
            conditional: ConditionalJobModule::new(create_job_transition()),
            constraints: vec![
                ConstraintVariant::HardRoute(Arc::new(BreakHardRouteConstraint { code })),
                ConstraintVariant::HardActivity(Arc::new(BreakHardActivityConstraint { code })),
                ConstraintVariant::SoftRoute(Arc::new(BreakSoftRouteConstraint {})),
            ],
        }
    }
}

impl ConstraintModule for BreakModule {
    fn accept_insertion(&self, solution_ctx: &mut SolutionContext, route_index: usize, _job: &Job) {
        let route_ctx = solution_ctx.routes.get_mut(route_index).unwrap();
        self.accept_route_state(route_ctx);
        self.accept_solution_state(solution_ctx);
    }

    fn accept_route_state(&self, ctx: &mut RouteContext) {
        self.conditional.accept_route_state(ctx);
    }

    fn accept_solution_state(&self, ctx: &mut SolutionContext) {
        self.conditional.accept_solution_state(ctx);

        if ctx.required.is_empty() {
            remove_invalid_breaks(ctx);
        }
    }

    fn state_keys(&self) -> Iter<i32> {
        self.conditional.state_keys()
    }

    fn get_constraints(&self) -> Iter<ConstraintVariant> {
        self.constraints.iter()
    }
}

struct BreakHardActivityConstraint {
    code: i32,
}

/// Locks break jobs to specific vehicles.
struct BreakHardRouteConstraint {
    code: i32,
}

impl HardRouteConstraint for BreakHardRouteConstraint {
    fn evaluate_job(&self, _: &SolutionContext, ctx: &RouteContext, job: &Job) -> Option<RouteConstraintViolation> {
        if let Some(single) = job.as_single() {
            if is_break_job(single) {
                let job = job.to_single();
                let vehicle_id = get_vehicle_id_from_job(&job).unwrap();
                let shift_index = get_shift_index(&job.dimens);

                return if !is_correct_vehicle(&ctx.route, vehicle_id, shift_index) {
                    Some(RouteConstraintViolation { code: self.code })
                } else {
                    None
                };
            }
        }

        None
    }
}

impl BreakHardActivityConstraint {
    fn stop(&self) -> Option<ActivityConstraintViolation> {
        Some(ActivityConstraintViolation { code: self.code, stopped: false })
    }
}

impl HardActivityConstraint for BreakHardActivityConstraint {
    fn evaluate_activity(
        &self,
        _: &RouteContext,
        activity_ctx: &ActivityContext,
    ) -> Option<ActivityConstraintViolation> {
        match as_break_job(&activity_ctx.target) {
            Some(_) if activity_ctx.prev.job.is_none() => self.stop(),
            _ => None,
        }
    }
}

/// Controls whether break is more preferable for insertion or not.
struct BreakSoftRouteConstraint {}

impl SoftRouteConstraint for BreakSoftRouteConstraint {
    fn estimate_job(&self, solution_ctx: &SolutionContext, _: &RouteContext, job: &Job) -> f64 {
        if let Some(single) = job.as_single() {
            if is_break_job(single) {
                -solution_ctx.get_max_cost()
            } else {
                0.
            }
        } else {
            0.
        }
    }
}

/// Promotes break jobs from required and ignored.
fn create_job_transition() -> Box<dyn JobContextTransition + Send + Sync> {
    Box::new(ConcreteJobContextTransition {
        remove_required: |ctx, job| !is_required_job(ctx, job, true),
        promote_required: |ctx, job| is_required_job(ctx, job, false),
        remove_locked: |_, _| false,
        promote_locked: |_, _| false,
    })
}

/// Mark job as ignored only if it has break type and vehicle id is not present in routes
fn is_required_job(ctx: &SolutionContext, job: &Job, default: bool) -> bool {
    match job {
        Job::Single(job) => {
            if is_break_job(job) {
                let vehicle_id = get_vehicle_id_from_job(job).unwrap();
                let shift_index = get_shift_index(&job.dimens);
                ctx.routes
                    .iter()
                    .any(move |rc| is_correct_vehicle(&rc.route, &vehicle_id, shift_index) && is_time(rc, job))
            } else {
                default
            }
        }
        Job::Multi(_) => default,
    }
}

/// Removes breaks which conditions are violated after ruin:
/// * break without location served separately when original job is removed, but break is kept.
/// * break is defined by interval, but its time is violated. This might happen due to departure time rescheduling.
fn remove_invalid_breaks(ctx: &mut SolutionContext) {
    let breaks_set = ctx.routes.iter_mut().fold(HashSet::new(), |mut acc, rc: &mut RouteContext| {
        // NOTE assume that first activity is never break (should be always departure)
        let (_, breaks_set) =
            rc.route.tour.all_activities().fold((0, HashSet::new()), |(prev, mut breaks), activity| {
                let current = activity.place.location;

                if let Some(break_job) = as_break_job(activity) {
                    // NOTE break should have location defined for all places or for none of them
                    let location_count = break_job.places.iter().filter(|p| p.location.is_some()).count();
                    assert!(location_count == 0 || location_count == break_job.places.len());

                    let is_orphan = prev != current && break_job.places.first().and_then(|p| p.location).is_none();
                    let is_not_on_time = !is_on_proper_time(rc, break_job, &activity.schedule);

                    if is_orphan || is_not_on_time {
                        // NOTE remove break with removed job location
                        breaks.insert(Job::Single(activity.job.as_ref().unwrap().clone()));
                    }
                }

                (current, breaks)
            });

        breaks_set.iter().for_each(|break_job| {
            rc.route_mut().tour.remove(break_job);
        });

        acc.extend(breaks_set.into_iter());

        acc
    });

    ctx.unassigned.extend(breaks_set.into_iter().map(|b| (b, 1)));
}

//region Helpers

fn is_break_job(job: &Arc<Single>) -> bool {
    job.dimens.get_value::<String>("type").map_or(false, |t| t == "break")
}

fn as_break_job(activity: &Activity) -> Option<&Arc<Single>> {
    as_single_job(activity, |job| is_break_job(job))
}

fn get_break_time_windows<'a>(break_job: &'a Arc<Single>, departure: f64) -> impl Iterator<Item = TimeWindow> + 'a {
    break_job.places.first().unwrap().times.iter().map(move |span| span.to_time_window(departure))
}

fn is_time(rc: &RouteContext, break_job: &Arc<Single>) -> bool {
    let departure = rc.route.tour.start().unwrap().schedule.departure;
    let arrival = rc.route.tour.end().map_or(0., |end| end.schedule.arrival);
    let actual_shift_time = TimeWindow::new(departure, arrival);

    get_break_time_windows(break_job, departure).any(|tw| tw.intersects(&actual_shift_time))
}

fn is_on_proper_time(rc: &RouteContext, break_job: &Arc<Single>, actual_schedule: &Schedule) -> bool {
    let departure = rc.route.tour.start().unwrap().schedule.departure;
    let actual_tw = TimeWindow::new(actual_schedule.arrival, actual_schedule.departure);

    get_break_time_windows(break_job, departure).any(|tw| tw.intersects(&actual_tw))
}

//endregion
