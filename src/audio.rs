//! Музыка: три зацикленных трека из саундтрека «ISOTOPE WARS — ARC 1 OST»
//! с плавной сменой при переходах между экранами.
//!
//! Файлы лежат в `assets/audio` (подробности в README той же папки):
//!   episode1.ogg — «Эпизод 1», 0:00–3:38: заставка и подготовка проекта;
//!   episode2.ogg — «Эпизод 2», 3:38–9:35: анимации, кризисы и развязки;
//!   lore.ogg     — «Lore Videos», 9:35–16:45: планирование миссий и досье.
//! Если файла нет, его трек молчит: игра работает и без музыки.
//!
//! Плеер каждого трека живёт всю игру. Неактивный трек затихает и встаёт
//! на паузу, а при возврате продолжает с того же места, а не с начала.

use bevy::audio::{AudioSinkPlayback, PlaybackSettings, Volume};
use bevy::prelude::*;

use crate::{asset_root, Screen, Session, Texts};

/// Сколько секунд длится смена трека.
const FADE_SECS: f32 = 1.6;
/// Громкость задаётся шагами по 10 %.
const VOLUME_STEPS: u8 = 10;
const DEFAULT_VOLUME: u8 = 6;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Track {
    Episode1,
    Episode2,
    Lore,
}

impl Track {
    const ALL: [Track; 3] = [Track::Episode1, Track::Episode2, Track::Lore];

    /// Путь внутри папки assets.
    fn path(self) -> &'static str {
        match self {
            Track::Episode1 => "audio/episode1.ogg",
            Track::Episode2 => "audio/episode2.ogg",
            Track::Lore => "audio/lore.ogg",
        }
    }
}

/// Громкость и трек, который должен звучать сейчас.
#[derive(Resource)]
pub struct Music {
    steps: u8,
    muted: bool,
    current: Track,
}

impl Default for Music {
    fn default() -> Self {
        Self { steps: DEFAULT_VOLUME, muted: false, current: Track::Episode1 }
    }
}

impl Music {
    fn master(&self) -> f32 {
        if self.muted {
            0.0
        } else {
            self.steps as f32 / VOLUME_STEPS as f32
        }
    }
}

/// Плеер одного трека и его текущая доля громкости от 0 до 1.
#[derive(Component)]
struct Channel {
    track: Track,
    level: f32,
}

pub struct MusicPlugin;

impl Plugin for MusicPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Music>()
            .add_systems(Startup, spawn_channels)
            .add_systems(Update, (choose_track, volume_keys, crossfade).chain());
    }
}

fn spawn_channels(mut commands: Commands, server: Res<AssetServer>) {
    let root = asset_root();
    for track in Track::ALL {
        if !root.join(track.path()).exists() {
            info!("музыка: нет файла {}, трек пропущен", root.join(track.path()).display());
            continue;
        }
        commands.spawn((
            AudioPlayer::new(server.load(track.path())),
            PlaybackSettings::LOOP.with_volume(Volume::SILENT).paused(),
            Channel { track, level: 0.0 },
        ));
    }
}

/// Выбирает трек по экрану и месту действия.
fn choose_track(screen: Res<State<Screen>>, session: Res<Session>, mut music: ResMut<Music>) {
    let location = session.state.location;
    let track = match screen.get() {
        Screen::Title => Track::Episode1,
        Screen::Gallery => Track::Lore,
        // Заставка первой книги идёт под ту же музыку, что и подготовка.
        Screen::Cutscene if location.is_preparation() => Track::Episode1,
        Screen::Cutscene => Track::Episode2,
        Screen::Playing if session.reading => Track::Lore,
        Screen::Playing if location.is_preparation() => Track::Episode1,
        Screen::Playing if location.is_climax() => Track::Episode2,
        Screen::Playing => Track::Lore,
    };
    if music.current != track {
        music.current = track;
    }
}

/// M — выключить звук, минус и плюс — громкость.
fn volume_keys(
    keys: Res<ButtonInput<KeyCode>>,
    texts: Res<Texts>,
    mut music: ResMut<Music>,
    mut session: ResMut<Session>,
) {
    let mut touched = false;
    if keys.any_just_pressed([KeyCode::Minus, KeyCode::NumpadSubtract]) {
        music.steps = music.steps.saturating_sub(1);
        music.muted = false;
        touched = true;
    }
    if keys.any_just_pressed([KeyCode::Equal, KeyCode::NumpadAdd]) {
        music.steps = (music.steps + 1).min(VOLUME_STEPS);
        music.muted = false;
        touched = true;
    }
    if keys.just_pressed(KeyCode::KeyM) {
        music.muted = !music.muted;
        touched = true;
    }
    if touched {
        session.report = if music.muted {
            texts.0.get("ui.muted").to_string()
        } else {
            let percent = music.steps as u32 * 100 / VOLUME_STEPS as u32;
            texts.0.get("ui.volume").replace("{n}", &percent.to_string())
        };
    }
}

/// Плавно поднимает активный трек и опускает остальные.
fn crossfade(time: Res<Time>, music: Res<Music>, mut channels: Query<(&mut Channel, Option<&mut AudioSink>)>) {
    let step = time.delta_secs() / FADE_SECS;
    let master = music.master();
    for (mut channel, sink) in &mut channels {
        let target = if channel.track == music.current { 1.0 } else { 0.0 };
        channel.level = approach(channel.level, target, step);
        // Плеер появляется, когда файл загружен; до этого просто ждём.
        let Some(mut sink) = sink else { continue };
        // Квадрат доли: на слух затухание идёт равномернее, чем по прямой.
        sink.set_volume(Volume::Linear(channel.level * channel.level * master));
        if channel.level <= 0.0 {
            if !sink.is_paused() {
                sink.pause();
            }
        } else if sink.is_paused() {
            sink.play();
        }
    }
}

fn approach(value: f32, target: f32, step: f32) -> f32 {
    if value < target {
        (value + step).min(target)
    } else {
        (value - step).max(target)
    }
}

#[cfg(test)]
mod tests {
    use super::approach;

    #[test]
    fn fade_reaches_target_without_overshoot() {
        assert_eq!(approach(0.0, 1.0, 0.4), 0.4);
        assert_eq!(approach(0.9, 1.0, 0.4), 1.0);
        assert_eq!(approach(0.3, 0.0, 0.4), 0.0);
    }
}
