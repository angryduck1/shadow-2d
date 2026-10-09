//! «Тень Старшего Брата» — двумерная игра на движке Bevy.
//!
//! Модули:
//!   content — тексты из assets/dialogs.txt;
//!   game    — правила, не зависящие от движка;
//!   gallery — галерея развязок и сохранение открытых;
//!   paint   — рисование фигурами и компоненты анимации;
//!   scenes  — локации, развязки и анимации между ходами;
//!   hud     — панель показателей и диалог;
//!   audio   — саундтрек с плавной сменой треков.
//!
//! Запуск: `cargo run --release`. Тесты правил: `cargo test`.

mod audio;
mod content;
mod game;
mod gallery;
mod hud;
mod paint;
mod scenes;

use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

use bevy::prelude::*;
use bevy::window::WindowResolution;

use content::Catalog;
use game::{Choice, Cutscene, GameState};
use hud::{ChoiceButton, DIALOG_HEIGHT, PANEL_WIDTH};
use paint::{pal, CameraShake, Painter, SceneClock, SceneEntity};

const WINDOW: Vec2 = Vec2::new(1280.0, 720.0);

/// Шрифты вшиты в программу: стандартный шрифт движка не знает кириллицы.
const FONT_REGULAR: &[u8] = include_bytes!("../assets/DejaVuSans.ttf");
const FONT_BOLD: &[u8] = include_bytes!("../assets/DejaVuSans-Bold.ttf");

/// Экраны игры. Движок сам запускает нужные системы при переходах.
#[derive(States, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Screen {
    #[default]
    Title,
    Playing,
    Cutscene,
    /// Галерея развязок: открывается клавишей E.
    Gallery,
}

#[derive(Resource)]
pub struct Texts(pub Catalog);

#[derive(Resource)]
pub struct Fonts {
    pub regular: Handle<Font>,
    pub bold: Handle<Font>,
}

/// Текущая партия и всё, что ждёт показа.
#[derive(Resource, Default)]
pub struct Session {
    pub state: GameState,
    /// Сводка последнего хода для правой панели.
    pub report: String,
    /// Подпись под анимацией.
    pub caption: Option<String>,
    /// Анимации, которые ещё не показаны.
    queue: VecDeque<Cutscene>,
    /// Сколько секунд осталось текущей анимации.
    remaining: Option<f32>,
    /// Игрок листает досье: под чтение звучит музыка лора.
    pub reading: bool,
    /// Следующая страница досье.
    page: usize,
}

impl Session {
    /// Новая партия. Зерно случайных событий берётся из `SHADOW_SEED`
    /// или из часов, чтобы партии отличались.
    fn new_game() -> Self {
        let seed = std::env::var("SHADOW_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or_else(|| {
            SystemTime::now().duration_since(UNIX_EPOCH).map_or(1973, |d| d.subsec_nanos() ^ d.as_secs() as u32)
        });
        Self { state: GameState::with_seed(seed), ..default() }
    }

    /// Новая партия начинается с заставки первой книги.
    fn begin(&mut self, next: &mut NextState<Screen>) {
        *self = Session::new_game();
        self.queue = VecDeque::from([Cutscene::Chapter(1)]);
        next.set(Screen::Cutscene);
    }
}

/// Решение игрока: клавиша или кнопка.
#[derive(Event)]
struct ChoiceMade(Choice);

/// Набор параметров, нужных для рисования сцены.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Canvas<'w, 's> {
    commands: Commands<'w, 's>,
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<ColorMaterial>>,
    fonts: Res<'w, Fonts>,
    clock: ResMut<'w, SceneClock>,
    orbit: ResMut<'w, paint::OrbitClock>,
    old: Query<'w, 's, Entity, With<SceneEntity>>,
}

impl<'w, 's> Canvas<'w, 's> {
    /// Стирает прежнюю сцену и даёт инструмент для новой.
    pub fn begin<'a>(&'a mut self, origin: Vec2) -> Painter<'a, 'w, 's> {
        for entity in &self.old {
            self.commands.entity(entity).despawn();
        }
        self.clock.0 = 0.0;
        self.orbit.t = 0.0;
        self.commands.insert_resource(CameraShake::default());
        let fonts = (self.fonts.bold.clone(), self.fonts.regular.clone());
        Painter::new(&mut self.commands, &mut self.meshes, &mut self.materials, fonts, origin)
    }
}

