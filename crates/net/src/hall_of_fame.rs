//! What outlives a world: who won it, and who came second and third. Kept as `hall_of_fame.json` beside the
//! world's save, one entry for every world that ended. See docs/eras.md.

use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use rimefront_sim::sim::catalog::SideColor;

pub const HALL_OF_FAME_FILE: &str = "hall_of_fame.json";

/// One side of a place: its colour and what its player was called.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct FamedSide {
    pub color: Option<SideColor>,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct HallOfFameEntry {
    pub map_seed: u64,
    /// Seconds since 1970 when the world ended, by the server's clock.
    pub ended_at: u64,
    /// The first, second and third place, each a camp of allies or a side alone; fewer when fewer sides
    /// built anything of a Project.
    pub places: Vec<Vec<FamedSide>>,
}

impl HallOfFameEntry {
    pub fn now(map_seed: u64, places: Vec<Vec<FamedSide>>) -> Self {
        let ended_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_secs());
        Self {
            map_seed,
            ended_at,
            places,
        }
    }

    /// The winners, for telling the players.
    pub fn winners(&self) -> String {
        match self.places.first() {
            Some(camp) => camp
                .iter()
                .map(|side| side.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            None => "nobody".into(),
        }
    }
}

pub fn read(directory: &Path) -> io::Result<Vec<HallOfFameEntry>> {
    match std::fs::read(directory.join(HALL_OF_FAME_FILE)) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

/// Adds `entry` to the hall in `directory`, through a staged file, so a crash leaves the hall as it was.
pub fn add(directory: &Path, entry: HallOfFameEntry) -> io::Result<()> {
    let mut hall = read(directory)?;
    hall.push(entry);
    std::fs::create_dir_all(directory)?;
    let staged = directory.join(format!("{HALL_OF_FAME_FILE}.new"));
    std::fs::write(
        &staged,
        serde_json::to_vec_pretty(&hall).map_err(io::Error::other)?,
    )?;
    std::fs::rename(staged, directory.join(HALL_OF_FAME_FILE))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hall_keeps_every_ended_world() {
        let directory = std::env::temp_dir().join(format!("rimefront_hall_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        let side = |name: &str| FamedSide {
            color: Some(SideColor([1, 2, 3])),
            name: name.into(),
        };
        add(
            &directory,
            HallOfFameEntry::now(29, vec![vec![side("Ada"), side("Bo")]]),
        )
        .unwrap();
        add(&directory, HallOfFameEntry::now(30, Vec::new())).unwrap();
        let hall = read(&directory).unwrap();
        assert_eq!(hall.len(), 2);
        assert_eq!(hall[0].winners(), "Ada, Bo");
        assert_eq!(hall[1].winners(), "nobody");
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
