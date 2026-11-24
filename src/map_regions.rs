use bevy::prelude::*;

use crate::map::{GameMap, MapBounds, chunk_of, map_bounds};

const STEPS: [IVec2; 4] = [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y];

/// Areas of passable cells a unit can drive between, so a route into another one fails at once instead of
/// after searching every cell it could reach. Each chunk labels its own parts, and the parts are joined across
/// chunk edges, so a structure placed or removed relabels only its chunk and the joins.
pub(crate) struct Regions {
    chunks: Vec<ChunkParts>,
    stale_chunks: Vec<bool>,
    joins_stale: bool,
    /// The region of every part of every chunk, numbered from `first_part` of its chunk.
    region_of_part: Vec<u32>,
    first_part: Vec<u32>,
}

/// The connected parts of one chunk's passable cells, labelled from 1; 0 for a blocked cell.
#[derive(Default)]
struct ChunkParts {
    labels: Vec<u16>,
    count: u16,
}

impl Regions {
    pub(crate) const IMPASSABLE: u32 = 0;

    pub(crate) fn new() -> Self {
        let chunks = map_bounds().chunk_count();
        Self {
            chunks: (0..chunks).map(|_| ChunkParts::default()).collect(),
            stale_chunks: vec![true; chunks],
            joins_stale: true,
            region_of_part: Vec::new(),
            first_part: Vec::new(),
        }
    }

    pub(crate) fn forget_around(&mut self, cell: IVec2) {
        if let Some(index) = map_bounds().chunk_index(chunk_of(cell)) {
            self.stale_chunks[index] = true;
            self.joins_stale = true;
        }
    }

    pub(crate) fn forget_all(&mut self) {
        self.stale_chunks.fill(true);
        self.joins_stale = true;
    }

    /// The region a cell belongs to, worked out again where the map changed; `IMPASSABLE` for a blocked cell.
    pub(crate) fn of(&mut self, map: &GameMap, cell: IVec2) -> u32 {
        self.refresh(map);
        let bounds = map_bounds();
        let Some(chunk_index) = bounds.chunk_index(chunk_of(cell)) else {
            return Self::IMPASSABLE;
        };
        let part = self.chunks[chunk_index].part_at(bounds, cell);
        if part == 0 {
            return Self::IMPASSABLE;
        }
        self.region_of_part[(self.first_part[chunk_index] + part as u32 - 1) as usize] + 1
    }

    fn refresh(&mut self, map: &GameMap) {
        if !self.joins_stale {
            return;
        }
        let bounds = map_bounds();
        for (index, stale) in self.stale_chunks.iter_mut().enumerate() {
            if *stale {
                self.chunks[index] = ChunkParts::work_out(map, bounds, bounds.chunk_at(index));
                *stale = false;
            }
        }
        self.join_parts(map, bounds);
        self.joins_stale = false;
    }

    fn join_parts(&mut self, map: &GameMap, bounds: MapBounds) {
        self.first_part.clear();
        let mut parts = 0;
        for chunk in &self.chunks {
            self.first_part.push(parts);
            parts += chunk.count as u32;
        }
        let mut joined = UnionFind::new(parts as usize);
        for index in 0..self.chunks.len() {
            let chunk = bounds.chunk_at(index);
            let (first, last) = bounds.cells_of_chunk(chunk);
            let east_edge = (first.y..=last.y).map(|y| (IVec2::new(last.x, y), IVec2::X));
            let south_edge = (first.x..=last.x).map(|x| (IVec2::new(x, last.y), IVec2::Y));
            for (cell, step) in east_edge.chain(south_edge) {
                let across = cell + step;
                let Some(across_index) = bounds.chunk_index(chunk_of(across)) else {
                    continue;
                };
                if !map.is_passable(cell) || !map.is_passable(across) {
                    continue;
                }
                let here = self.part_number(bounds, index, cell);
                let there = self.part_number(bounds, across_index, across);
                joined.join(here, there);
            }
        }
        self.region_of_part = (0..parts as usize)
            .map(|part| joined.root(part) as u32)
            .collect();
    }

    fn part_number(&self, bounds: MapBounds, chunk_index: usize, cell: IVec2) -> usize {
        let part = self.chunks[chunk_index].part_at(bounds, cell);
        (self.first_part[chunk_index] + part as u32 - 1) as usize
    }
}

