use bevy::prelude::*;

use serde::{Deserialize, Serialize};

use crate::i18n::{tr, tr_fmt};
use crate::sim::catalog::{BuildingKind, Team, UnitKind};
use crate::sim::combat::DamageDealt;
use crate::sim::components::{Building, Harvester};
use crate::sim::cues::{Sound, SoundCue};
use crate::sim::economy::Economies;
use crate::sim::eras::Era;
use crate::sim::player_commands::LocalTeam;
use crate::sim::production::{BuildItem, EnqueueRefusal};
use crate::sim::sides::{PerSide, Sides};
use crate::sim::veterancy::Rank;

const MESSAGE_LIFETIME_SECS: f32 = 4.0;
/// A mission's own messages carry orders and warnings, so they stay long enough to be read in a fight.
const MISSION_MESSAGE_LIFETIME_SECS: f32 = 12.0;
const MAX_VISIBLE_MESSAGES: usize = 4;
const ATTACK_WARNING_COOLDOWN_SECS: f32 = 12.0;

/// Where the latest attack on the local side was reported, for the jump-to-alert key.
#[derive(Resource, Default)]
pub struct LastAlert(pub Option<Vec3>);

/// Short status lines for the player, shown at the top of the battlefield.
#[derive(Resource, Default)]
pub struct Notifications {
    messages: Vec<(String, f32)>,
}

impl Notifications {
    pub fn push(&mut self, message: impl Into<String>) {
        self.push_for(message.into(), MESSAGE_LIFETIME_SECS);
    }

    pub fn push_mission_message(&mut self, message: impl Into<String>) {
        self.push_for(message.into(), MISSION_MESSAGE_LIFETIME_SECS);
    }

    fn push_for(&mut self, message: String, lifetime_secs: f32) {
        self.messages.retain(|(existing, _)| *existing != message);
        self.messages.push((message, lifetime_secs));
        if self.messages.len() > MAX_VISIBLE_MESSAGES {
            self.messages.remove(0);
        }
    }

    pub fn visible(&self) -> impl Iterator<Item = (&str, f32)> {
        self.messages
            .iter()
            .map(|(message, remaining)| (message.as_str(), (remaining / 0.6).min(1.0)))
    }

    fn age(&mut self, dt: f32) {
        for (_, remaining) in &mut self.messages {
            *remaining -= dt;
        }
        self.messages.retain(|(_, remaining)| *remaining > 0.0);
    }
}

/// Something a side is told; the game of the player on that side puts it in words and sound.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum SideNews {
    Finished(BuildItem),
    Promoted(UnitKind, Rank),
    Captured(BuildingKind),
    CapturedByEnemy(BuildingKind),
    Repaired(BuildingKind),
    RepairStarted,
    StructureSold,
    CargoSold,
    Refused(EnqueueRefusal),
    NoMine,
    /// An order against a side whose player is away, which the shelter turned down.
    Sheltered,
    /// A new crystal deposit appeared somewhere in the world.
    CrystalSurfaced,
    /// A unit is finished but waits: the side has as many units as a world allows.
    UnitLimitReached,
    BaseUnderAttack,
    HarvesterUnderAttack,
    LowPower,
    /// Another side asks for an alliance; offering one back seals it.
    AllianceOffered(Team),
    AllianceFormed(Team),
    /// The two sides are no longer allies.
    AllianceLeft(Team),
    /// The player came back to a world after being away.
    WelcomeBack(AwayReport),
    /// A new age has begun: its army is built from now on, and the old one can be promoted.
    AgeTurned(Era),
}

/// How long a player was away from a world and what their side lost meanwhile; the losses are `None` when the
/// world was restarted while they were away and did not keep count.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AwayReport {
    pub minutes: u32,
    pub losses: Option<AwayLosses>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct AwayLosses {
    pub structures: u32,
    pub units: u32,
}