/// Центр игровой сцены: окно минус панель справа и диалог снизу.
pub fn scene_origin() -> Vec2 {
    Vec2::new(-PANEL_WIDTH / 2.0, DIALOG_HEIGHT / 2.0)
}

/// Папка с файлами игры. Как и тексты, она ищется в папке запуска;
/// иначе движок ищет её рядом с программой (или в корне проекта при `cargo run`).
pub fn asset_root() -> std::path::PathBuf {
    let local = std::path::Path::new("assets");
    match std::fs::canonicalize(local) {
        Ok(path) if path.is_dir() => path,
        _ => bevy::asset::io::file::FileAssetReader::get_base_path().join(local),
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Тень Старшего Брата".into(),
                    resolution: WindowResolution::new(WINDOW.x, WINDOW.y),
                    resizable: false,
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin { file_path: asset_root().to_string_lossy().into_owned(), ..default() }),
    );

    // Шрифты нужны уже первой сцене, а она рисуется раньше систем запуска,
    // поэтому ресурс создаётся прямо здесь.
    let fonts = {
        let mut assets = app.world_mut().resource_mut::<Assets<Font>>();
        let mut load = |bytes: &[u8]| {
            assets.add(Font::try_from_bytes(bytes.to_vec()).expect("встроенный шрифт повреждён"))
        };
        Fonts { regular: load(FONT_REGULAR), bold: load(FONT_BOLD) }
    };

    app.insert_resource(fonts)
        .insert_resource(ClearColor(pal::NIGHT))
        .insert_resource(Texts(Catalog::load()))
        .init_resource::<Session>()
        .init_resource::<SceneClock>()
        .init_resource::<paint::OrbitClock>()
        .init_resource::<CameraShake>()
        .add_plugins(audio::MusicPlugin)
        .init_resource::<Autoplay>()
        .insert_resource(gallery::Gallery::load())
        .init_state::<Screen>()
        .add_event::<ChoiceMade>()
        .add_systems(Startup, (setup, hud::spawn_hud, spawn_tooltip).chain())
        .add_systems(OnEnter(Screen::Title), show_title)
        .add_systems(OnEnter(Screen::Playing), show_location)
        .add_systems(
            Update,
            (
                autoplay,
                shots,
                parade,
                start_game.run_if(in_state(Screen::Title)),
                (read_keys, read_buttons, apply_choice).chain().run_if(in_state(Screen::Playing)),
                run_cutscenes.run_if(in_state(Screen::Cutscene)),
                gallery::open_gallery,
                (gallery::gallery_input, gallery::show_gallery).chain().run_if(in_state(Screen::Gallery)),
                hud::update_hud,
                hud::highlight_buttons,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                paint::tick_clock,
                paint::orbit_warp_keys,
                paint::inspect_hover,
                paint::animate_twinkle,
                paint::animate_blink,
                paint::animate_motion,
                paint::animate_slide,
                paint::animate_orbit,
                paint::animate_flicker,
                paint::animate_sweep,
                paint::animate_reveal,
                paint::animate_grow,
                paint::animate_fade,
                paint::shake_camera,
            ),
        )
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}

// ═══════════════════════════════ ЗАСТАВКА ═══════════════════════════════

fn show_title(mut canvas: Canvas, texts: Res<Texts>) {
    let mut p = canvas.begin(Vec2::ZERO);
    scenes::draw_title(&mut p, &texts.0);
    p.text(0.0, 150.0, texts.0.get("ui.title"), 76.0, pal::TEXT, 10.0);
    p.text(0.0, 82.0, texts.0.get("ui.subtitle"), 22.0, pal::AMBER, 10.0);
    let hint = p.text(0.0, 24.0, texts.0.get("ui.start"), 20.0, pal::MUTED, 10.0);
    p.commands.entity(hint).insert(paint::Blink);
    p.text(0.0, -8.0, texts.0.get("ui.title.gallery"), 15.0, pal::MUTED, 10.0);
}

