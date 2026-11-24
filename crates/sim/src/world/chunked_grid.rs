use bevy::prelude::*;

use crate::world::map::{CHUNK_CELLS, MapBounds, chunk_of};

const CELLS_PER_CHUNK: usize = (CHUNK_CELLS * CHUNK_CELLS) as usize;

/// A value for every cell of the map, kept by chunk: a chunk exists once it is opened, and the map can widen
/// without moving the chunks it has. A cell outside the bounds or in a chunk not yet opened has no value.
#[derive(Clone, PartialEq, Debug)]
pub struct ChunkedGrid<T> {
    bounds: MapBounds,
    chunks: Vec<Option<Box<[T]>>>,
    fill: T,
}

impl<T: Copy> ChunkedGrid<T> {
    /// Every chunk of `bounds` opened and filled with `fill`.
    pub fn filled(bounds: MapBounds, fill: T) -> Self {
        let mut grid = Self {
            bounds,
            chunks: vec![None; bounds.chunk_count()],
            fill,
        };
        for index in 0..bounds.chunk_count() {
            grid.open_chunk(bounds.chunk_at(index));
        }
        grid
    }

    pub fn bounds(&self) -> MapBounds {
        self.bounds
    }

    pub fn open_chunk(&mut self, chunk: IVec2) {
        if let Some(index) = self.bounds.chunk_index(chunk) {
            self.chunks[index].get_or_insert_with(|| vec![self.fill; CELLS_PER_CHUNK].into());
        }
    }

    pub fn has_chunk(&self, chunk: IVec2) -> bool {
        self.bounds
            .chunk_index(chunk)
            .is_some_and(|index| self.chunks[index].is_some())
    }

    fn slot(&self, cell: IVec2) -> Option<(usize, usize)> {
        if !self.bounds.contains(cell) {
            return None;
        }
        let chunk = self.bounds.chunk_index(chunk_of(cell))?;
        let local = cell.rem_euclid(IVec2::splat(CHUNK_CELLS));
        Some((chunk, (local.y * CHUNK_CELLS + local.x) as usize))
    }

    pub fn get(&self, cell: IVec2) -> Option<T> {
        let (chunk, at) = self.slot(cell)?;
        self.chunks[chunk].as_ref().map(|cells| cells[at])
    }

    pub fn get_mut(&mut self, cell: IVec2) -> Option<&mut T> {
        let (chunk, at) = self.slot(cell)?;
        self.chunks[chunk].as_mut().map(|cells| &mut cells[at])
    }

    /// Changes every opened cell, those of chunks reaching past the bounds included.
    pub fn update_all(&mut self, change: impl Fn(T) -> T) {
        for cells in self.chunks.iter_mut().flatten() {
            for value in cells.iter_mut() {
                *value = change(*value);
            }
        }
    }

    /// Sets every opened cell to `value`.
    pub fn fill_all(&mut self, value: T) {
        for cells in self.chunks.iter_mut().flatten() {
            cells.fill(value);
        }
    }

    /// The chunks whose cells differ from those of `other`, a grid of the same bounds.
    pub fn chunks_differing_from(&self, other: &Self) -> Vec<IVec2>
    where
        T: PartialEq,
    {
        self.chunks
            .iter()
            .zip(&other.chunks)
            .enumerate()
            .filter(|(_, (mine, theirs))| mine != theirs)
            .map(|(index, _)| self.bounds.chunk_at(index))
            .collect()
    }

    /// Every cell of the bounds row by row, as the dense grids of saved games have them.
    pub fn to_dense(&self) -> Vec<T> {
        self.bounds
            .cells()
            .map(|cell| self.get(cell).unwrap_or(self.fill))
            .collect()
    }

    /// Takes the values of a dense grid of the same bounds, row by row.
    pub fn set_from_dense(&mut self, values: &[T]) {
        let bounds = self.bounds;
        for (cell, &value) in bounds.cells().zip(values) {
            if let Some(slot) = self.get_mut(cell) {
                *slot = value;
            }
        }
    }

    /// Covers `wider`, which contains the bounds; the chunks already opened keep their cells.
    pub fn widen(&mut self, wider: MapBounds) {
        let mut chunks = vec![None; wider.chunk_count()];
        for (index, cells) in self.chunks.drain(..).enumerate() {
            let chunk = self.bounds.chunk_at(index);
            if let (Some(cells), Some(new_index)) = (cells, wider.chunk_index(chunk)) {
                chunks[new_index] = Some(cells);
            }
        }
        self.chunks = chunks;
        self.bounds = wider;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widening_keeps_every_cell_where_it_was() {
        let narrow = MapBounds {
            min: IVec2::new(-5, 3),
            size: IVec2::new(50, 40),
        };
        let mut grid = ChunkedGrid::filled(narrow, 0u32);
        for (index, cell) in narrow.cells().enumerate() {
            *grid.get_mut(cell).unwrap() = index as u32;
        }
        let before = grid.to_dense();
        let wide = MapBounds {
            min: narrow.min - IVec2::splat(CHUNK_CELLS),
            size: narrow.size + IVec2::splat(2 * CHUNK_CELLS),
        };
        grid.widen(wide);

        let kept: Vec<u32> = narrow.cells().map(|cell| grid.get(cell).unwrap()).collect();
        assert_eq!(kept, before);
        let new_cell = wide.min;
        assert_eq!(
            grid.get(new_cell),
            None,
            "a chunk not yet opened has no cells"
        );
        grid.open_chunk(chunk_of(new_cell));
        assert_eq!(grid.get(new_cell), Some(0));
    }

    #[test]
    fn a_dense_grid_goes_out_and_comes_back_unchanged() {
        let bounds = MapBounds {
            min: IVec2::new(3, -7),
            size: IVec2::new(70, 33),
        };
        let mut grid = ChunkedGrid::filled(bounds, false);
        let dense: Vec<bool> = (0..bounds.area()).map(|i| i % 7 == 0).collect();
        grid.set_from_dense(&dense);
        assert_eq!(grid.to_dense(), dense);
    }
}
