use bevy::{audio::Volume, platform::collections::HashMap, prelude::*, window::PrimaryWindow};

use super::{campaign::*, campaign_save::CampaignStore};
use crate::state::GameState;

pub struct GameAudioPlugin;

#[derive(Event, Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Sound {
    Click,
    Back,
    Open,
    Confirm,
    Error,
    Notify,
    Tick,
    Chip,
    Win,
    Loss,
    Door,
    Knock,
    Bell,
    Bag,
    Cook,
    Cloth,
    Clean,
    Dog,
    Vibrate,
    Tv,
    Roulette,
    Whistle,
    Step,
}

const CLIPS: &[&str] = &[
    "click", "back", "open", "confirm", "error", "notify", "tick", "chip", "door", "bell", "bag",
    "cook", "cloth", "clean", "dog", "vibrate", "tv", "roulette", "whistle", "step1", "knock",
];

impl Sound {
    fn sample(self) -> (&'static str, f32, f32) {
        match self {
            Self::Click => ("click", 0.4, 1.0),
            Self::Back => ("back", 0.4, 1.0),
            Self::Open => ("open", 0.4, 1.0),
            Self::Confirm => ("confirm", 0.45, 1.0),
            Self::Error => ("error", 0.35, 0.85),
            Self::Notify => ("notify", 0.4, 1.0),
            Self::Tick => ("tick", 0.32, 1.0),
            Self::Chip => ("chip", 0.6, 1.0),
            Self::Win => ("confirm", 0.65, 1.15),
            Self::Loss => ("back", 0.5, 0.65),
            Self::Door => ("door", 0.6, 1.0),
            Self::Knock => ("knock", 0.65, 1.0),
            Self::Bell => ("bell", 0.45, 1.0),
            Self::Bag => ("bag", 0.5, 1.0),
            Self::Cook => ("cook", 0.5, 1.0),
            Self::Cloth => ("cloth", 0.45, 1.0),
            Self::Clean => ("clean", 0.5, 1.0),
            Self::Dog => ("dog", 0.45, 1.0),
            Self::Vibrate => ("vibrate", 0.45, 1.0),
            Self::Tv => ("tv", 0.3, 1.0),
            Self::Roulette => ("roulette", 0.4, 1.0),
            Self::Whistle => ("whistle", 0.25, 1.0),
            Self::Step => ("step1", 0.4, 1.0),
        }
    }

    pub(super) fn action(action: Action) -> Option<Self> {
        Some(match action {
            Action::Bet { .. } => Self::Chip,
            Action::EndBetSession => return None,
            #[cfg(test)]
            Action::Wait => Self::Back,
            Action::Work => Self::Door,
            Action::Groceries | Action::OrderFood => Self::Bag,
            Action::Cook => Self::Cook,
            Action::WatchTv => Self::Tv,
            Action::TimeWithPipoca => Self::Dog,
            Action::Clean => Self::Clean,
            Action::Call(_) | Action::CareConversation | Action::TherapySession => Self::Vibrate,
            Action::FinancialPlanning => Self::Notify,
            Action::ReceiveVisit(_) => Self::Door,
            Action::Sleep => Self::Cloth,
            Action::AcknowledgeReport => Self::Open,
            Action::Sell(HouseholdItem::Pipoca) => Self::Door,
            Action::Sell(_)
            | Action::PayBill(_)
            | Action::NegotiateRent
            | Action::SetDailyCap(_)
            | Action::BlockBetting => Self::Confirm,
        })
    }
}

#[derive(Resource)]
struct SoundAssets(HashMap<&'static str, Handle<AudioSource>>);

#[derive(Component)]
struct Ambience(bool);

impl Plugin for GameAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_observer(play)
            .add_systems(PostUpdate, environment);
    }
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(SoundAssets(
        CLIPS
            .iter()
            .map(|&name| (name, assets.load(format!("audio/{name}.wav"))))
            .collect(),
    ));
    for (name, fridge) in [("room", false), ("fridge", true)] {
        commands.spawn((
            Ambience(fridge),
            AudioPlayer::new(assets.load(format!("audio/{name}.wav"))),
            PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
        ));
    }
}

