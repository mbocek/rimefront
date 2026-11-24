use std::collections::HashMap;

use bevy::audio::Volume;
use bevy::prelude::*;

use crate::app::i18n::Language;
use crate::app::settings::GameSettings;
use crate::audio::sound::{Sound, SoundCue};

/// What the radio operator reports; the recordings live in `assets/audio/voice/<language>/`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Report {
    UnitReady,
    ConstructionComplete,
    ResearchComplete,
    BaseUnderAttack,
    HarvesterUnderAttack,
    StructureLost,
}

impl Report {
    const ALL: [Report; 6] = [
        Report::UnitReady,
        Report::ConstructionComplete,
        Report::ResearchComplete,
        Report::BaseUnderAttack,
        Report::HarvesterUnderAttack,
        Report::StructureLost,
    ];

    pub fn of(sound: Sound) -> Option<Report> {
        match sound {
            Sound::UnitReady => Some(Report::UnitReady),
            Sound::ConstructionComplete => Some(Report::ConstructionComplete),
            Sound::ResearchComplete => Some(Report::ResearchComplete),
            Sound::BaseAlert => Some(Report::BaseUnderAttack),
            Sound::HarvesterAlert => Some(Report::HarvesterUnderAttack),
            Sound::StructureLost => Some(Report::StructureLost),
            _ => None,
        }
    }

    fn file(self) -> &'static str {
        match self {
            Report::UnitReady => "unit_ready",
            Report::ConstructionComplete => "construction_complete",
            Report::ResearchComplete => "research_complete",
            Report::BaseUnderAttack => "base_under_attack",
            Report::HarvesterUnderAttack => "harvester_under_attack",
            Report::StructureLost => "structure_lost",
        }
    }

    /// Alarms are worth repeating sooner than routine news, which would otherwise chatter on every unit.
    fn quiet_secs(self) -> f32 {
        match self {
            Report::UnitReady | Report::ConstructionComplete => 8.0,
            _ => 4.0,
        }
    }
}

pub fn language_folder(language: Language) -> &'static str {
    match language {
        Language::English => "en",
        Language::Slovak => "sk",
        Language::Czech => "cs",
    }
}

pub const LANGUAGES: [Language; 3] = [Language::English, Language::Slovak, Language::Czech];
/// The operator speaks just after the chime, not over it.
const AFTER_CHIME_SECS: f32 = 0.35;
const VOICE_VOLUME: f32 = 0.9;

#[derive(Resource)]
pub struct Recordings(HashMap<(Language, Report), Handle<AudioSource>>);

pub fn load_recordings(mut commands: Commands, asset_server: Res<AssetServer>) {
    let mut recordings = HashMap::new();
    for language in LANGUAGES {
        for report in Report::ALL {
            let path = format!(
                "audio/voice/{}/{}.ogg",
                language_folder(language),
                report.file()
            );
            recordings.insert((language, report), asset_server.load(path));
        }
    }
    commands.insert_resource(Recordings(recordings));
}

#[derive(Component)]
pub struct OperatorVoice;

#[derive(Default)]
pub struct Operator {
    waiting: Option<(f32, Report)>,
    last_said: HashMap<Report, f32>,
}

/// One report at a time: news arriving while the operator speaks or waits is dropped, alarms first.
pub fn announce(
    mut commands: Commands,
    mut operator: Local<Operator>,
    mut cues: MessageReader<SoundCue>,
    time: Res<Time<Real>>,
    settings: Res<GameSettings>,
    recordings: Option<Res<Recordings>>,
    speaking: Query<(), With<OperatorVoice>>,
) {
    let now = time.elapsed_secs();
    for report in cues.read().filter_map(|cue| Report::of(cue.sound)) {
        let recently = operator
            .last_said
            .get(&report)
            .is_some_and(|&said| now - said < report.quiet_secs());
        let more_urgent = operator
            .waiting
            .is_none_or(|(_, waiting)| report.quiet_secs() < waiting.quiet_secs());
        if !recently && more_urgent {
            operator.waiting = Some((now + AFTER_CHIME_SECS, report));
        }
    }
    let Some((due, report)) = operator.waiting else {
        return;
    };
    if now < due || !speaking.is_empty() {
        return;
    }
    operator.waiting = None;
    operator.last_said.insert(report, now);
    let Some(recording) = recordings.and_then(|r| r.0.get(&(settings.language, report)).cloned())
    else {
        return;
    };
    commands.spawn((
        OperatorVoice,
        AudioPlayer(recording),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(VOICE_VOLUME * settings.voice_volume)),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_report_has_a_recording_in_every_language() {
        for language in LANGUAGES {
            for report in Report::ALL {
                let path = format!(
                    "{}/assets/audio/voice/{}/{}.ogg",
                    env!("CARGO_MANIFEST_DIR"),
                    language_folder(language),
                    report.file()
                );
                assert!(std::path::Path::new(&path).exists(), "{path}");
            }
        }
    }

    #[test]
    fn alarms_and_news_have_their_own_reports() {
        assert_eq!(Report::of(Sound::BaseAlert), Some(Report::BaseUnderAttack));
        assert_eq!(
            Report::of(Sound::HarvesterAlert),
            Some(Report::HarvesterUnderAttack)
        );
        assert_eq!(
            Report::of(Sound::ResearchComplete),
            Some(Report::ResearchComplete)
        );
        assert_eq!(Report::of(Sound::RifleShot), None);
    }
}
