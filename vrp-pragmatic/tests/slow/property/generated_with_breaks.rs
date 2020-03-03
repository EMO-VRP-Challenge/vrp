use crate::checker::solve_and_check;
use crate::generator::*;
use crate::json::problem::*;

use crate::json::Location;
use proptest::prelude::*;
use std::time::Duration;

fn get_test_locations() -> impl Strategy<Value = Location> {
    generate_simple_locations(1..30000)
}

fn get_breaks() -> impl Strategy<Value = Option<Vec<VehicleBreak>>> {
    prop::collection::vec(generate_break(get_break_locations(), generate_durations(10..100), get_break_times()), 1..4)
        .prop_map(|reloads| Some(reloads))
}

fn get_break_locations() -> impl Strategy<Value = Option<Vec<Location>>> {
    prop_oneof![
        Just(None),
        Just(Some(vec![default_vehicle_location()])),
        prop::collection::vec(get_test_locations(), 1..5).prop_map(|locations| Some(locations))
    ]
}

fn get_break_times() -> impl Strategy<Value = VehicleBreakTime> {
    prop_oneof![get_break_offset_time(), get_break_time_windows()]
}

prop_compose! {
    fn get_break_offset_time()
        (start in 100..500,
         length in 10..200) -> VehicleBreakTime {
        VehicleBreakTime::TimeOffset(vec![start as f64, (start + length) as f64])
    }
}

pub fn get_break_time_windows() -> impl Strategy<Value = VehicleBreakTime> {
    generate_multiple_time_windows_fixed(
        START_DAY,
        vec![Duration::from_secs(100), Duration::from_secs(400)],
        vec![Duration::from_secs(20), Duration::from_secs(80)],
        1..2,
    )
    .prop_map(|tws| VehicleBreakTime::TimeWindows(tws))
}

prop_compose! {
    fn get_vehicle_type_with_breaks()
        (vehicle in default_vehicle_type_prototype(),
         breaks in get_breaks()
        ) -> VehicleType {
        assert_eq!(vehicle.shifts.len(), 1);

        let mut vehicle = vehicle;
        vehicle.shifts.first_mut().unwrap().breaks = breaks;

        vehicle
    }
}

prop_compose! {
    fn get_problem_with_breaks()
        (plan in generate_plan(generate_jobs(default_job_prototype(), 1..256)),
         fleet in generate_fleet(generate_vehicles(get_vehicle_type_with_breaks(), 1..4), default_profiles())
        )
        -> Problem {
        Problem {
            plan,
            fleet,
            config: None
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    #[ignore]
    fn can_solve_problem_with_breaks(problem in get_problem_with_breaks()) {
        let result = solve_and_check(problem);

        assert_eq!(result, Ok(()));
    }
}