impl AwayReport {
    fn message(self) -> String {
        let hours = self.minutes / 60;
        let away = if hours > 0 {
            tr_fmt("{} h {} min", &[&hours, &(self.minutes % 60)])
        } else {
            tr_fmt("{} min", &[&self.minutes])
        };
        match self.losses {
            Some(AwayLosses {
                structures: 0,
                units: 0,
            }) => tr_fmt("Welcome back after {}: nothing was lost", &[&away]),
            Some(lost) => tr_fmt(
                "Welcome back after {}: lost {} structures and {} units",
                &[&away, &lost.structures, &lost.units],
            ),
            None => tr_fmt("Welcome back after {}", &[&away]),
        }
    }
}

impl SideNews {
    pub fn message(self) -> Option<String> {
        let name = |kind: BuildingKind| tr(kind.spec().name);
        Some(match self {
            SideNews::Finished(BuildItem::Structure(_)) => tr("Construction complete").into(),
            SideNews::Finished(BuildItem::Unit(kind)) => {
                tr_fmt("{} ready", &[&tr(kind.spec().name)])
            }
            SideNews::Finished(BuildItem::Research(upgrade)) => {
                tr_fmt("Research complete: {}", &[&tr(upgrade.spec().name)])
            }
            SideNews::Promoted(kind, rank) => {
                tr_fmt("{} promoted to {}", &[&tr(kind.spec().name), &rank.name()])
            }
            SideNews::Captured(kind) => tr_fmt("{} captured", &[&name(kind)]),
            SideNews::CapturedByEnemy(kind) => tr_fmt("{} captured by the enemy", &[&name(kind)]),
            SideNews::Repaired(kind) => tr_fmt("{} repaired", &[&name(kind)]),
            SideNews::Refused(refusal) => refusal.message(),
            SideNews::NoMine => tr("No mine: the cell has one or the credits ran out").into(),
            SideNews::Sheltered => tr("Its player is away: it cannot be attacked now").into(),
            SideNews::CrystalSurfaced => {
                tr("New crystal has surfaced somewhere in the world").into()
            }
            SideNews::UnitLimitReached => {
                tr("Unit limit reached: lose units to deploy more").into()
            }
            SideNews::BaseUnderAttack => tr("Our base is under attack!").into(),
            SideNews::HarvesterUnderAttack => tr("Our harvester is under attack!").into(),
            SideNews::LowPower => tr("Low power - production slowed").into(),
            SideNews::AllianceOffered(team) => tr_fmt("{} offers an alliance", &[&team.name()]),
            SideNews::AllianceFormed(team) => tr_fmt("Allied with {}", &[&team.name()]),
            SideNews::AllianceLeft(team) => tr_fmt("No longer allied with {}", &[&team.name()]),
            SideNews::WelcomeBack(report) => report.message(),
            SideNews::AgeTurned(era) => tr_fmt(
                "A new age begins: {}. Promote your army and defences",
                &[&era.label()],
            ),
            SideNews::Finished(BuildItem::Promotion { into, .. }) => {
                tr_fmt("Promoted to {}", &[&tr(into.spec().name)])
            }
            SideNews::Finished(BuildItem::Fortification { into, .. }) => {
                tr_fmt("Promoted to {}", &[&tr(into.spec().name)])
            }
            SideNews::RepairStarted | SideNews::StructureSold | SideNews::CargoSold => return None,
        })
    }

    pub fn sound(self) -> Option<Sound> {
        Some(match self {
            SideNews::Finished(BuildItem::Structure(_)) => Sound::ConstructionComplete,
            SideNews::Finished(BuildItem::Research(_)) => Sound::ResearchComplete,
            SideNews::Finished(BuildItem::Unit(_)) => Sound::UnitReady,
            SideNews::Finished(BuildItem::Promotion { .. } | BuildItem::Fortification { .. }) => {
                Sound::UnitPromoted
            }
            SideNews::AgeTurned(_) => Sound::BaseAlert,
            SideNews::Promoted(..) => Sound::UnitPromoted,
            SideNews::Captured(_) => Sound::StructureCaptured,
            SideNews::CapturedByEnemy(_) => Sound::StructureLost,
            SideNews::Repaired(_) | SideNews::RepairStarted => Sound::RepairStarted,
            SideNews::StructureSold => Sound::StructureSold,
            SideNews::CargoSold => Sound::Credits,
            SideNews::BaseUnderAttack => Sound::BaseAlert,
            SideNews::HarvesterUnderAttack => Sound::HarvesterAlert,
            SideNews::Refused(_)
            | SideNews::NoMine
            | SideNews::Sheltered
            | SideNews::CrystalSurfaced
            | SideNews::UnitLimitReached
            | SideNews::LowPower
            | SideNews::AllianceOffered(_)
            | SideNews::AllianceFormed(_)
            | SideNews::AllianceLeft(_)
            | SideNews::WelcomeBack(_) => {
                return None;
            }
        })
    }

