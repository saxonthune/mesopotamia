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

/// After a regenerate cycle (OnExit(Running) teardown + OnEnter(Generating) worldgen),
/// all elk are despawned, lifecycle resources reset, and the grid is rebuilt from
/// the new seed — leaving ElkParams untouched.
#[test]
fn regenerate_teardown_and_rebuild() {
    let mut app = make_app();

    // Run until at least one elk exists and the spawner has ticked.
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

    // Capture pre-regenerate grid water signature (static after worldgen).
    let water_before = total_water(app.world());

    // Inject a deterministic new seed and trigger regeneration.
    let new_seed: u64 = 0xDEAD_BEEF_CAFE_1234;
    app.world_mut().resource_mut::<WorldSeed>().0 = new_seed;
    app.world_mut().resource_mut::<NextState<Sim>>().set(Sim::Generating);

    // One update applies OnExit(Running) teardown and OnEnter(Generating) worldgen.
    // The state after this frame is Generating (Running transition queued but not yet applied).
    app.update();

    // Teardown must have despawned all elk.
    assert_eq!(elk_count(app.world_mut()), 0, "teardown must despawn all elk on OnExit(Running)");

    // Spawner must be reset to its default (elapsed = 0).
    assert_eq!(spawner_elapsed(app.world()), 0, "Spawner.elapsed must be 0 after teardown");

    // Herds must be cleared.
    {
        let herds = app.world().get_resource::<Herds>().unwrap();
        assert!(herds.cohorts.is_empty(), "Herds.cohorts must be empty after teardown");
        assert!(herds.order.is_empty(), "Herds.order must be empty after teardown");
    }

    // Grid must have been rebuilt from the new seed: water layout differs.
    let water_after = total_water(app.world());
    assert!(
        (water_after - water_before).abs() > 0.1,
        "grid water layout did not change after regenerate (before={water_before:.2}, after={water_after:.2})"
    );
}
