#[cfg(test)]
#[path = "../../tests/unit/common/text_reader_test.rs"]
mod text_reader_test;

use std::collections::HashMap;
use std::io::prelude::*;
use std::io::{BufReader, Read};
use std::sync::Arc;
use vrp_core::construction::constraints::*;
use vrp_core::construction::heuristics::{create_end_activity, create_start_activity};
use vrp_core::models::common::*;
use vrp_core::models::problem::*;
use vrp_core::models::solution::{Activity, Registry, Route, Tour};
use vrp_core::models::{Problem, Solution};

pub trait TextReader {
    fn read_problem(&mut self) -> Result<Problem, String> {
        let fleet = self.read_fleet()?;
        let jobs = self.read_jobs()?;
        let transport = self.create_transport()?;
        let activity = Arc::new(SimpleActivityCost::default());
        let jobs = Jobs::new(&fleet, jobs, &transport);

        Ok(Problem {
            fleet: Arc::new(fleet),
            jobs: Arc::new(jobs),
            locks: vec![],
            constraint: Arc::new(create_constraint(activity.clone(), transport.clone())),
            activity,
            transport,
            objective: Arc::new(ObjectiveCost::default()),
            extras: Arc::new(Default::default()),
        })
    }

    fn read_fleet(&mut self) -> Result<Fleet, String>;

    fn read_jobs(&mut self) -> Result<Vec<Job>, String>;

    fn create_transport(&self) -> Result<Arc<dyn TransportCost + Send + Sync>, String>;
}

pub fn create_fleet_with_distance_costs(number: usize, capacity: usize, location: Location, time: TimeWindow) -> Fleet {
    Fleet::new(
        vec![Arc::new(Driver {
            costs: Costs {
                fixed: 0.0,
                per_distance: 0.0,
                per_driving_time: 0.0,
                per_waiting_time: 0.0,
                per_service_time: 0.0,
            },
            dimens: create_dimens_with_id("driver", 0),
            details: Default::default(),
        })],
        (0..number)
            .map(|i| {
                let mut dimens = create_dimens_with_id("v", i);
                dimens.set_capacity(capacity as i32);
                Arc::new(Vehicle {
                    profile: 0,
                    costs: Costs {
                        fixed: 0.0,
                        per_distance: 1.0,
                        per_driving_time: 0.0,
                        per_waiting_time: 0.0,
                        per_service_time: 0.0,
                    },
                    dimens,
                    details: vec![VehicleDetail {
                        start: Some(location),
                        end: Some(location),
                        time: Some(time.clone()),
                    }],
                })
            })
            .collect(),
        Box::new(|_| Box::new(|_| 0)),
    )
}

pub fn create_dimens_with_id(prefix: &str, id: usize) -> Dimensions {
    let mut dimens = Dimensions::new();
    dimens.set_id([prefix.to_string(), id.to_string()].concat().as_str());
    dimens
}

pub fn create_constraint(
    activity: Arc<SimpleActivityCost>,
    transport: Arc<dyn TransportCost + Send + Sync>,
) -> ConstraintPipeline {
    let mut constraint = ConstraintPipeline::default();
    constraint.add_module(Box::new(TransportConstraintModule::new(
        activity,
        transport,
        Arc::new(|_| (None, None)),
        1,
        2,
        3,
    )));
    constraint.add_module(Box::new(CapacityConstraintModule::<i32>::new(4)));
    constraint.add_module(Box::new(FleetUsageConstraintModule::new_minimized()));

    constraint
}

pub fn read_init_solution<R: Read>(mut reader: BufReader<R>, problem: Arc<Problem>) -> Result<Solution, String> {
    let mut buffer = String::new();

    let mut solution = Solution {
        registry: Registry::new(&problem.fleet),
        routes: vec![],
        unassigned: Default::default(),
        extras: problem.extras.clone(),
    };

    loop {
        match read_line(&mut reader, &mut buffer) {
            Ok(read) if read > 0 => {
                let route: Vec<_> = buffer.split(':').collect();
                assert_eq!(route.len(), 2);
                let id_map = problem.jobs.all().fold(HashMap::<String, Arc<Single>>::new(), |mut acc, job| {
                    let single = job.to_single().clone();
                    acc.insert(single.dimens.get_id().unwrap().to_string(), single);
                    acc
                });

                let actor = solution.registry.next().next().unwrap();
                let mut tour = Tour::default();
                tour.set_start(create_start_activity(&actor));
                create_end_activity(&actor).map(|end| tour.set_end(end));

                route.last().unwrap().split_whitespace().for_each(|id| {
                    let single = id_map.get(id).unwrap();
                    let place = single.places.first().unwrap();
                    tour.insert_last(Activity {
                        place: vrp_core::models::solution::Place {
                            location: place.location.unwrap(),
                            duration: place.duration,
                            time: place.times.first().and_then(|span| span.as_time_window()).unwrap(),
                        },
                        schedule: Schedule::new(0.0, 0.0),
                        job: Some(single.clone()),
                    });
                });

                solution.routes.push(Route { actor, tour });
            }
            Ok(_) => break,
            Err(error) => {
                if buffer.is_empty() {
                    break;
                } else {
                    return Err(error);
                }
            }
        }
    }

    Ok(solution)
}

pub fn read_line<R: Read>(reader: &mut BufReader<R>, mut buffer: &mut String) -> Result<usize, String> {
    buffer.clear();
    reader.read_line(&mut buffer).map_err(|err| err.to_string())
}