/// Всплывающая подсказка у курсора: одна на всю игру, прячется сама.
fn spawn_tooltip(mut commands: Commands, fonts: Res<Fonts>) {
    commands
        .spawn((
            paint::Tooltip,
            Node {
                position_type: PositionType::Absolute,
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                max_width: Val::Px(360.0),
                ..default()
            },
            BackgroundColor(pal::PANEL.with_alpha(0.94)),
            BorderColor(pal::USSR),
            GlobalZIndex(10),
            Visibility::Hidden,
        ))
        .with_children(|tip| {
            tip.spawn((
                paint::TooltipText,
                Text::new(""),
                TextFont { font: fonts.regular.clone(), font_size: 14.0, ..default() },
                TextColor(pal::TEXT),
            ));
        });
}

fn start_game(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut session: ResMut<Session>,
    mut next: ResMut<NextState<Screen>>,
) {
    if keys.any_just_pressed([KeyCode::Space, KeyCode::Enter]) || mouse.just_pressed(MouseButton::Left) {
        session.begin(&mut next);
    }
}

// ════════════════════════════════ ПАРТИЯ ════════════════════════════════

/// Рисует сцену текущего места. Срабатывает при каждом входе в игру.
fn show_location(mut canvas: Canvas, mut session: ResMut<Session>, texts: Res<Texts>) {
    session.caption = None;
    let mut p = canvas.begin(scene_origin());
    scenes::draw_location(&mut p, &session.state, &texts.0);
}

fn read_keys(
    keys: Res<ButtonInput<KeyCode>>,
    texts: Res<Texts>,
    mut session: ResMut<Session>,
    mut choices: EventWriter<ChoiceMade>,
    mut canvas: Canvas,
) {
    if keys.any_just_pressed([KeyCode::Digit1, KeyCode::Numpad1]) {
        choices.write(ChoiceMade(Choice::First));
    }
    if keys.any_just_pressed([KeyCode::Digit2, KeyCode::Numpad2]) {
        choices.write(ChoiceMade(Choice::Second));
    }
    if keys.just_pressed(KeyCode::KeyH) {
        session.report = texts.0.get("help").to_string();
    }
    if keys.just_pressed(KeyCode::KeyL) {
        open_dossier(&mut session, &texts.0);
    }
    if keys.just_pressed(KeyCode::KeyR) {
        *session = Session::new_game();
        let mut p = canvas.begin(scene_origin());
        scenes::draw_location(&mut p, &session.state, &texts.0);
    }
}

/// Показывает следующую открытую запись досье GLASS EYE в правой панели.
fn open_dossier(session: &mut Session, texts: &Catalog) {
    let entries = session.state.dossier();
    if entries.is_empty() {
        session.report = texts.get("ui.dossier.empty").to_string();
        return;
    }
    let index = session.page % entries.len();
    let key = entries[index];
    let header = texts
        .get("ui.dossier")
        .replace("{i}", &(index + 1).to_string())
        .replace("{n}", &entries.len().to_string());
    session.report = format!(
        "{header}\n\n{}\n{}",
        texts.get(&format!("lore.{key}.name")),
        texts.get(&format!("lore.{key}.text")).replace('\n', " ")
    );
    session.page = index + 1;
    session.reading = true;
}

fn read_buttons(
    buttons: Query<(&Interaction, &ChoiceButton), Changed<Interaction>>,
    mut choices: EventWriter<ChoiceMade>,
) {
    for (interaction, button) in &buttons {
        if *interaction == Interaction::Pressed {
            choices.write(ChoiceMade(button.0));
        }
    }
}

/// Передаёт решение правилам игры и решает, что показать дальше.
fn apply_choice(
    mut choices: EventReader<ChoiceMade>,
    texts: Res<Texts>,
    mut session: ResMut<Session>,
    mut endings: ResMut<gallery::Gallery>,
    mut next: ResMut<NextState<Screen>>,
    mut canvas: Canvas,
) {
    // За кадр принимается одно решение; лишние нажатия отбрасываются.
    let Some(ChoiceMade(choice)) = choices.read().last() else { return };
    if session.state.location.is_ending() {
        return;
    }
    let turn = session.state.apply(*choice, &texts.0);
    let ending = session.state.location;
    session.report = if ending.is_ending() {
        // Развязка: в панели эпилог, а сама развязка попадает в галерею.
        let fresh = endings.discover(ending);
        gallery::ending_report(&session.state, &turn.report, fresh, &texts.0)
    } else {
        turn.report
    };
    session.reading = false;
    session.queue = turn.cutscenes.into();

    if session.queue.is_empty() {
        let mut p = canvas.begin(scene_origin());
        scenes::draw_location(&mut p, &session.state, &texts.0);
    } else {
        next.set(Screen::Cutscene);
    }
}

