//! Интерфейс поверх сцены: панель показателей справа и диалог снизу.

use bevy::prelude::*;

use crate::game::{Choice, Location, Outcome, War, DEFENCE_MODULE, SOVIET_MODULES};
use crate::paint::pal;
use crate::{Fonts, Screen, Session, Texts};

/// Ширина правой панели и высота нижнего диалога в пикселях.
pub const PANEL_WIDTH: f32 = 360.0;
pub const DIALOG_HEIGHT: f32 = 230.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Stat {
    Secrecy,
    Unrest,
    Tension,
    Base,
}

/// Какая надпись интерфейса находится в этом узле.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum Label {
    Date,
    Rockets,
    Station,
    Budget,
    Power,
    Moscow,
    Value(Stat),
    Title,
    Report,
    Body,
    Option(Choice),
    Hint,
}

#[derive(Component)]
pub struct BarFill(Stat);

#[derive(Component)]
pub struct ChoiceButton(pub Choice);

#[derive(Component)]
pub struct HudRoot;

fn text(fonts: &Fonts, bold: bool, size: f32, color: Color, content: &str) -> impl Bundle {
    let font = if bold { fonts.bold.clone() } else { fonts.regular.clone() };
    (Text::new(content), TextFont { font, font_size: size, ..default() }, TextColor(color))
}

