//! Галерея развязок: все девятнадцать исходов, открытые и ещё нет.
//!
//! Открытые развязки хранятся между запусками в файле `endings.txt`
//! (по ключу на строку) в папке данных пользователя. Галерея показывает
//! сцену развязки, её текст и эпилог, а у неоткрытой — подсказку, как к ней
//! прийти.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::content::Catalog;
use crate::game::{GameState, Location};
use crate::hud::{outcome_color, reflow};
use crate::paint::pal;
use crate::{scene_origin, Canvas, Screen, Texts};

/// Открытые развязки и состояние экрана галереи.
#[derive(Resource)]
pub struct Gallery {
    /// Бит на развязку в порядке `Location::ENDINGS`.
    found: u32,
    selected: usize,
    /// Куда вернуться: на заставку или в партию.
    back: Screen,
    path: Option<PathBuf>,
}

impl Gallery {
    /// Читает сохранённый список. Нет файла — нет открытых развязок.
    pub fn load() -> Self {
        let path = save_path();
        let found = path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map_or(0, |source| parse(&source));
        Self { found, selected: 0, back: Screen::Title, path }
    }

    /// Открывает галерею и запоминает, куда вернуться.
    pub fn enter(&mut self, from: Screen, next: &mut NextState<Screen>) {
        self.back = from;
        next.set(Screen::Gallery);
    }

    pub fn leave(&self, next: &mut NextState<Screen>) {
        next.set(self.back);
    }

    /// Следующая развязка в списке.
    pub fn step(&mut self) {
        self.selected = (self.selected + 1) % Location::ENDINGS.len();
    }

    pub fn is_found(&self, ending: Location) -> bool {
        ending.ending_index().is_some_and(|i| self.found & 1 << i != 0)
    }

    pub fn count(&self) -> u32 {
        self.found.count_ones()
    }

    /// Отмечает развязку открытой и сохраняет список. Возвращает true,
    /// если она открыта впервые.
    pub fn discover(&mut self, ending: Location) -> bool {
        let Some(index) = ending.ending_index() else { return false };
        if self.found & 1 << index != 0 {
            return false;
        }
        self.found |= 1 << index;
        self.selected = index;
        self.save();
        true
    }

    fn save(&self) {
        let Some(path) = &self.path else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        // Сохранение не критично: если диск недоступен, игра идёт дальше.
        let _ = std::fs::write(path, serialize(self.found));
    }
}

/// Ключи развязок по строке, неизвестные строки пропускаются.
fn parse(source: &str) -> u32 {
    source
        .lines()
        .filter_map(|line| Location::ending_from_key(line.trim()))
        .filter_map(Location::ending_index)
        .fold(0, |mask, i| mask | 1 << i)
}

fn serialize(found: u32) -> String {
    Location::ENDINGS
        .iter()
        .enumerate()
        .filter(|(i, _)| found & 1 << i != 0)
        .map(|(_, ending)| format!("{}\n", ending.key()))
        .collect()
}

/// Папка данных пользователя: XDG на Linux, APPDATA на Windows,
/// Application Support на macOS.
fn save_path() -> Option<PathBuf> {
    let env = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    let base = if cfg!(windows) {
        env("APPDATA")
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|home| home.join("Library/Application Support"))
    } else {
        env("XDG_DATA_HOME").or_else(|| env("HOME").map(|home| home.join(".local/share")))
    };
    Some(base?.join("shadow-2d").join("endings.txt"))
}

/// Открывает галерею клавишей E с заставки или из партии.
pub fn open_gallery(
    keys: Res<ButtonInput<KeyCode>>,
    screen: Res<State<Screen>>,
    mut gallery: ResMut<Gallery>,
    mut next: ResMut<NextState<Screen>>,
) {
    let here = *screen.get();
    if keys.just_pressed(KeyCode::KeyE) && matches!(here, Screen::Title | Screen::Playing) {
        gallery.enter(here, &mut next);
    }
}

/// Стрелки и щелчок выбирают развязку, Esc или E возвращают назад.
pub fn gallery_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    mut gallery: ResMut<Gallery>,
    mut next: ResMut<NextState<Screen>>,
) {
    if keys.any_just_pressed([KeyCode::Escape, KeyCode::KeyE, KeyCode::Backspace]) {
        gallery.leave(&mut next);
        return;
    }
    let total = Location::ENDINGS.len();
    let mut selected = gallery.selected;
    if keys.any_just_pressed([KeyCode::ArrowDown, KeyCode::KeyS]) {
        selected = (selected + 1) % total;
    }
    if keys.any_just_pressed([KeyCode::ArrowUp, KeyCode::KeyW]) {
        selected = (selected + total - 1) % total;
    }
    if mouse.just_pressed(MouseButton::Left) {
        let cursor = windows.single().ok().and_then(|w| w.cursor_position());
        if let Some(row) = cursor.and_then(|c| row_at(Vec2::new(c.x - 640.0, 360.0 - c.y))) {
            selected = row;
        }
    }
    if selected != gallery.selected {
        gallery.selected = selected;
    }
}

/// Раскладка списка: правая полоса окна, строка на развязку.
const LIST_X: f32 = 460.0;
const LIST_TOP: f32 = 262.0;
const ROW: f32 = 29.0;