// ═══════════════════════════════ АНИМАЦИИ ═══════════════════════════════

fn caption_for(cutscene: Cutscene, texts: &Catalog) -> String {
    match cutscene {
        Cutscene::Chapter(book) => texts.get(&format!("chapter.{book}.title")).to_string(),
        Cutscene::Abort(mission) => {
            format!("{}. {}", texts.get(&format!("{}.name", mission.key())), texts.get("cut.abort"))
        }
        Cutscene::N1Launch(number) => texts.get("cut.n1").replace("{n}", &number.to_string()),
        Cutscene::Launch(mission) => {
            format!("{}. {}", texts.get(&format!("{}.name", mission.key())), texts.get("cut.liftoff"))
        }
        Cutscene::Landing(mission) => {
            format!("{}. {}", texts.get(&format!("{}.name", mission.key())), texts.get("cut.descent"))
        }
        Cutscene::SovietModule(number) => texts.get("cut.docking").replace("{n}", &number.to_string()),
        Cutscene::Intercept => texts.get("cut.intercept").to_string(),
        Cutscene::SovietLanding => texts.get("cut.farside").to_string(),
        Cutscene::Strike => texts.get("cut.strike").to_string(),
        Cutscene::Crackdown => texts.get("cut.crackdown").to_string(),
        Cutscene::Flash => texts.get("cut.flash").to_string(),
    }
}

/// Показывает анимации по очереди. Пробел или щелчок пропускают текущую.
fn run_cutscenes(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    texts: Res<Texts>,
    mut session: ResMut<Session>,
    mut next: ResMut<NextState<Screen>>,
    mut canvas: Canvas,
) {
    let skip = keys.any_just_pressed([KeyCode::Space, KeyCode::Enter]) || mouse.just_pressed(MouseButton::Left);

    if let Some(remaining) = session.remaining {
        let left = remaining - time.delta_secs();
        if left > 0.0 && !skip {
            // `bypass_change_detection` не будит интерфейс каждый кадр.
            session.bypass_change_detection().remaining = Some(left);
            return;
        }
        session.remaining = None;
    }

    match session.queue.pop_front() {
        Some(cutscene) => {
            let mut p = canvas.begin(scene_origin());
            let duration = scenes::draw_cutscene(&mut p, cutscene, &session.state, &texts.0);
            session.caption = Some(caption_for(cutscene, &texts.0));
            session.remaining = Some(duration);
        }
        None => next.set(Screen::Playing),
    }
}

// ═══════════════════════════ ДЕМОНСТРАЦИЯ ═══════════════════════════════

/// Самостоятельное прохождение для проверки и показа.
///
/// Переменная окружения `SHADOW_AUTOPLAY` задаёт сценарий, например
/// `"s 1 2 1 1 s s"`: `s` — пробел, `1` и `2` — выбор, `w` — пауза,
/// `e` — открыть или закрыть галерею развязок, `v` — следующая развязка в ней.
/// Выбор выполняется после того, как закончатся анимации.
/// Каждый шаг выполняется через `SHADOW_AUTOPLAY_STEP` секунд (по умолчанию 1).
#[derive(Resource)]
struct Autoplay {
    script: VecDeque<char>,
    step: f32,
    wait: f32,
}

impl Default for Autoplay {
    fn default() -> Self {
        let script = std::env::var("SHADOW_AUTOPLAY").unwrap_or_default();
        let step = std::env::var("SHADOW_AUTOPLAY_STEP").ok().and_then(|s| s.parse().ok()).unwrap_or(1.0);
        Self { script: script.chars().filter(|c| !c.is_whitespace()).collect(), step, wait: step }
    }
}