    fn is_alarm(self) -> bool {
        matches!(
            self,
            SideNews::BaseUnderAttack | SideNews::HarvesterUnderAttack
        )
    }
}

/// News for one side; `at` is where it happened, for the news whose sound comes from a place.
#[derive(Message, Clone, Copy, Debug)]
pub struct News {
    pub team: Team,
    pub news: SideNews,
    pub at: Option<Vec3>,
}

impl News {
    pub fn to(team: Team, news: SideNews) -> Self {
        Self {
            team,
            news,
            at: None,
        }
    }

    pub fn at(team: Team, news: SideNews, at: Vec3) -> Self {
        Self {
            team,
            news,
            at: Some(at),
        }
    }
}

/// The local side's news becomes the lines at the top of the battlefield and the sounds with them.
pub fn tell_local_side(
    local: Res<LocalTeam>,
    time: Res<Time>,
    mut news: MessageReader<News>,
    mut notifications: ResMut<Notifications>,
    mut cues: MessageWriter<SoundCue>,
    mut alert: ResMut<LastAlert>,
) {
    notifications.age(time.delta_secs());
    for told in news.read().filter(|told| told.team == local.0) {
        if let Some(message) = told.news.message() {
            notifications.push(message);
        }
        if let Some(sound) = told.news.sound() {
            cues.write(match (told.at, told.news.is_alarm()) {
                (Some(at), false) => SoundCue::at(sound, at),
                _ => SoundCue::interface(sound),
            });
        }
        if told.news.is_alarm() {
            alert.0 = told.at;
        }
    }
}

#[derive(Default, Clone)]
pub struct BaseWatch {
    low_power: bool,
    warning_cooldown: f32,
    harvester_warning_cooldown: f32,
}

/// Warns every side of attacks on its base and harvesters, and of running short of power.
pub fn watch_bases(
    time: Res<Time>,
    sides: Res<Sides>,
    economies: Res<Economies>,
    mut news: MessageWriter<News>,
    mut watches: Local<PerSide<BaseWatch>>,
    mut hits: MessageReader<DamageDealt>,
    victims: Query<(&Transform, Has<Building>, Has<Harvester>)>,
) {
    let dt = time.delta_secs();
    for watch in watches.iter_mut() {
        watch.warning_cooldown -= dt;
        watch.harvester_warning_cooldown -= dt;
    }
    for hit in hits.read() {
        let Ok((transform, is_building, is_harvester)) = victims.get(hit.victim) else {
            continue;
        };
        let watch = watches.of_mut(hit.team);
        let warning = if is_building && watch.warning_cooldown <= 0.0 {
            watch.warning_cooldown = ATTACK_WARNING_COOLDOWN_SECS;
            SideNews::BaseUnderAttack
        } else if is_harvester && watch.harvester_warning_cooldown <= 0.0 {
            watch.harvester_warning_cooldown = ATTACK_WARNING_COOLDOWN_SECS;
            SideNews::HarvesterUnderAttack
        } else {
            continue;
        };
        news.write(News::at(hit.team, warning, transform.translation));
    }
    for team in sides.iter() {
        let low_power = economies.of(team).has_low_power();
        let watch = watches.of_mut(team);
        if low_power && !watch.low_power {
            news.write(News::to(team, SideNews::LowPower));
        }
        watch.low_power = low_power;
    }
}
