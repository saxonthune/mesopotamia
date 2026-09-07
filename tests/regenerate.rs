use bevy::prelude::*;
use mesopotamia::elk::Herds;
use mesopotamia::grid::Grid;
use mesopotamia::sim::Sim;
use mesopotamia::sim_harness::{elk_count, make_app, spawner_elapsed};
use mesopotamia::worldgen::WorldSeed;

fn total_water(world: &World) -> f32 {
    let grid = world.get_resource::<Grid>().unwrap();
    (0..grid.len()).map(|i| grid.water(i)).sum()
}

// OnExit(Running) teardown + OnEnter(Generating) worldgen; ElkParams survive
#[test]
fn regenerate_teardown_and_rebuild() {
    let mut app = make_app();

    let mut elk_appeared = false;
    for _ in 0..2000 {
        app.update();
        if elk_count(app.world_mut()) > 0 {
            elk_appeared = true;
            break;
        }
    }
    assert!(elk_appeared, "no elk appeared within 2000 ticks; can't test regenerate teardown");
    assert!(spawner_elapsed(app.world()) > 0, "Spawner never ticked before regenerate test");

    let water_before = total_water(app.world());

    let new_seed: u64 = 0xDEAD_BEEF_CAFE_1234;
    app.world_mut().resource_mut::<WorldSeed>().0 = new_seed;
    app.world_mut().resource_mut::<NextState<Sim>>().set(Sim::Generating);

    // state after this frame is Generating (Running→Generating transition not yet applied)
    app.update();

    assert_eq!(elk_count(app.world_mut()), 0, "teardown must despawn all elk on OnExit(Running)");
    assert_eq!(spawner_elapsed(app.world()), 0, "Spawner.elapsed must be 0 after teardown");

    {
        let herds = app.world().get_resource::<Herds>().unwrap();
        assert!(herds.cohorts.is_empty(), "Herds.cohorts must be empty after teardown");
        assert!(herds.order.is_empty(), "Herds.order must be empty after teardown");
    }

    let water_after = total_water(app.world());
    assert!(
        (water_after - water_before).abs() > 0.1,
        "grid water layout did not change after regenerate (before={water_before:.2}, after={water_after:.2})"
    );
}