fn row_at(point: Vec2) -> Option<usize> {
    if point.x < LIST_X - 170.0 {
        return None;
    }
    let row = ((LIST_TOP + ROW / 2.0 - point.y) / ROW).floor();
    (row >= 0.0 && (row as usize) < Location::ENDINGS.len()).then_some(row as usize)
}

/// Перерисовывает галерею при входе и при смене выбора.
pub fn show_gallery(mut canvas: Canvas, gallery: Res<Gallery>, texts: Res<Texts>, screen: Res<State<Screen>>) {
    if !gallery.is_changed() && !screen.is_changed() {
        return;
    }
    let t = &texts.0;
    let ending = Location::ENDINGS[gallery.selected];
    let found = gallery.is_found(ending);

    // Сцена развязки в той же области, что и в партии. Неоткрытая — под вуалью.
    let mut p = canvas.begin(scene_origin());
    crate::scenes::draw_location(&mut p, &GameState::showcase(ending), t);
    if !found {
        p.rect(0.0, 0.0, 920.0, 490.0, pal::NIGHT.with_alpha(0.93), 55.0);
        p.text(0.0, 30.0, "?", 120.0, pal::LINE, 56.0);
        p.text(0.0, -70.0, t.get("ui.gallery.locked"), 20.0, pal::MUTED, 56.0);
    }

    // Правая полоса: список развязок. Слои выше всего, что рисует сцена.
    p.origin = Vec2::ZERO;
    p.rect(LIST_X, 0.0, 360.0, 720.0, pal::PANEL, 60.0);
    p.rect(LIST_X - 179.0, 0.0, 2.0, 720.0, pal::LINE, 60.1);
    p.label(LIST_X - 160.0, 330.0, t.get("ui.gallery.title"), 18.0, pal::TEXT, 61.0, Anchor::CenterLeft);
    let count = t
        .get("ui.gallery.count")
        .replace("{k}", &gallery.count().to_string())
        .replace("{n}", &Location::ENDINGS.len().to_string());
    p.label(LIST_X - 160.0, 302.0, &count, 14.0, pal::AMBER, 61.0, Anchor::CenterLeft);
    for (i, &item) in Location::ENDINGS.iter().enumerate() {
        let y = LIST_TOP - i as f32 * ROW;
        if i == gallery.selected {
            p.rect(LIST_X, y, 344.0, ROW - 3.0, pal::LINE, 60.5);
        }
        let open = gallery.is_found(item);
        let (name, color) = match item.outcome() {
            Some(outcome) if open => (t.get(&format!("title.{}", item.key())), outcome_color(outcome)),
            _ => (t.get("ui.gallery.hidden"), pal::MUTED),
        };
        p.label(LIST_X - 160.0, y, &format!("{:02}   {name}", i + 1), 15.0, color, 61.0, Anchor::CenterLeft);
    }

    // Нижняя полоса: исход, название и текст.
    p.rect(-180.0, -245.0, 920.0, 230.0, pal::PANEL, 60.0);
    p.rect(-180.0, -131.0, 920.0, 2.0, pal::LINE, 60.1);
    let outcome = ending.outcome().expect("в галерее только развязки");
    let heading = if found {
        format!("{} · {}", t.get(outcome.key()), ending.title(t))
    } else {
        ending.title(t).split(". ").next().unwrap_or_default().to_string()
    };
    let color = if found { outcome_color(outcome) } else { pal::MUTED };
    p.label(-616.0, -150.0, &heading, 18.0, color, 61.0, Anchor::CenterLeft);
    let body = if found {
        format!(
            "{}\n{}: {}",
            reflow(t.get(&format!("text.{}", ending.key()))),
            t.get("ui.epilogue"),
            reflow(t.get(&format!("epi.{}", ending.key())))
        )
    } else {
        format!("{} {}", t.get("ui.gallery.howto"), t.get(&format!("hint.{}", ending.key())))
    };
    p.paragraph(-616.0, -166.0, &body, 14.0, pal::TEXT, 61.0, 872.0);
    p.label(-616.0, -345.0, t.get("ui.gallery.keys"), 13.0, pal::MUTED, 61.0, Anchor::CenterLeft);
}

/// Текст для правой панели, когда партия дошла до развязки.
pub fn ending_report(state: &GameState, report: &str, fresh: bool, text: &Catalog) -> String {
    let mut out = report.to_string();
    if let Some(epilogue) = state.epilogue(text) {
        out.push_str(&format!("\n\n{}\n{}", text.get("ui.epilogue"), reflow(epilogue)));
    }
    if fresh {
        let name = text.get(&format!("title.{}", state.location.key()));
        out.push_str("\n\n");
        out.push_str(&text.get("ui.gallery.new").replace("{name}", name));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_file_round_trips() {
        let found = 1 << 0 | 1 << 7 | 1 << 18;
        assert_eq!(parse(&serialize(found)), found);
        assert_eq!(parse("мусор\n\nmars\n  armageddon  \n"), 1 | 1 << 18);
    }

    #[test]
    fn rows_map_to_endings() {
        assert_eq!(row_at(Vec2::new(LIST_X, LIST_TOP)), Some(0));
        assert_eq!(row_at(Vec2::new(LIST_X, LIST_TOP - ROW * 18.0)), Some(18));
        assert_eq!(row_at(Vec2::new(0.0, LIST_TOP)), None);
    }
}