/// Кадры для проверки сцен: `SHADOW_SHOTS=<папка>` сохраняет снимок окна
/// каждые `SHADOW_SHOTS_STEP` секунд (по умолчанию 2,5). Удобно вместе
/// с `SHADOW_AUTOPLAY`, когда нужно посмотреть партию без игрока.
fn shots(time: Res<Time>, mut commands: Commands, mut state: Local<Option<Option<(std::path::PathBuf, f32, f32, u32)>>>) {
    use bevy::render::view::screenshot::{save_to_disk, Screenshot};
    let Some((dir, step, wait, count)) = state.get_or_insert_with(|| {
        let dir = std::env::var_os("SHADOW_SHOTS").map(std::path::PathBuf::from)?;
        let step = std::env::var("SHADOW_SHOTS_STEP").ok().and_then(|s| s.parse().ok()).unwrap_or(2.5);
        Some((dir, step, step, 0))
    }) else {
        return;
    };
    *wait -= time.delta_secs();
    if *wait > 0.0 {
        return;
    }
    *wait = *step;
    *count += 1;
    let path = dir.join(format!("{count:03}.png"));
    commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
}

/// Парад сцен для проверки рисунков: `SHADOW_PARADE=<секунды>` вместо партии
/// показывает по очереди все кризисы, события досье и развязки. Вместе
/// с `SHADOW_SHOTS` даёт снимок каждой сцены.
fn parade(
    time: Res<Time>,
    texts: Res<Texts>,
    mut session: ResMut<Session>,
    mut next: ResMut<NextState<Screen>>,
    mut canvas: Canvas,
    mut state: Local<Option<Option<(f32, f32, usize)>>>,
) {
    use game::{Event, Location};
    let Some((step, wait, index)) = state.get_or_insert_with(|| {
        let step = std::env::var("SHADOW_PARADE").ok()?.parse().ok()?;
        Some((step, 1.0, 0))
    }) else {
        return;
    };
    *wait -= time.delta_secs();
    if *wait > 0.0 {
        return;
    }
    *wait = *step;
    let crises = [Location::Uprising, Location::Bankrupt, Location::MoonWar, Location::Brink, Location::Politburo];
    let scenes: Vec<Location> = crises
        .into_iter()
        .chain(Event::ALL.into_iter().map(Location::Event))
        .chain(Location::ENDINGS)
        .collect();
    let Some(&location) = scenes.get(*index) else { return };
    *index += 1;
    session.state = GameState::showcase(location);
    session.report = String::new();
    next.set(Screen::Playing);
    let mut p = canvas.begin(scene_origin());
    scenes::draw_location(&mut p, &session.state, &texts.0);
}

fn autoplay(
    time: Res<Time>,
    mut auto: ResMut<Autoplay>,
    screen: Res<State<Screen>>,
    mut session: ResMut<Session>,
    mut next: ResMut<NextState<Screen>>,
    mut choices: EventWriter<ChoiceMade>,
    mut endings: ResMut<gallery::Gallery>,
) {
    if auto.script.is_empty() {
        return;
    }
    auto.wait -= time.delta_secs();
    if auto.wait > 0.0 {
        return;
    }
    // Выбор ждёт, пока закончатся анимации: сценарий не теряет шаги.
    let choice = matches!(auto.script.front(), Some('1' | '2'));
    if choice && *screen.get() != Screen::Playing {
        return;
    }
    auto.wait = auto.step;
    match (auto.script.pop_front(), *screen.get()) {
        (Some('s'), Screen::Title) => session.begin(&mut next),
        (Some('s'), Screen::Cutscene) => session.remaining = Some(0.0),
        (Some('1'), Screen::Playing) => {
            choices.write(ChoiceMade(Choice::First));
        }
        (Some('2'), Screen::Playing) => {
            choices.write(ChoiceMade(Choice::Second));
        }
        (Some('e'), screen @ (Screen::Title | Screen::Playing)) => endings.enter(screen, &mut next),
        (Some('e'), Screen::Gallery) => endings.leave(&mut next),
        (Some('v'), Screen::Gallery) => endings.step(),
        _ => {}
    }
}