impl ChunkParts {
    fn part_at(&self, bounds: MapBounds, cell: IVec2) -> u16 {
        let (first, last) = bounds.cells_of_chunk(chunk_of(cell));
        let local = cell - first;
        let width = last.x - first.x + 1;
        self.labels
            .get((local.y * width + local.x) as usize)
            .copied()
            .unwrap_or(0)
    }

    fn work_out(map: &GameMap, bounds: MapBounds, chunk: IVec2) -> Self {
        let (first, last) = bounds.cells_of_chunk(chunk);
        let size = last - first + IVec2::ONE;
        let slot = |cell: IVec2| {
            let local = cell - first;
            let inside = local.cmpge(IVec2::ZERO).all() && local.cmplt(size).all();
            inside.then(|| (local.y * size.x + local.x) as usize)
        };
        let mut labels = vec![0u16; (size.x * size.y) as usize];
        let mut count = 0;
        let mut frontier = Vec::new();
        for index in 0..labels.len() {
            let seed = first + IVec2::new(index as i32 % size.x, index as i32 / size.x);
            if labels[index] != 0 || !map.is_passable(seed) {
                continue;
            }
            count += 1;
            labels[index] = count;
            frontier.push(seed);
            while let Some(cell) = frontier.pop() {
                for step in STEPS {
                    let next = cell + step;
                    let Some(at) = slot(next) else { continue };
                    if labels[at] == 0 && map.is_passable(next) {
                        labels[at] = count;
                        frontier.push(next);
                    }
                }
            }
        }
        Self { labels, count }
    }
}

struct UnionFind(Vec<usize>);

impl UnionFind {
    fn new(size: usize) -> Self {
        Self((0..size).collect())
    }

    fn root(&mut self, mut node: usize) -> usize {
        while self.0[node] != node {
            self.0[node] = self.0[self.0[node]];
            node = self.0[node];
        }
        node
    }

    fn join(&mut self, a: usize, b: usize) {
        let (a, b) = (self.root(a), self.root(b));
        self.0[a.max(b)] = a.min(b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::set_map_bounds;
    use rand::rngs::StdRng;
    use rand::{RngExt, SeedableRng};

    /// Labels the whole map in one flood fill, as before chunks.
    fn flood_filled(map: &GameMap, bounds: MapBounds) -> Vec<u32> {
        let mut labels = vec![0; bounds.area()];
        let mut next = 0;
        for index in 0..labels.len() {
            let seed = bounds.cell_at(index);
            if labels[index] != 0 || !map.is_passable(seed) {
                continue;
            }
            next += 1;
            labels[index] = next;
            let mut frontier = vec![seed];
            while let Some(cell) = frontier.pop() {
                for step in STEPS {
                    let neighbour = cell + step;
                    let Some(at) = bounds.index(neighbour) else {
                        continue;
                    };
                    if labels[at] == 0 && map.is_passable(neighbour) {
                        labels[at] = next;
                        frontier.push(neighbour);
                    }
                }
            }
        }
        labels
    }

    #[test]
    fn chunked_regions_connect_exactly_what_one_flood_fill_connects() {
        let bounds = MapBounds {
            min: IVec2::new(-20, 9),
            size: IVec2::new(100, 70),
        };
        set_map_bounds(bounds);
        let mut rng = StdRng::seed_from_u64(4);
        let mut map = GameMap::open_field();
        for _ in 0..bounds.area() * 2 / 5 {
            let cell = bounds.cell_at(rng.random_range(0..bounds.area()));
            map.set_obstacle(cell, true);
        }
        let mut regions = Regions::new();
        for round in 0..30 {
            for _ in 0..rng.random_range(1..40) {
                let cell = bounds.cell_at(rng.random_range(0..bounds.area()));
                map.set_obstacle(cell, rng.random_bool(0.45));
                regions.forget_around(cell);
            }
            let reference = flood_filled(&map, bounds);
            for _ in 0..400 {
                let (a, b) = (
                    rng.random_range(0..bounds.area()),
                    rng.random_range(0..bounds.area()),
                );
                let (cell_a, cell_b) = (bounds.cell_at(a), bounds.cell_at(b));
                assert_eq!(
                    regions.of(&map, cell_a) == regions.of(&map, cell_b),
                    reference[a] == reference[b],
                    "round {round}: {cell_a} and {cell_b}"
                );
            }
        }
    }
}
