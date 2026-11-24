use std::collections::HashMap;

use bevy::audio::Volume;
use bevy::prelude::*;
use rand::RngExt;

use crate::app::i18n::Language;
use crate::app::settings::GameSettings;
use crate::audio::announcer::{LANGUAGES, language_folder};
use crate::sim::catalog::UnitKind;
use crate::sim::components::{Selected, Unit};
use crate::sim::player_commands::LocalTeam;
use crate::ui::orders::{MarkerKind, OrderMarker};

/// Lines recorded per reply; must match `UNIT_REPLIES` in `tools/audio/voice.py`.
const VARIANTS: usize = 2;
const REPLY_VOLUME: f32 = 0.75;
/// Clicking through orders should not turn the radio into a chatter of half-spoken lines.
const QUIET_BETWEEN_REPLIES_SECS: f32 = 0.9;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Speaker {
    Infantry,
    Vehicle,
    Aircraft,
}

impl Speaker {
    const ALL: [Speaker; 3] = [Speaker::Infantry, Speaker::Vehicle, Speaker::Aircraft];

    pub fn of(kind: UnitKind) -> Speaker {
        if kind.flies() {
            Speaker::Aircraft
        } else if UnitKind::INFANTRY.contains(&kind) {
            Speaker::Infantry
        } else {
            Speaker::Vehicle
        }
    }

    fn file(self) -> &'static str {
        match self {
            Speaker::Infantry => "infantry",
            Speaker::Vehicle => "vehicle",
            Speaker::Aircraft => "aircraft",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Reply {
    Select,
    Move,
    Attack,
}

impl Reply {
    const ALL: [Reply; 3] = [Reply::Select, Reply::Move, Reply::Attack];

    fn file(self) -> &'static str {
        match self {
            Reply::Select => "select",
            Reply::Move => "move",
            Reply::Attack => "attack",
        }
    }

    fn to_order(kind: MarkerKind) -> Option<Reply> {
        match kind {
            MarkerKind::Move | MarkerKind::Patrol => Some(Reply::Move),
            MarkerKind::Attack => Some(Reply::Attack),
            MarkerKind::Rally => None,
        }
    }
}

fn recording_path(language: Language, speaker: Speaker, reply: Reply, variant: usize) -> String {
    format!(
        "audio/voice/{}/reply_{}_{}_{}.ogg",
        language_folder(language),
        speaker.file(),
        reply.file(),
        variant + 1
    )
}

#[derive(Resource)]
pub struct UnitVoiceRecordings(HashMap<(Language, Speaker, Reply), Vec<Handle<AudioSource>>>);

pub fn load_unit_voices(mut commands: Commands, asset_server: Res<AssetServer>) {
    let mut recordings = HashMap::new();
    for language in LANGUAGES {
        for speaker in Speaker::ALL {
            for reply in Reply::ALL {
                let lines = (0..VARIANTS)
                    .map(|variant| {
                        asset_server.load(recording_path(language, speaker, reply, variant))
                    })
                    .collect();
                recordings.insert((language, speaker, reply), lines);
            }
        }
    }
    commands.insert_resource(UnitVoiceRecordings(recordings));
}

#[derive(Component)]
pub struct UnitReplyVoice;

/// The voice of a group is the kind most of it is; ties go to the first kind met.
fn group_speaker(kinds: impl Iterator<Item = UnitKind>) -> Option<Speaker> {
    let mut counts: Vec<(Speaker, usize)> = Vec::new();
    for speaker in kinds.map(Speaker::of) {
        match counts.iter_mut().find(|(known, _)| *known == speaker) {
            Some((_, count)) => *count += 1,
            None => counts.push((speaker, 1)),
        }
    }
    counts
        .into_iter()
        .rev()
        .max_by_key(|&(_, count)| count)
        .map(|(speaker, _)| speaker)
}

#[allow(clippy::too_many_arguments)]
pub fn answer_the_player(
    mut commands: Commands,
    time: Res<Time<Real>>,
    settings: Res<GameSettings>,
    local: Res<LocalTeam>,
    marker: Res<OrderMarker>,
    recordings: Option<Res<UnitVoiceRecordings>>,
    mut last_order: Local<u32>,
    mut last_reply_at: Local<f32>,
    newly_selected: Query<(&Unit, &crate::sim::catalog::Team), Added<Selected>>,
    selected: Query<(&Unit, &crate::sim::catalog::Team), With<Selected>>,
    speaking: Query<(), With<UnitReplyVoice>>,
) {
    let ordered = marker.orders_acknowledged != *last_order;
    *last_order = marker.orders_acknowledged;
    let own =
        |(unit, team): (&Unit, &crate::sim::catalog::Team)| (*team == local.0).then_some(unit.0);
    let (speaker, reply) = if ordered {
        let Some(reply) = Reply::to_order(marker.kind) else {
            return;
        };
        (group_speaker(selected.iter().filter_map(own)), reply)
    } else {
        (
            group_speaker(newly_selected.iter().filter_map(own)),
            Reply::Select,
        )
    };
    let Some(speaker) = speaker else { return };
    let now = time.elapsed_secs();
    if now - *last_reply_at < QUIET_BETWEEN_REPLIES_SECS || !speaking.is_empty() {
        return;
    }
    let Some(lines) = recordings
        .as_ref()
        .and_then(|r| r.0.get(&(settings.language, speaker, reply)))
    else {
        return;
    };
    *last_reply_at = now;
    let line = lines[rand::rng().random_range(0..lines.len())].clone();
    commands.spawn((
        UnitReplyVoice,
        AudioPlayer(line),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(REPLY_VOLUME * settings.voice_volume)),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_reply_has_its_lines_in_every_language() {
        for language in LANGUAGES {
            for speaker in Speaker::ALL {
                for reply in Reply::ALL {
                    for variant in 0..VARIANTS {
                        let path = format!(
                            "{}/assets/{}",
                            env!("CARGO_MANIFEST_DIR"),
                            recording_path(language, speaker, reply, variant)
                        );
                        assert!(std::path::Path::new(&path).exists(), "{path}");
                    }
                }
            }
        }
    }

    #[test]
    fn a_mixed_group_answers_in_the_voice_of_most_of_it() {
        let group = [
            UnitKind::Tank,
            UnitKind::Rifleman,
            UnitKind::Rifleman,
            UnitKind::Gunship,
        ];
        assert_eq!(group_speaker(group.into_iter()), Some(Speaker::Infantry));
        assert_eq!(
            group_speaker([UnitKind::Fighter].into_iter()),
            Some(Speaker::Aircraft)
        );
        assert_eq!(group_speaker(std::iter::empty()), None);
    }
}