fn play(
    sound: On<Sound>,
    mut commands: Commands,
    assets: Res<SoundAssets>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    if !windows.single().is_ok_and(|w| w.focused) {
        return;
    }
    let (name, volume, speed) = sound.sample();
    commands.spawn((
        *sound.event(),
        AudioPlayer::new(assets.0[name].clone()),
        PlaybackSettings::DESPAWN
            .with_volume(Volume::Linear(volume))
            .with_speed(speed),
    ));
}

fn ambience_levels(campaign: Option<&Campaign>, state: GameState, focused: bool) -> (f32, f32) {
    if !focused || matches!(state, GameState::Menu | GameState::Settings) {
        return (0.0, 0.0);
    }
    if state == GameState::Title {
        return (0.06, 0.0);
    }
    let Some(c) = campaign else { return (0.0, 0.0) };
    if c.stage != CampaignStage::Playing {
        return (0.025, 0.0);
    }
    (
        0.10,
        if c.has_power() && c.household.owns(HouseholdItem::Fridge) {
            0.075
        } else {
            0.0
        },
    )
}

fn environment(
    mut commands: Commands,
    state: Res<State<GameState>>,
    store: Res<CampaignStore>,
    phone: Res<super::phone::Phone>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut ambience: Query<(&Ambience, &mut AudioSink)>,
    sounds: Query<(Entity, &Sound)>,
    mut last_period: Local<Option<(u8, Period)>>,
) {
    let focused = windows.single().is_ok_and(|w| w.focused);
    let campaign = store.campaign();
    let (room, fridge) = ambience_levels(campaign, *state.get(), focused);
    for (ambience, mut sink) in &mut ambience {
        sink.set_volume(Volume::Linear(if ambience.0 { fridge } else { room }));
    }
    for (entity, sound) in &sounds {
        let ui = matches!(
            sound,
            Sound::Click
                | Sound::Back
                | Sound::Open
                | Sound::Confirm
                | Sound::Error
                | Sound::Notify
        );
        if !focused
            || !ui && *state.get() != GameState::Playing
            || matches!(sound, Sound::Roulette | Sound::Tick) && !phone.animating()
        {
            commands.entity(entity).despawn();
        }
    }
    if let Some(c) = campaign {
        let period = (c.clock.day, c.clock.period());
        if last_period.is_some_and(|old| old != period)
            && *state.get() == GameState::Playing
            && focused
            && c.stage == CampaignStage::Playing
        {
            if c.commitment_due(Contact::Rafa) {
                commands.trigger(Sound::Bell);
            } else if c.commitment_due(Contact::Marina) {
                commands.trigger(Sound::Vibrate);
            }
        }
        *last_period = Some(period);
    } else {
        *last_period = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ambience_follows_focus_power_ownership_and_campaign_stage() {
        let mut c = Campaign::new(17);
        assert_eq!(
            ambience_levels(Some(&c), GameState::Playing, true),
            (0.10, 0.075)
        );
        for state in [GameState::Menu, GameState::Settings] {
            assert_eq!(ambience_levels(Some(&c), state, true), (0.0, 0.0));
        }
        assert_eq!(
            ambience_levels(Some(&c), GameState::Playing, false),
            (0.0, 0.0)
        );
        c.clock.day = 6;
        assert_eq!(ambience_levels(Some(&c), GameState::Playing, true).1, 0.0);
        c.clock.day = 1;
        c.attributes.stress = 70;
        c.apply(Action::Sell(HouseholdItem::Fridge)).unwrap();
        assert_eq!(ambience_levels(Some(&c), GameState::Playing, true).1, 0.0);
        c.stage = CampaignStage::Ended(Ending::FinalDay);
        assert_eq!(
            ambience_levels(Some(&c), GameState::Playing, true),
            (0.025, 0.0)
        );
    }

    #[test]
    fn audio_palette_is_present_and_session_exit_has_no_reward_sound() {
        for name in CLIPS
            .iter()
            .chain(["room", "fridge", "step1", "step2"].iter())
        {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("assets/audio/{name}.wav"));
            let bytes = std::fs::read(path).unwrap();
            assert_eq!(&bytes[..4], b"RIFF");
            assert!(bytes.len() > 100);
        }
        assert_eq!(Sound::action(Action::EndBetSession), None);
        assert_eq!(
            Sound::action(Action::Sell(HouseholdItem::Pipoca)),
            Some(Sound::Door)
        );
    }
}
