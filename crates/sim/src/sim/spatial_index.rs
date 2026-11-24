use bevy::prelude::*;

const BUCKET_SIZE: f32 = 8.0;
/// Below this many bodies checking each one is as fast as searching the buckets; measured in
/// docs/persistent-world.md.
const FEWEST_BODIES_TO_SEARCH: usize = 256;

/// Round bodies sorted by the patch of ground they stand on, so a search near a point visits only the patches
/// around it. Built from a slice of bodies and searched with that same slice.
#[derive(Default)]
pub struct SpatialIndex {
    by_bucket: Vec<(IVec2, usize)>,
    largest_radius: f32,
}

impl SpatialIndex {
    pub fn rebuild(&mut self, bodies: impl IntoIterator<Item = (Vec3, f32)>) {
        self.by_bucket.clear();
        self.largest_radius = 0.0;
        for (index, (position, radius)) in bodies.into_iter().enumerate() {
            self.by_bucket.push((bucket_of(position.xz()), index));
            self.largest_radius = self.largest_radius.max(radius);
        }
        if self.by_bucket.len() >= FEWEST_BODIES_TO_SEARCH {
            self.by_bucket
                .sort_unstable_by_key(|&(bucket, index)| (bucket.y, bucket.x, index));
        }
    }

    /// Every body whose edge may lie within `reach` of `from`, in the slice's order; a superset, so the caller
    /// still measures the distance.
    pub fn near<'a, T>(&self, bodies: &'a [T], from: Vec3, reach: f32) -> NearbyBodies<'a, T> {
        debug_assert_eq!(bodies.len(), self.by_bucket.len());
        if bodies.len() < FEWEST_BODIES_TO_SEARCH {
            return NearbyBodies::Every(bodies.iter());
        }
        let search = reach + self.largest_radius;
        let low = bucket_of(from.xz() - Vec2::splat(search));
        let high = bucket_of(from.xz() + Vec2::splat(search));
        let searched_buckets = (high - low + IVec2::ONE).as_vec2().element_product();
        if searched_buckets >= bodies.len() as f32 {
            return NearbyBodies::Every(bodies.iter());
        }
        let mut found = Vec::new();
        for row in low.y..=high.y {
            let first = self.position_of(IVec2::new(low.x, row));
            let past_last = self.position_of(IVec2::new(high.x + 1, row));
            found.extend(
                self.by_bucket[first..past_last]
                    .iter()
                    .map(|&(_, index)| index),
            );
        }
        found.sort_unstable();
        NearbyBodies::Found {
            bodies,
            found: found.into_iter(),
        }
    }

    fn position_of(&self, bucket: IVec2) -> usize {
        self.by_bucket
            .partition_point(|&(other, _)| (other.y, other.x) < (bucket.y, bucket.x))
    }
}

pub enum NearbyBodies<'a, T> {
    Every(std::slice::Iter<'a, T>),
    Found {
        bodies: &'a [T],
        found: std::vec::IntoIter<usize>,
    },
}

impl<'a, T> Iterator for NearbyBodies<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        match self {
            NearbyBodies::Every(all) => all.next(),
            NearbyBodies::Found { bodies, found } => found.next().map(|index| &bodies[index]),
        }
    }
}

fn bucket_of(point: Vec2) -> IVec2 {
    (point / BUCKET_SIZE).floor().as_ivec2()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Body {
        id: usize,
        position: Vec3,
        radius: f32,
    }

    fn at(x: f32, z: f32) -> Vec3 {
        Vec3::new(x, 0.0, z)
    }

    fn scattered_bodies(offset: Vec3) -> Vec<Body> {
        (0..600)
            .map(|id| Body {
                id,
                position: at((id * 37 % 600) as f32, (id * 53 % 600) as f32) + offset,
                radius: 0.5,
            })
            .collect()
    }

    fn indexed(bodies: &[Body]) -> SpatialIndex {
        let mut index = SpatialIndex::default();
        index.rebuild(bodies.iter().map(|body| (body.position, body.radius)));
        index
    }

    fn ids_near(index: &SpatialIndex, bodies: &[Body], from: Vec3, reach: f32) -> Vec<usize> {
        index
            .near(bodies, from, reach)
            .map(|body| body.id)
            .collect()
    }

    #[test]
    fn finds_bodies_whose_edge_is_within_reach_in_the_order_given() {
        let mut bodies = scattered_bodies(Vec3::ZERO);
        let large = Body {
            id: bodies.len(),
            position: at(93.0, 103.0),
            radius: 4.0,
        };
        bodies.push(large);
        let index = indexed(&bodies);
        let from = at(100.0, 100.0);

        let found = ids_near(&index, &bodies, from, 6.0);

        assert!(found.is_sorted());
        for body in &bodies {
            let within = body.position.distance(from) - body.radius <= 6.0;
            assert!(
                !within || found.contains(&body.id),
                "body {} missed",
                body.id
            );
        }
        assert!(found.contains(&600));
        assert!(found.len() < bodies.len() / 10);
    }

    #[test]
    fn a_reach_wider_than_the_bodies_finds_all_of_them() {
        let bodies = scattered_bodies(Vec3::ZERO);
        let index = indexed(&bodies);

        let found = ids_near(&index, &bodies, at(0.0, 0.0), 10_000.0);

        assert_eq!(found, (0..bodies.len()).collect::<Vec<_>>());
    }

    #[test]
    fn rebuilding_forgets_the_bodies_before() {
        let mut index = indexed(&scattered_bodies(Vec3::ZERO));
        let far_away = Vec3::new(5000.0, 0.0, 5000.0);
        let moved = scattered_bodies(far_away);
        index.rebuild(moved.iter().map(|body| (body.position, body.radius)));

        assert!(ids_near(&index, &moved, at(0.0, 0.0), 5.0).is_empty());
        assert!(ids_near(&index, &moved, far_away, 5.0).contains(&0));
    }
}
