use bevy::prelude::*;

/// Where something the simulation moves stood after the last two ticks. Frames between ticks draw it part
/// way; before each tick its exact simulated transform comes back, so drawing never changes the battle.
#[derive(Component, Default)]
pub struct TickTransforms {
    previous: Option<Transform>,
    current: Option<Transform>,
}

pub fn restore_simulated_transforms(mut moving: Query<(&mut Transform, &mut TickTransforms)>) {
    for (mut transform, mut ticks) in &mut moving {
        if let Some(current) = ticks.current {
            *transform = current;
        }
        ticks.previous = ticks.current;
    }
}

pub fn record_simulated_transforms(mut moving: Query<(&Transform, &mut TickTransforms)>) {
    for (transform, mut ticks) in &mut moving {
        ticks.current = Some(*transform);
    }
}

pub fn draw_between_ticks(
    time: Res<Time<Fixed>>,
    mut moving: Query<(&mut Transform, &TickTransforms)>,
) {
    let progress = time.overstep_fraction();
    for (mut transform, ticks) in &mut moving {
        let (Some(previous), Some(current)) = (ticks.previous, ticks.current) else {
            continue;
        };
        *transform = Transform {
            translation: previous.translation.lerp(current.translation, progress),
            rotation: previous.rotation.slerp(current.rotation, progress),
            scale: previous.scale.lerp(current.scale, progress),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn tick_to(world: &mut World, entity: Entity, x: f32) {
        world.run_system_once(restore_simulated_transforms).unwrap();
        world.get_mut::<Transform>(entity).unwrap().translation.x = x;
        world.run_system_once(record_simulated_transforms).unwrap();
    }

    #[test]
    fn frames_between_ticks_draw_part_way_and_ticks_resume_from_the_exact_spot() {
        let mut world = World::new();
        let mut fixed = Time::<Fixed>::from_duration(crate::SIMULATION_TICK);
        let entity = world
            .spawn((Transform::default(), TickTransforms::default()))
            .id();
        tick_to(&mut world, entity, 0.0);
        tick_to(&mut world, entity, 10.0);

        fixed.accumulate_overstep(crate::SIMULATION_TICK / 4);
        world.insert_resource(fixed);
        world.run_system_once(draw_between_ticks).unwrap();
        let drawn = world.get::<Transform>(entity).unwrap().translation.x;
        assert!((drawn - 2.5).abs() < 1e-4, "drawn at {drawn}");

        world.run_system_once(restore_simulated_transforms).unwrap();
        assert_eq!(world.get::<Transform>(entity).unwrap().translation.x, 10.0);
    }
}