/// Строит интерфейс один раз при запуске. Дальше меняется только текст.
pub fn spawn_hud(mut commands: Commands, fonts: Res<Fonts>, texts: Res<Texts>) {
    let t = &texts.0;

    // ── Правая панель: показатели и сводка ──
    commands
        .spawn((
            HudRoot,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(PANEL_WIDTH),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(18.0)),
                row_gap: Val::Px(6.0),
                border: UiRect::left(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(pal::PANEL),
            BorderColor(pal::LINE),
        ))
        .with_children(|panel| {
            panel.spawn(text(&fonts, true, 13.0, pal::MUTED, "GLASS EYE // ДОСТУП ОГРАНИЧЕН"));
            panel.spawn((text(&fonts, true, 26.0, pal::TEXT, ""), Label::Date));

            for (key, label, color) in [
                ("ui.rockets", Label::Rockets, pal::US),
                ("ui.station", Label::Station, pal::USSR),
                ("ui.budget", Label::Budget, pal::GREEN),
                ("ui.power", Label::Power, pal::TEXT),
                ("ui.moscow", Label::Moscow, pal::TEXT),
            ] {
                panel
                    .spawn(Node { justify_content: JustifyContent::SpaceBetween, ..default() })
                    .with_children(|row| {
                        row.spawn(text(&fonts, false, 15.0, pal::MUTED, t.get(key)));
                        row.spawn((text(&fonts, true, 17.0, color, ""), label));
                    });
            }

            for (key, stat) in [
                ("ui.secrecy", Stat::Secrecy),
                ("ui.unrest", Stat::Unrest),
                ("ui.tension", Stat::Tension),
                ("ui.base", Stat::Base),
            ] {
                panel
                    .spawn(Node {
                        justify_content: JustifyContent::SpaceBetween,
                        margin: UiRect::top(Val::Px(6.0)),
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn(text(&fonts, false, 15.0, pal::MUTED, t.get(key)));
                        row.spawn((text(&fonts, true, 15.0, pal::TEXT, ""), Label::Value(stat)));
                    });
                panel
                    .spawn((
                        Node { width: Val::Percent(100.0), height: Val::Px(12.0), ..default() },
                        BackgroundColor(pal::LINE),
                    ))
                    .with_children(|track| {
                        track.spawn((
                            Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() },
                            BackgroundColor(pal::GREEN),
                            BarFill(stat),
                        ));
                    });
            }

            panel.spawn((
                Node { height: Val::Px(2.0), margin: UiRect::vertical(Val::Px(10.0)), ..default() },
                BackgroundColor(pal::LINE),
            ));
            panel.spawn((text(&fonts, false, 15.0, pal::AMBER, ""), Label::Report));
        });

    // ── Нижний диалог: описание сцены и два варианта ──
    commands
        .spawn((
            HudRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                bottom: Val::Px(0.0),
                right: Val::Px(PANEL_WIDTH),
                height: Val::Px(DIALOG_HEIGHT),
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(Val::Px(24.0), Val::Px(14.0)),
                row_gap: Val::Px(8.0),
                border: UiRect::top(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(pal::PANEL),
            BorderColor(pal::LINE),
        ))
        .with_children(|dialog| {
            dialog.spawn((text(&fonts, true, 18.0, pal::US, ""), Label::Title));
            dialog.spawn((
                text(&fonts, false, 16.0, pal::TEXT, ""),
                Label::Body,
                Node { flex_grow: 1.0, ..default() },
            ));
            for choice in [Choice::First, Choice::Second] {
                dialog
                    .spawn((
                        Button,
                        ChoiceButton(choice),
                        Node {
                            padding: UiRect::axes(Val::Px(12.0), Val::Px(5.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(pal::NIGHT),
                        BorderColor(pal::LINE),
                    ))
                    .with_children(|button| {
                        button.spawn((text(&fonts, false, 16.0, pal::GREEN, ""), Label::Option(choice)));
                    });
            }
            dialog.spawn((text(&fonts, false, 13.0, pal::MUTED, ""), Label::Hint));
        });
}

/// Переписывает надписи и шкалы, когда меняется партия или экран.
#[allow(clippy::type_complexity)]
pub fn update_hud(
    session: Res<Session>,
    texts: Res<Texts>,
    screen: Res<State<Screen>>,
    mut labels: Query<(&Label, &mut Text, &mut TextColor)>,
    mut bars: Query<(&BarFill, &mut Node, &mut BackgroundColor), Without<ChoiceButton>>,
    mut roots: Query<&mut Visibility, (With<HudRoot>, Without<ChoiceButton>)>,
    mut buttons: Query<(&mut Visibility, &mut Node), (With<ChoiceButton>, Without<HudRoot>)>,
) {
    if !session.is_changed() && !screen.is_changed() {
        return;
    }
    let (state, t) = (&session.state, &texts.0);
    let playing = *screen.get() == Screen::Playing;

    let shown = !matches!(screen.get(), Screen::Title | Screen::Gallery);
    for mut visibility in &mut roots {
        *visibility = if shown { Visibility::Inherited } else { Visibility::Hidden };
    }
    let options = if playing { state.options(t) } else { None };
    // Без вариантов кнопки убираются из раскладки: тексту развязки нужно место.
    for (mut visibility, mut node) in &mut buttons {
        let (seen, display) = if options.is_some() { (Visibility::Inherited, Display::Flex) } else { (Visibility::Hidden, Display::None) };
        *visibility = seen;
        node.display = display;
    }

    let value = |stat: Stat| match stat {
        Stat::Secrecy => state.secrecy,
        Stat::Unrest => state.unrest,
        Stat::Tension => state.tension,
        Stat::Base => state.base,
    };

    for (label, mut text, mut color) in &mut labels {
        let content = match *label {
            Label::Date => state.date(),
            Label::Rockets => {
                // ▲ — ракета на заводе, △ — уже запущена.
                let left = state.saturn_v as usize;
                let built = state.saturn_built as usize;
                format!("{}{}", "▲ ".repeat(left), "△ ".repeat(built - left))
            }
            Label::Station if state.war == War::Won => "✕ ".repeat(state.soviet_modules as usize),
            Label::Station => {
                // ◆ — оборонный модуль, ■ — остальные, □ — ещё не выведены.
                (1..=SOVIET_MODULES)
                    .map(|n| match n {
                        n if n > state.soviet_modules => "□ ",
                        DEFENCE_MODULE => "◆ ",
                        _ => "■ ",
                    })
                    .collect()
            }
            Label::Budget => state.budget.to_string(),
            Label::Power if state.autocracy => t.get("ui.power.junta").to_string(),
            Label::Power => t.get("ui.power.republic").to_string(),
            Label::Moscow => match (state.location, state.war) {
                (Location::MoonWar | Location::Brink, _) => t.get("ui.moscow.war").to_string(),
                (_, War::Won) => t.get("ui.moscow.won").to_string(),
                (_, War::Truce) => t.get("ui.moscow.truce").to_string(),
                _ => {
                    let dots: String = (0..2).map(|i| if i < state.detente { '●' } else { '○' }).collect();
                    t.get("ui.moscow.detente").replace("{dots}", &dots)
                }
            },
            Label::Value(stat) => value(stat).to_string(),
            Label::Title => match &session.caption {
                Some(caption) if !playing => caption.clone(),
                _ => state.title(t),
            },
            Label::Body if playing => reflow(&state.description(t)),
            Label::Body => String::new(),
            Label::Report => session.report.clone(),
            Label::Option(choice) => {
                let index = if choice == Choice::First { 0 } else { 1 };
                options.as_ref().map(|o| format!("{}.  {}", index + 1, o[index])).unwrap_or_default()
            }
            Label::Hint if !playing => t.get("ui.skip").to_string(),
            Label::Hint if state.location.is_ending() => t.get("ui.restart").to_string(),
            Label::Hint => t.get("ui.keys").to_string(),
        };
        match *label {
            // Заголовок развязки окрашен по её исходу.
            Label::Title => {
                color.0 = match state.location.outcome() {
                    Some(outcome) if playing => outcome_color(outcome),
                    _ => pal::US,
                };
            }
            Label::Power => color.0 = if state.autocracy { pal::USSR } else { pal::TEXT },
            Label::Moscow => {
                color.0 = match state.war {
                    _ if matches!(state.location, Location::MoonWar | Location::Brink) => pal::USSR,
                    War::Won => pal::AMBER,
                    _ => pal::TEXT,
                }
            }
            _ => {}
        }
        if text.0 != content {
            text.0 = content;
        }
    }

    for (bar, mut node, mut background) in &mut bars {
        let amount = value(bar.0).clamp(0, 100);
        node.width = Val::Percent(amount as f32);
        // Для недовольства и напряжённости опасен рост, для остальных — падение.
        let danger = match bar.0 {
            Stat::Unrest | Stat::Tension => amount,
            Stat::Secrecy | Stat::Base => 100 - amount,
        };
        background.0 = match danger {
            d if d >= 75 => pal::USSR,
            d if d >= 50 => pal::AMBER,
            _ => pal::GREEN,
        };
    }
}

/// Цвет исхода: победа, пиррова победа, поражение.
pub fn outcome_color(outcome: Outcome) -> Color {
    match outcome {
        Outcome::Victory => pal::GREEN,
        Outcome::Pyrrhic => pal::AMBER,
        Outcome::Defeat => pal::USSR,
    }
}

/// Подсветка кнопки под курсором.
pub fn highlight_buttons(
    mut buttons: Query<(&Interaction, &mut BackgroundColor, &mut BorderColor), (Changed<Interaction>, With<ChoiceButton>)>,
) {
    for (interaction, mut background, mut border) in &mut buttons {
        let hovered = *interaction != Interaction::None;
        background.0 = if hovered { pal::LINE } else { pal::NIGHT };
        border.0 = if hovered { pal::GREEN } else { pal::LINE };
    }
}

/// Склеивает строки одного предложения: в файле диалогов они разбиты
/// для удобства чтения, а в окне текст переносится сам. Разрыв остаётся
/// после законченного предложения и перед новой репликой.
pub fn reflow(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        let sentence_ended = out.ends_with(['.', '!', '?', ':']);
        if out.is_empty() {
        } else if sentence_ended || line.starts_with('—') {
            out.push('\n');
        } else {
            out.push(' ');
        }
        out.push_str(line.trim());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::reflow;

    #[test]
    fn reflow_joins_broken_sentences() {
        assert_eq!(reflow("Раз два\nтри. Четыре\nпять.\nШесть."), "Раз два три. Четыре пять.\nШесть.");
        assert_eq!(reflow("Он сказал\n— Да."), "Он сказал\n— Да.");
    }
}
