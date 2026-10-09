//! Сцены игры: локации, развязки и анимации между ходами.
//!
//! Начало координат сцены — её центр. Размер видимой области 920×490.
//!
//! Правила анимаций, чтобы игрок их читал:
//! - каждая сцена проявляется из черноты, а анимация в неё и уходит;
//! - путь корабля показан пунктиром заранее, цель отмечена рамкой;
//! - ключевой момент (отрыв, касание, стыковка, попадание) подтверждён
//!   надписью, вспышкой или тряской камеры;
//! - движения заканчиваются точно в заданный момент (`Slide`), поэтому
//!   пыль, вспышки и надписи привязаны к нему без подгонки.

use bevy::prelude::*;

use crate::content::Catalog;
use crate::game::{Cutscene, Event, GameState, Location, Mission, War, DEFENCE_MODULE, SATURN_LIMIT};
use crate::paint::*;

/// Половина ширины и высоты сцены.
pub const HW: f32 = 460.0;
pub const HH: f32 = 245.0;
/// Линия лунного горизонта.
const GROUND: f32 = -95.0;

fn v(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

/// Длина затемнения на краях сцены, секунды.
const FADE: f32 = 0.35;
/// Слой затемнения и кинорамки: выше всего, что рисует сцена.
const Z_CURTAIN: f32 = 50.0;
const Z_LETTERBOX: f32 = 45.0;
const Z_LIGHT: f32 = 30.0;

// ═════════════════════════ ПЕРЕХОДЫ И ПОДСКАЗКИ ═════════════════════════

/// Сцена проявляется из черноты.
fn fade_in(p: &mut Painter) {
    let curtain = p.rect(0.0, 0.0, HW * 2.0, HH * 2.0, Color::BLACK, Z_CURTAIN);
    p.commands.entity(curtain).insert(Fade::keys(&[(0.0, 1.0), (FADE, 0.0)]));
}

/// Анимация проявляется в начале и уходит в черноту к концу `duration`.
fn curtain(p: &mut Painter, duration: f32) {
    let curtain = p.rect(0.0, 0.0, HW * 2.0, HH * 2.0, Color::BLACK, Z_CURTAIN);
    p.commands.entity(curtain).insert(Fade::keys(&[
        (0.0, 1.0),
        (FADE, 0.0),
        (duration - FADE, 0.0),
        (duration, 1.0),
    ]));
}

/// Кинорамка: чёрные полосы въезжают сверху и снизу.
fn letterbox(p: &mut Painter, height: f32) {
    for side in [-1.0, 1.0] {
        let bar = p.rect(0.0, side * (HH + height / 2.0), HW * 2.0, height, Color::BLACK, Z_LETTERBOX);
        p.commands.entity(bar).insert(Slide::new(v(0.0, -side * height), 0.6, 0.0));
    }
}

/// Цветной свет на всю сцену: зарево старта, вспышка взрыва.
fn light(p: &mut Painter, color: Color, keys: &[(f32, f32)]) {
    let glow = p.rect(0.0, 0.0, HW * 2.0, HH * 2.0, color, Z_LIGHT);
    p.commands.entity(glow).insert(Fade::keys(keys));
}

/// Пунктир по ломаной: так игрок заранее видит, куда полетит корабль.
fn dashes(p: &mut Painter, points: &[Vec2], color: Color, z: f32) -> Group {
    p.group(|p| {
        for pair in points.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let steps = (a.distance(b) / 12.0).max(1.0) as i32;
            for i in (0..steps).step_by(2) {
                let (t0, t1) = (i as f32 / steps as f32, (i + 1) as f32 / steps as f32);
                p.line(a.lerp(b, t0), a.lerp(b, t1), 2.0, color, z);
            }
        }
    })
}

/// Рамка цели с мигающими уголками и подписью. Видна до момента `until`.
fn target_mark(p: &mut Painter, x: f32, y: f32, color: Color, label: &str, until: f32) {
    let mark = p.group(|p| {
        for (dx, dy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            let (cx, cy) = (x + dx * 22.0, y + dy * 16.0);
            let a = p.rect(cx - dx * 5.0, cy, 12.0, 2.0, color, 9.5);
            let b = p.rect(cx, cy - dy * 5.0, 2.0, 12.0, color, 9.5);
            p.commands.entity(a).insert(Twinkle(0.0));
            p.commands.entity(b).insert(Twinkle(0.0));
        }
        if !label.is_empty() {
            let text = p.text(x, y + 32.0, label, 13.0, color, 9.5);
            p.commands.entity(text).insert(Blink);
        }
    });
    p.tag(&mark, Reveal { from: 0.0, to: until });
}

/// Надпись, которая проявляется в `from` и гаснет к `to`.
fn flash_label(p: &mut Painter, x: f32, y: f32, text: &str, size: f32, color: Color, from: f32, to: f32) {
    let label = p.text(x, y, text, size, color, 40.0);
    p.commands.entity(label).insert((Fade::window(from, to, 0.2), TextLayout::new_with_justify(JustifyText::Center)));
}

/// Клубы дыма у земли: расходятся в стороны и тают.
fn ground_smoke(p: &mut Painter, x: f32, y: f32, from: f32, count: usize, spread: f32, seed: u32) {
    let mut dice = Dice(seed);
    for i in 0..count {
        let side = if i % 2 == 0 { -1.0 } else { 1.0 };
        let start = from + i as f32 * 0.06;
        let puff = p.circle(x + side * dice.range(6.0, 30.0), y, dice.range(10.0, 18.0), pal::STEEL.with_alpha(0.6), 9.0);
        p.commands.entity(puff).insert((
            Reveal { from: start, to: 99.0 },
            Motion { vel: v(side * dice.range(0.4, 1.0) * spread, dice.range(4.0, 26.0)), acc: v(0.0, 6.0), delay: start },
            Grow(0.5),
            Fade::keys(&[(start + 2.0, 1.0), (start + 4.0, 0.0)]),
        ));
    }
}

/// Дымный след за ракетой, которая взлетает с ускорением `acc` с момента
/// `lift`. Клубы ставятся туда, где в этот момент будут сопла.
fn exhaust_trail(p: &mut Painter, x: f32, base: f32, lift: f32, acc: f32, until: f32, seed: u32) {
    let mut dice = Dice(seed);
    let mut t = lift + 0.12;
    while t < until {
        let y = base + 0.5 * acc * (t - lift).powi(2);
        if y > HH + 30.0 {
            break;
        }
        let puff = p.circle(x + dice.range(-5.0, 5.0), y - 6.0, dice.range(7.0, 12.0), pal::STEEL.with_alpha(0.5), 6.5);
        p.commands.entity(puff).insert((
            Reveal { from: t, to: 99.0 },
            Grow(0.35),
            Fade::keys(&[(t, 1.0), (t + 2.2, 0.0)]),
        ));
        t += 0.07;
    }
}

// ═══════════════════════════ КИРПИЧИКИ СЦЕН ═════════════════════════════

/// Фон и мерцающие звёзды выше линии `floor`.
fn sky(p: &mut Painter, color: Color, stars: usize, floor: f32, seed: u32) {
    p.rect(0.0, 0.0, HW * 2.0, HH * 2.0, color, 0.0);
    let mut dice = Dice(seed);
    for _ in 0..stars {
        let (x, y) = (dice.range(-HW, HW), dice.range(floor, HH));
        let size = if dice.roll() > 0.85 { 3.0 } else { 2.0 };
        let star = p.rect(x, y, size, size, pal::WHITE, 1.0);
        p.commands.entity(star).insert(Twinkle(dice.range(0.0, std::f32::consts::TAU)));
    }
}

fn earth(p: &mut Painter, x: f32, y: f32, r: f32) {
    p.circle(x, y, r * 1.08, pal::EARTH.with_alpha(0.18), 2.0);
    p.circle(x, y, r, pal::EARTH, 2.1);
    p.ellipse(x - r * 0.25, y + r * 0.25, r * 0.38, r * 0.28, pal::LAND, 2.2);
    p.ellipse(x + r * 0.30, y - r * 0.15, r * 0.25, r * 0.36, pal::LAND, 2.2);
    p.ellipse(x - r * 0.05, y - r * 0.55, r * 0.30, r * 0.12, pal::WHITE.with_alpha(0.8), 2.3);
    p.ellipse(x + r * 0.15, y + r * 0.60, r * 0.40, r * 0.10, pal::WHITE.with_alpha(0.8), 2.3);
    // Тень превращает диск в серп.
    p.circle(x + r * 0.45, y - r * 0.10, r * 0.98, pal::NIGHT.with_alpha(0.72), 2.4);
}

/// Маленький диск Луны, как он виден с Земли.
fn far_moon(p: &mut Painter, x: f32, y: f32, r: f32) {
    p.circle(x, y, r, pal::MOON_LIGHT, 2.0);
    p.circle(x - r * 0.3, y + r * 0.25, r * 0.22, pal::MOON, 2.1);
    p.circle(x + r * 0.25, y - r * 0.30, r * 0.30, pal::MOON, 2.1);
    p.circle(x + r * 0.35, y + r * 0.35, r * 0.12, pal::MOON, 2.1);
}

/// Лунная поверхность с холмами и кратерами.
fn moon_ground(p: &mut Painter, tint: Color, seed: u32) {
    let depth = HH + GROUND;
    p.ellipse(-250.0, GROUND - 10.0, 300.0, 50.0, pal::MOON_DARK, 3.0);
    p.ellipse(260.0, GROUND - 5.0, 340.0, 42.0, pal::MOON_DARK, 3.0);
    p.rect(0.0, GROUND - depth / 2.0, HW * 2.0, depth, tint, 4.0);
    p.rect(0.0, GROUND - 2.0, HW * 2.0, 4.0, pal::MOON_LIGHT, 4.1);
    let mut dice = Dice(seed);
    for _ in 0..9 {
        let (x, y) = (dice.range(-HW, HW), dice.range(-HH + 12.0, GROUND - 28.0));
        let size = dice.range(14.0, 46.0);
        p.ellipse(x, y, size, size * 0.28, pal::MOON_LIGHT, 4.2);
        p.ellipse(x, y - 2.0, size * 0.86, size * 0.22, pal::MOON_DARK, 4.3);
    }
}

/// Ракета Saturn V. Точка (x, y) — основание.
fn rocket(p: &mut Painter, x: f32, y: f32, s: f32) {
    let z = 8.0;
    // Первая ступень со стабилизаторами.
    p.tri(x, y, [(-26.0 * s, 0.0), (-12.0 * s, 0.0), (-12.0 * s, 34.0 * s)], pal::STEEL, z);
    p.tri(x, y, [(26.0 * s, 0.0), (12.0 * s, 0.0), (12.0 * s, 34.0 * s)], pal::STEEL, z);
    p.rect(x, y + 45.0 * s, 26.0 * s, 90.0 * s, pal::WHITE, z + 0.1);
    p.rect(x - 6.5 * s, y + 20.0 * s, 13.0 * s, 26.0 * s, pal::DARK, z + 0.2);
    p.rect(x + 6.5 * s, y + 66.0 * s, 13.0 * s, 22.0 * s, pal::DARK, z + 0.2);
    p.rect(x, y + 91.0 * s, 26.0 * s, 4.0 * s, pal::DARK, z + 0.2);
    // Вторая ступень.
    p.rect(x, y + 121.0 * s, 26.0 * s, 56.0 * s, pal::WHITE, z + 0.1);
    p.rect(x, y + 150.0 * s, 26.0 * s, 4.0 * s, pal::DARK, z + 0.2);
    // Переходник и третья ступень.
    p.tri(x, y + 152.0 * s, [(-13.0 * s, 0.0), (13.0 * s, 0.0), (-9.0 * s, 12.0 * s)], pal::WHITE, z + 0.1);
    p.tri(x, y + 152.0 * s, [(13.0 * s, 0.0), (9.0 * s, 12.0 * s), (-9.0 * s, 12.0 * s)], pal::WHITE, z + 0.1);
    p.rect(x, y + 182.0 * s, 18.0 * s, 36.0 * s, pal::WHITE, z + 0.1);
    p.rect(x, y + 176.0 * s, 18.0 * s, 4.0 * s, pal::DARK, z + 0.2);
    // Корабль и башня спасения.
    p.rect(x, y + 207.0 * s, 12.0 * s, 14.0 * s, pal::STEEL, z + 0.1);
    p.tri(x, y + 214.0 * s, [(-6.0 * s, 0.0), (6.0 * s, 0.0), (0.0, 12.0 * s)], pal::WHITE, z + 0.1);
    p.rect(x, y + 233.0 * s, 2.0 * s, 16.0 * s, pal::STEEL, z + 0.1);
    p.text(x, y + 118.0 * s, "U\nS\nA", 9.0 * s, pal::USSR, z + 0.3);
}

/// Пламя под точкой (x, y). Возвращает фигуры, чтобы скрывать их до старта.
fn flame(p: &mut Painter, x: f32, y: f32, s: f32) -> Group {
    p.group(|p| {
        let outer = p.tri(x, y, [(-14.0 * s, 0.0), (14.0 * s, 0.0), (0.0, -70.0 * s)], pal::FIRE, 7.0);
        let inner = p.tri(x, y, [(-8.0 * s, 0.0), (8.0 * s, 0.0), (0.0, -42.0 * s)], pal::AMBER, 7.1);
        let core = p.tri(x, y, [(-4.0 * s, 0.0), (4.0 * s, 0.0), (0.0, -22.0 * s)], pal::WHITE, 7.2);
        p.commands.entity(outer).insert(Flicker::new(0.0));
        p.commands.entity(inner).insert(Flicker::new(1.7));
        p.commands.entity(core).insert(Flicker::new(3.1));
    })
}

/// Башня обслуживания. Точка (x, y) — основание.
fn tower(p: &mut Painter, x: f32, y: f32, h: f32) {
    let red = Color::srgb(0.72, 0.22, 0.18);
    p.rect(x - 14.0, y + h / 2.0, 4.0, h, red, 6.0);
    p.rect(x + 14.0, y + h / 2.0, 4.0, h, red, 6.0);
    let steps = (h / 28.0) as i32;
    for i in 0..steps {
        let base = y + i as f32 * 28.0;
        p.line(v(x - 14.0, base), v(x + 14.0, base + 28.0), 2.0, red, 6.0);
        p.line(v(x + 14.0, base), v(x - 14.0, base + 28.0), 2.0, red, 6.0);
        p.rect(x, base, 28.0, 2.0, red, 6.0);
    }
    for arm in [0.35, 0.6, 0.85] {
        p.rect(x - 34.0, y + h * arm, 44.0, 4.0, pal::STEEL, 6.1);
    }
    p.rect(x, y + h + 14.0, 3.0, 28.0, pal::STEEL, 6.0);
    let lamp = p.rect(x, y + h + 30.0, 5.0, 5.0, pal::USSR, 6.2);
    p.commands.entity(lamp).insert(Twinkle(0.0));
}

/// Лунный корабль. Точка (x, y) — уровень опор. `hull` задаёт цвет стороны.
fn lander(p: &mut Painter, x: f32, y: f32, s: f32, hull: Color, crew: u8) {
    let z = 8.0;
    for side in [-1.0, 1.0] {
        p.line(v(x + side * 16.0 * s, y + 22.0 * s), v(x + side * 34.0 * s, y), 2.5 * s, pal::STEEL, z);
        p.rect(x + side * 34.0 * s, y, 12.0 * s, 3.0 * s, pal::STEEL, z);
    }
    p.tri(x, y + 12.0 * s, [(-7.0 * s, 0.0), (7.0 * s, 0.0), (0.0, 8.0 * s)], pal::DARK, z);
    p.rect(x, y + 24.0 * s, 40.0 * s, 18.0 * s, pal::GOLD, z + 0.1);
    p.rect(x, y + 44.0 * s, 28.0 * s, 22.0 * s, hull, z + 0.1);
    p.rect(x, y + 58.0 * s, 8.0 * s, 6.0 * s, pal::STEEL, z + 0.1);
    for i in 0..crew {
        let offset = (i as f32 - (crew as f32 - 1.0) / 2.0) * 7.0 * s;
        p.rect(x + offset, y + 46.0 * s, 4.0 * s, 4.0 * s, pal::DARK, z + 0.2);
    }
}

fn astronaut(p: &mut Painter, x: f32, y: f32, suit: Color) {
    p.rect(x, y + 9.0, 7.0, 10.0, suit, 9.0);
    p.circle(x, y + 17.0, 4.0, suit, 9.0);
    p.rect(x + 1.0, y + 17.0, 4.0, 3.0, pal::GOLD, 9.1);
    p.rect(x - 2.0, y + 2.0, 2.5, 5.0, suit, 9.0);
    p.rect(x + 2.0, y + 2.0, 2.5, 5.0, suit, 9.0);
}

fn flag(p: &mut Painter, x: f32, y: f32, color: Color) {
    p.rect(x, y + 16.0, 2.0, 32.0, pal::STEEL, 9.0);
    p.rect(x + 9.0, y + 26.0, 16.0, 10.0, color, 9.0);
}

/// Модули базы «Тихо» в порядке доставки.
fn base_module(p: &mut Painter, mission: Mission) {
    let g = GROUND;
    match mission {
        Mission::Apollo18 => {
            // SABATIER-L: печь и бак с кислородом.
            p.rect(-330.0, g + 12.0, 34.0, 24.0, pal::STEEL, 8.0);
            p.circle(-300.0, g + 14.0, 13.0, pal::WHITE, 8.0);
            p.text(-300.0, g + 14.0, "O₂", 11.0, pal::US, 8.1);
            p.rect(-338.0, g + 34.0, 4.0, 20.0, pal::STEEL, 8.0);
        }
        Mission::Apollo19 => {
            // Реактор вдали и робот Mole-1.
            p.rect(-420.0, g + 16.0, 14.0, 32.0, pal::DARK, 8.0);
            for i in 0..4 {
                p.rect(-420.0, g + 6.0 + i as f32 * 8.0, 26.0, 2.0, pal::AMBER, 8.1);
            }
            p.line(v(-412.0, g + 2.0), v(-348.0, g + 2.0), 2.0, pal::DARK, 7.9);
            mole(p, -230.0, g);
        }
        Mission::Apollo20 => {
            // Habitat-Alpha, шлюз и солнечная ферма.
            p.rect(-120.0, g + 15.0, 70.0, 30.0, pal::WHITE, 8.0);
            p.circle(-155.0, g + 15.0, 15.0, pal::WHITE, 8.0);
            p.text(-120.0, g + 15.0, "ALPHA", 11.0, pal::DARK, 8.1);
            p.rect(-75.0, g + 12.0, 20.0, 22.0, pal::STEEL, 8.0);
            for i in 0..3 {
                let x = 110.0 + i as f32 * 34.0;
                p.rect(x, g + 8.0, 2.0, 16.0, pal::STEEL, 8.0);
                p.line(v(x - 14.0, g + 12.0), v(x + 14.0, g + 24.0), 5.0, pal::US, 8.1);
            }
        }
        Mission::Apollo21 => {
            p.rect(-20.0, g + 15.0, 80.0, 30.0, pal::WHITE, 8.0);
            p.circle(20.0, g + 15.0, 15.0, pal::WHITE, 8.0);
            p.text(-20.0, g + 15.0, "BETA", 11.0, pal::DARK, 8.1);
            // Скальный навес над жилыми блоками.
            p.ellipse(-70.0, g + 44.0, 130.0, 14.0, pal::MOON_DARK, 8.2);
        }
        Mission::Apollo22 => {
            lander(p, 290.0, g, 1.5, pal::WHITE, 4);
            for (i, x) in [228.0, 244.0, 340.0, 356.0].into_iter().enumerate() {
                astronaut(p, x, g - 14.0 - (i % 2) as f32 * 6.0, pal::WHITE);
            }
            flag(p, 210.0, g - 12.0, pal::US);
        }
        Mission::Apollo23 => {}
        Mission::Reserve => {
            // Склады резервного рейса.
            for (i, x) in [150.0, 176.0, 163.0].into_iter().enumerate() {
                let y = g + 9.0 + if i == 2 { 18.0 } else { 0.0 };
                p.rect(x, y, 24.0, 18.0, pal::GOLD, 8.0);
                p.rect(x, y, 24.0, 2.0, pal::DARK, 8.1);
            }
        }
    }
}

/// Робот-землекоп Mole-1. Точка (x, y) — уровень грунта.
fn mole(p: &mut Painter, x: f32, y: f32) {
    p.rect(x, y + 10.0, 36.0, 12.0, pal::GOLD, 8.0);
    p.circle(x - 12.0, y + 4.0, 5.0, pal::DARK, 8.1);
    p.circle(x + 12.0, y + 4.0, 5.0, pal::DARK, 8.1);
    p.line(v(x + 18.0, y + 14.0), v(x + 34.0, y + 4.0), 3.0, pal::STEEL, 8.0);
}

/// Солнечная панель: рама, ферма и сетка фотоэлементов. Центр в (x, y).
fn solar_wing(p: &mut Painter, x: f32, y: f32, w: f32, h: f32, z: f32) {
    p.rect(x, y, w, h, pal::DARK, z);
    let cols = (w / 5.0).max(2.0) as i32;
    let cell = (w - 2.0) / cols as f32;
    for i in 0..cols {
        let cx = x - w / 2.0 + 1.0 + cell * (i as f32 + 0.5);
        p.rect(cx, y, cell - 1.0, h - 2.0, Color::srgb(0.20, 0.36, 0.66), z + 0.01);
    }
    // Блик по диагонали: панель повёрнута к Солнцу.
    p.rect(x - w * 0.15, y + h * 0.2, w * 0.35, h * 0.12, pal::WHITE.with_alpha(0.25), z + 0.02);
}

/// Корабль «Союз» у причала станции: бытовой отсек, спускаемый аппарат,
/// приборный отсек с панелями. `dir` — куда смотрит нос (вверх 1, вниз −1).
fn soyuz(p: &mut Painter, x: f32, y: f32, s: f32, dir: f32) {
    let green = Color::srgb(0.42, 0.52, 0.40);
    p.ellipse(x, y + dir * 6.0 * s, 4.5 * s, 4.5 * s, green, 6.0);
    p.tri(x, y + dir * 10.0 * s, [(-4.5 * s, 0.0), (4.5 * s, 0.0), (0.0, dir * 7.0 * s)], green, 6.0);
    p.rect(x, y + dir * 19.0 * s, 7.0 * s, 6.0 * s, pal::STEEL, 6.0);
    solar_wing(p, x - 9.0 * s, y + dir * 19.0 * s, 9.0 * s, 3.0 * s, 5.95);
    solar_wing(p, x + 9.0 * s, y + dir * 19.0 * s, 9.0 * s, 3.0 * s, 5.95);
}

/// Один модуль орбитальной станции: цилиндр с бликом и тенью, шпангоуты,
/// стыковочный узел и оборудование по назначению. Ракетный модуль красный.
fn station_module(p: &mut Painter, x: f32, y: f32, s: f32, number: u8) {
    let armed = number == DEFENCE_MODULE;
    let hull = if armed { pal::USSR } else { pal::STEEL };
    let shine = if armed { Color::srgb(1.0, 0.70, 0.64) } else { pal::WHITE };
    let shade = Color::srgb(0.22, 0.24, 0.29);
    p.rect(x, y, 28.0 * s, 16.0 * s, hull, 6.0);
    p.rect(x, y + 4.5 * s, 28.0 * s, 3.0 * s, shine.with_alpha(0.55), 6.05);
    p.rect(x, y - 5.5 * s, 28.0 * s, 4.0 * s, shade.with_alpha(0.55), 6.05);
    for dx in [-9.0, 0.0, 9.0] {
        p.rect(x + dx * s, y, 1.2 * s, 16.0 * s, shade.with_alpha(0.45), 6.08);
    }
    // Стыковочный узел к предыдущему модулю.
    p.rect(x - 15.0 * s, y, 4.0 * s, 9.0 * s, pal::DARK, 6.1);
    match number {
        1 => {
            // Ферма реактора, радиаторы и тлеющий источник на конце.
            p.rect(x, y + 20.0 * s, 1.6 * s, 26.0 * s, pal::STEEL, 5.9);
            for i in 0..3 {
                p.rect(x, y + (13.0 + i as f32 * 6.0) * s, 14.0 * s, 1.4 * s, pal::MUTED, 5.92);
            }
            let core = p.circle(x, y + 35.0 * s, 4.5 * s, pal::AMBER, 6.0);
            p.commands.entity(core).insert(Flicker::new(0.7));
            p.circle(x, y + 35.0 * s, 7.5 * s, pal::AMBER.with_alpha(0.25), 5.95);
        }
        2 => {
            for dir in [-1.0, 1.0] {
                p.rect(x, y + dir * 11.0 * s, 1.5 * s, 6.0 * s, pal::STEEL, 5.9);
                solar_wing(p, x, y + dir * 25.0 * s, 18.0 * s, 22.0 * s, 5.9);
            }
        }
        3 => soyuz(p, x, y + 8.0 * s, s, 1.0),
        4 => {
            // Пусковые трубы с красными головками и сканирующий радар.
            for i in 0..4 {
                let tx = x + (i as f32 - 1.5) * 5.5 * s;
                p.rect(tx, y - 12.0 * s, 3.5 * s, 8.0 * s, pal::DARK, 6.0);
                p.rect(tx, y - 16.5 * s, 3.5 * s, 2.0 * s, pal::FIRE, 6.01);
            }
            p.ellipse(x, y + 12.0 * s, 7.0 * s, 2.5 * s, pal::WHITE, 6.15);
            let beam = p.tri(x, y + 12.0 * s, [(-1.5 * s, 0.0), (1.5 * s, 0.0), (0.0, 22.0 * s)], pal::USSR.with_alpha(0.55), 6.2);
            p.commands.entity(beam).insert(Sweep { base: 0.0, amp: 1.1, speed: 2.5 });
        }
        _ => {
            soyuz(p, x, y - 8.0 * s, s, -1.0);
            // Тяжёлый спускаемый корабль: золотая изоляция и опоры.
            p.rect(x, y + 13.0 * s, 14.0 * s, 9.0 * s, pal::GOLD, 6.0);
            for dir in [-1.0, 1.0] {
                p.line(v(x + dir * 6.0 * s, y + 17.0 * s), v(x + dir * 11.0 * s, y + 23.0 * s), 1.2 * s, pal::STEEL, 5.98);
            }
        }
    }
}

/// Станция из `modules` модулей; левый край в точке (x, y). На концах
/// мигают навигационные огни: красный слева, зелёный справа.
fn station(p: &mut Painter, x: f32, y: f32, s: f32, modules: u8) {
    for number in 1..=modules {
        station_module(p, x + (number as f32 - 1.0) * 32.0 * s, y, s, number);
    }
    if modules > 0 {
        let right = x + (modules as f32 - 1.0) * 32.0 * s + 15.0 * s;
        for (lx, color, phase) in [(x - 17.0 * s, pal::USSR, 0.0), (right, pal::GREEN, 3.1)] {
            let lamp = p.rect(lx, y, 2.5 * s.max(0.8), 2.5 * s.max(0.8), color, 6.3);
            p.commands.entity(lamp).insert(Twinkle(phase));
        }
    }
}

/// Середина станции из `modules` модулей относительно её левого края.
fn station_middle(s: f32, modules: u8) -> f32 {
    (modules.max(1) as f32 - 1.0) * 16.0 * s
}

/// Справка о советской станции для подсказки под курсором.
fn los_tip(text: &Catalog, modules: u8, war: War) -> String {
    let status = match war {
        War::Truce => "tip.los.truce",
        _ if modules >= DEFENCE_MODULE => "tip.los.armed",
        _ => "tip.los.calm",
    };
    format!(
        "{}\n{}\n{}",
        text.get("tip.los").replace("{n}", &modules.to_string()),
        text.get(status),
        text.get("tip.warp")
    )
}

/// Станция на небесной орбите с подсказкой. `anchor` — её середина.
#[allow(clippy::too_many_arguments)]
fn orbiting_station(p: &mut Painter, s: f32, modules: u8, tip: String, center: Vec2, radius: Vec2, speed: f32, phase: f32, body: Body) {
    let middle = station_middle(s, modules);
    let group = p.group(|p| {
        station(p, 0.0, 0.0, s, modules);
        p.hotspot(middle, 0.0, 28.0 + middle, tip);
    });
    p.orbit_body(&group, v(middle, 0.0), center, radius, speed, phase, body);
}

/// Станция, которая идёт по дуге над горизонтом и уходит под него.
fn passing_station(p: &mut Painter, modules: u8, war: War, text: &Catalog) {
    if modules == 0 || war == War::Won {
        return;
    }
    let tip = los_tip(text, modules, war);
    orbiting_station(p, 0.55, modules, tip, v(-40.0, GROUND - 120.0), v(430.0, 300.0), -0.12, 2.3, Body::overhead(1.6));
}

/// Станция снабжения Apollo 23: баки, склад и две панели.
fn supply_hub(p: &mut Painter, x: f32, y: f32, s: f32) {
    p.rect(x, y, 30.0 * s, 13.0 * s, pal::WHITE, 6.0);
    p.rect(x, y + 4.0 * s, 30.0 * s, 2.5 * s, pal::STEEL.with_alpha(0.6), 6.05);
    for dx in [-8.0, 8.0] {
        p.circle(x + dx * s, y - 9.0 * s, 4.5 * s, pal::GOLD, 5.95);
    }
    for dir in [-1.0, 1.0] {
        p.rect(x + dir * 18.0 * s, y, 6.0 * s, 1.5 * s, pal::STEEL, 5.9);
        solar_wing(p, x + dir * 30.0 * s, y, 18.0 * s, 20.0 * s, 5.9);
    }
    p.rect(x, y + 9.0 * s, 5.0 * s, 4.0 * s, pal::DARK, 6.1);
    let lamp = p.rect(x, y + 12.0 * s, 2.0 * s, 2.0 * s, pal::GREEN, 6.2);
    p.commands.entity(lamp).insert(Twinkle(1.0));
}

/// Командно-служебный модуль «Аполлона», нос смотрит вправо.
fn csm(p: &mut Painter, x: f32, y: f32, s: f32) {
    p.tri(x - 34.0 * s, y, [(0.0, -9.0 * s), (0.0, 9.0 * s), (-12.0 * s, 0.0)], pal::DARK, 6.0);
    p.rect(x - 14.0 * s, y, 40.0 * s, 20.0 * s, pal::STEEL, 6.1);
    p.rect(x - 14.0 * s, y, 4.0 * s, 20.0 * s, pal::DARK, 6.2);
    p.tri(x + 6.0 * s, y, [(0.0, -10.0 * s), (0.0, 10.0 * s), (18.0 * s, 0.0)], pal::WHITE, 6.1);
}

// ═══════════════════════════════ ЛОКАЦИИ ════════════════════════════════

/// Рисует сцену для текущего места действия.
pub fn draw_location(p: &mut Painter, state: &GameState, text: &Catalog) {
    location(p, state, text);
    fade_in(p);
}

fn location(p: &mut Painter, state: &GameState, text: &Catalog) {
    match state.location {
        Location::Briefing => briefing(p, text),
        Location::FundingOffice => funding(p),
        Location::DesignBureau => design(p),
        Location::Factory => factory(p, state),
        Location::LaunchPad => launch_pad(p, state, true),
        Location::Surface => surface(p, state, true, text),
        Location::UnitedNations => united_nations(p),
        Location::Accident => accident(p),
        Location::Event(event) => dossier_event(p, event),
        Location::SovietFirst | Location::SovietLanding => far_side(p, state, true, text),
        Location::SpareFlight => {
            launch_pad(p, state, true);
            p.text(-40.0, 110.0, "SHADOW-7", 16.0, pal::AMBER, 12.0);
        }
        Location::MarsWindow => mars_window(p, text),
        Location::MarsEra => mars_era(p),
        Location::Bypass => bypass(p),
        Location::Handshake => handshake(p, text),
        Location::Tribunal => tribunal(p),
        Location::OpenBase => {
            surface(p, state, false, text);
            for (i, color) in [pal::GREEN, pal::AMBER, pal::WHITE, pal::USSR, pal::US].into_iter().enumerate() {
                flag(p, -400.0 + i as f32 * 40.0, GROUND - 70.0, color);
            }
            lander(p, -330.0, GROUND - 40.0, 0.8, pal::GREEN, 2);
        }
        Location::Outpost => {
            surface(p, state, false, text);
            p.rect(0.0, 0.0, HW * 2.0, HH * 2.0, pal::NIGHT.with_alpha(0.55), 20.0);
            let lamp = p.circle(-78.0, GROUND + 36.0, 5.0, pal::AMBER, 21.0);
            p.commands.entity(lamp).insert(Flicker::new(0.0));
        }
        Location::ColdPeace => cold_peace(p, text),
        Location::MoonWar => lunar_war(p, state, text),
        Location::Stranded => stranded(p, state, text),
        Location::SecondPlace => {
            far_side(p, state, true, text);
            p.rect(-150.0, GROUND + 14.0, 90.0, 28.0, pal::STEEL, 8.0);
            p.circle(-195.0, GROUND + 14.0, 14.0, pal::STEEL, 8.0);
            flag(p, -90.0, GROUND, pal::USSR);
        }
        Location::Impeached => impeached(p),
        Location::Uprising => uprising(p, state.autocracy),
        Location::Bankrupt => bankrupt(p),
        Location::Brink => war_room(p),
        Location::Politburo => politburo(p),
        Location::JointMars => joint_mars(p),
        Location::SovietCollapse => collapse(p, state, text),
        Location::MoonTreaty => treaty(p, text),
        Location::MoonVictory => moon_victory(p, state, false, text),
        Location::IronMoon => moon_victory(p, state, true, text),
        Location::RedMoon => red_moon(p, state, text),
        Location::Tactical => earth_from_moon(p, false),
        Location::Armageddon => earth_from_moon(p, true),
        Location::Cancelled => {
            launch_pad(p, state, false);
            p.rect(250.0, -60.0, 180.0, 70.0, pal::AMBER, 12.0);
            p.rect(250.0, -60.0, 170.0, 60.0, pal::DARK, 12.1);
            p.text(250.0, -60.0, "ЗАКРЫТО", 28.0, pal::AMBER, 12.2);
            p.rect(250.0, -115.0, 6.0, 40.0, pal::STEEL, 11.9);
        }
    }
}

fn briefing(p: &mut Painter, text: &Catalog) {
    sky(p, pal::NIGHT, 110, -HH, 7);
    // Земля крупно, спутник-разведчик и далёкая Луна под вопросом.
    p.circle(-250.0, -330.0, 420.0, pal::EARTH.with_alpha(0.25), 2.0);
    p.circle(-250.0, -330.0, 400.0, pal::EARTH, 2.1);
    p.ellipse(-330.0, -20.0, 150.0, 60.0, pal::LAND, 2.2);
    p.ellipse(-90.0, -70.0, 110.0, 45.0, pal::LAND, 2.2);
    p.ellipse(-200.0, 20.0, 190.0, 16.0, pal::WHITE.with_alpha(0.7), 2.3);
    let target = p.circle(-120.0, -40.0, 9.0, pal::USSR, 2.4);
    p.commands.entity(target).insert(Flicker::new(0.0));
    p.text(-120.0, -65.0, "ТЮРА-ТАМ", 13.0, pal::USSR, 2.5);

    let satellite = p.group(|p| {
        p.rect(0.0, 0.0, 34.0, 12.0, pal::GOLD, 6.0);
        p.rect(-28.0, 0.0, 18.0, 22.0, pal::US, 6.0);
        p.rect(28.0, 0.0, 18.0, 22.0, pal::US, 6.0);
        p.tri(0.0, -6.0, [(-5.0, 0.0), (5.0, 0.0), (0.0, -10.0)], pal::DARK, 6.0);
        p.text(0.0, 22.0, "KH-9", 12.0, pal::MUTED, 6.0);
        p.hotspot(0.0, 0.0, 30.0, format!("{}\n{}", text.get("tip.kh9"), text.get("tip.warp")));
    });
    let body = Body { ecc: 0.08, far_z: 1.5, far_up: true, shrink: 0.3, tilt: 0.4 };
    orbit_trace(p, v(-250.0, -330.0), v(500.0, 470.0), body, pal::US, 1.9);
    p.orbit_body(&satellite, v(0.0, 0.0), v(-250.0, -330.0), v(500.0, 470.0), -0.10, 1.75, body);

    far_moon(p, 330.0, 140.0, 46.0);
    let mark = p.text(330.0, 140.0, "?", 54.0, pal::USSR, 3.0);
    p.commands.entity(mark).insert(Blink);
    for i in 0..9 {
        p.rect(40.0 + i as f32 * 26.0, 60.0 + i as f32 * 7.0, 12.0, 2.0, pal::LINE, 1.5);
    }
    p.text(150.0, 60.0, "384 000 км", 13.0, pal::MUTED, 1.6);
}

fn funding(p: &mut Painter) {
    p.rect(0.0, 0.0, HW * 2.0, HH * 2.0, Color::srgb(0.10, 0.12, 0.20), 0.0);
    p.rect(0.0, 120.0, HW * 2.0, 250.0, Color::srgb(0.16, 0.14, 0.24), 0.1);
    p.rect(0.0, -200.0, HW * 2.0, 90.0, pal::DARK, 1.0);
    // Здание казначейства.
    let stone = Color::srgb(0.78, 0.76, 0.70);
    let shade = Color::srgb(0.55, 0.54, 0.50);
    p.rect(-110.0, -150.0, 520.0, 14.0, stone, 2.0);
    p.rect(-110.0, -136.0, 480.0, 14.0, shade, 2.0);
    p.rect(-110.0, -30.0, 440.0, 200.0, Color::srgb(0.20, 0.20, 0.26), 2.0);
    for i in 0..7 {
        let x = -300.0 + i as f32 * 63.0;
        p.rect(x, -30.0, 22.0, 200.0, stone, 2.2);
        p.rect(x - 7.0, -30.0, 4.0, 200.0, shade, 2.3);
        p.rect(x, 74.0, 32.0, 8.0, stone, 2.3);
        p.rect(x, -126.0, 32.0, 8.0, stone, 2.3);
    }
    p.rect(-110.0, 88.0, 480.0, 20.0, stone, 2.4);
    p.tri(-110.0, 98.0, [(-250.0, 0.0), (250.0, 0.0), (0.0, 80.0)], stone, 2.4);
    p.text(-110.0, 124.0, "F C E E S", 26.0, pal::DARK, 2.5);
    p.text(-110.0, 88.0, "FEDERAL COMMITTEE FOR ECOLOGICAL COMPENSATION", 10.0, pal::DARK, 2.5);
    // Деньги уходят в «чёрный бюджет».
    let mut dice = Dice(42);
    for i in 0..10 {
        let bill = p.group(|p| {
            p.rect(0.0, 0.0, 30.0, 16.0, pal::GREEN, 6.0);
            p.text(0.0, 0.0, "$", 13.0, pal::DARK, 6.1);
        });
        let center = v(250.0 + dice.range(-30.0, 120.0), -60.0 + i as f32 * 26.0);
        p.orbit(&bill, v(0.0, 0.0), center, v(14.0, 22.0), dice.range(0.8, 1.6), dice.range(0.0, 6.0));
    }
    p.rect(320.0, -170.0, 150.0, 60.0, pal::DARK, 5.0);
    p.rect(320.0, -142.0, 160.0, 8.0, pal::STEEL, 5.1);
    p.text(320.0, -172.0, "BLACK BUDGET", 14.0, pal::AMBER, 5.2);
}

fn design(p: &mut Painter) {
    let paper = Color::srgb(0.05, 0.16, 0.30);
    let ink = Color::srgb(0.75, 0.86, 1.0);
    let grid = Color::srgb(0.10, 0.24, 0.42);
    p.rect(0.0, 0.0, HW * 2.0, HH * 2.0, paper, 0.0);
    for i in 0..24 {
        p.rect(-HW + i as f32 * 40.0, 0.0, 1.0, HH * 2.0, grid, 0.1);
    }
    for i in 0..13 {
        p.rect(0.0, -HH + i as f32 * 40.0, HW * 2.0, 1.0, grid, 0.1);
    }
    p.rect(0.0, 0.0, 2.0, HH * 2.0 - 60.0, ink.with_alpha(0.4), 0.2);

    // План А: доработанный «Аполлон».
    p.text(-230.0, 205.0, "ПЛАН А · APOLLO / ALM", 18.0, ink, 1.0);
    let outline = |p: &mut Painter, points: &[(f32, f32)]| {
        for pair in points.windows(2) {
            p.line(v(pair[0].0, pair[0].1), v(pair[1].0, pair[1].1), 2.0, ink, 1.0);
        }
    };
    outline(p, &[(-260.0, 150.0), (-230.0, 180.0), (-200.0, 150.0), (-260.0, 150.0)]);
    outline(p, &[(-262.0, 150.0), (-262.0, 70.0), (-198.0, 70.0), (-198.0, 150.0)]);
    outline(p, &[(-245.0, 70.0), (-255.0, 48.0), (-205.0, 48.0), (-215.0, 70.0)]);
    outline(p, &[(-270.0, 30.0), (-250.0, 46.0), (-210.0, 46.0), (-190.0, 30.0), (-190.0, -10.0), (-270.0, -10.0), (-270.0, 30.0)]);
    outline(p, &[(-285.0, -10.0), (-175.0, -10.0), (-175.0, -50.0), (-285.0, -50.0), (-285.0, -10.0)]);
    outline(p, &[(-285.0, -30.0), (-330.0, -95.0), (-345.0, -95.0)]);
    outline(p, &[(-175.0, -30.0), (-130.0, -95.0), (-115.0, -95.0)]);
    p.text(-370.0, 110.0, "CM", 13.0, ink, 1.0);
    p.text(-370.0, 10.0, "ALM", 13.0, ink, 1.0);
    p.line(v(-350.0, 110.0), v(-266.0, 110.0), 1.0, ink, 1.0);
    p.line(v(-346.0, 10.0), v(-274.0, 10.0), 1.0, ink, 1.0);
    p.text(-230.0, -150.0, "готов к лету 1974", 14.0, pal::AMBER, 1.0);

    // План Б: база в лавовой трубке.
    p.text(230.0, 205.0, "ПЛАН Б · ПОДЗЕМНАЯ БАЗА", 18.0, ink, 1.0);
    outline(p, &[(40.0, 90.0), (120.0, 96.0), (190.0, 84.0), (270.0, 98.0), (340.0, 88.0), (420.0, 94.0)]);
    outline(p, &[(90.0, 40.0), (130.0, 70.0), (330.0, 70.0), (380.0, 40.0), (380.0, -40.0), (90.0, -40.0), (90.0, 40.0)]);
    outline(p, &[(120.0, -40.0), (120.0, 0.0), (210.0, 0.0), (210.0, -40.0)]);
    outline(p, &[(250.0, -40.0), (250.0, 0.0), (350.0, 0.0), (350.0, -40.0)]);
    outline(p, &[(210.0, -20.0), (250.0, -20.0)]);
    p.text(165.0, -20.0, "A", 16.0, ink, 1.0);
    p.text(300.0, -20.0, "B", 16.0, ink, 1.0);
    p.text(230.0, 140.0, "реголит 4 м: защита от радиации", 13.0, ink, 1.0);
    p.line(v(230.0, 128.0), v(230.0, 96.0), 1.0, ink, 1.0);
    p.text(230.0, -150.0, "смета ×3", 14.0, pal::AMBER, 1.0);
}

/// Стартовая площадка. `lit` включает прожекторы.
fn launch_pad(p: &mut Painter, state: &GameState, lit: bool) {
    sky(p, pal::NIGHT, 80, -150.0, 11);
    far_moon(p, 320.0, 160.0, 30.0);
    if state.soviet_modules > 0 {
        let dot = p.rect(362.0, 172.0, 4.0, 4.0, pal::USSR, 3.0);
        p.commands.entity(dot).insert(Twinkle(1.0));
    }
    // Океан, советский корабль радиоразведки, берег.
    p.rect(0.0, -165.0, HW * 2.0, 30.0, pal::SEA, 2.0);
    p.rect(-340.0, -148.0, 46.0, 6.0, pal::DARK, 2.1);
    p.rect(-346.0, -141.0, 14.0, 8.0, pal::DARK, 2.1);
    p.rect(-330.0, -134.0, 2.0, 22.0, pal::DARK, 2.1);
    let spy = p.rect(-330.0, -122.0, 4.0, 4.0, pal::USSR, 2.2);
    p.commands.entity(spy).insert(Twinkle(2.0));
    p.rect(0.0, -212.0, HW * 2.0, 66.0, Color::srgb(0.10, 0.13, 0.12), 3.0);
    p.rect(-30.0, -176.0, 260.0, 12.0, pal::STEEL, 5.0);
    p.rect(-30.0, -186.0, 220.0, 10.0, pal::DARK, 5.0);

    if lit {
        for (x, base) in [(-300.0, -0.45), (240.0, 0.5)] {
            let beam = p.tri(x, -180.0, [(-4.0, 0.0), (4.0, 0.0), (0.0, 420.0)], pal::WHITE.with_alpha(0.10), 4.0);
            p.commands.entity(beam).insert(Sweep { base, amp: 0.18, speed: 0.7 });
            p.rect(x, -178.0, 14.0, 10.0, pal::STEEL, 5.0);
        }
    }
    tower(p, 50.0, -170.0, 250.0);
    rocket(p, -40.0, -170.0, 1.0);
}

/// База «Тихо». `visiting` добавляет груз и корабль текущей миссии.
fn surface(p: &mut Painter, state: &GameState, visiting: bool, text: &Catalog) {
    sky(p, pal::NIGHT, 120, GROUND, 3);
    earth(p, -330.0, 150.0, 44.0);
    passing_station(p, state.soviet_modules, state.war, text);
    moon_ground(p, pal::MOON, 5);
    p.text(-395.0, GROUND + 62.0, "кратер Тихо", 12.0, pal::MUTED, 3.5);

    let on_site = if visiting { state.delivered.with(state.mission) } else { state.delivered };
    for mission in on_site.iter() {
        base_module(p, mission);
    }
    if on_site.contains(Mission::Apollo23) {
        // Станция снабжения висит над базой.
        let hub = p.group(|p| {
            supply_hub(p, 0.0, 0.0, 0.8);
            p.hotspot(0.0, 0.0, 34.0, format!("{}\n{}", text.get("tip.hub"), text.get("tip.warp")));
        });
        p.orbit_body(&hub, v(0.0, 0.0), v(120.0, 150.0), v(70.0, 16.0), 0.5, 0.0, Body::around(1.6));
    }
    // Экипаж обычной миссии: двое на поверхности у своего корабля.
    if visiting && state.mission != Mission::Apollo22 && state.mission != Mission::Apollo23 {
        lander(p, 350.0, GROUND - 6.0, 1.0, pal::STEEL, 2);
        astronaut(p, 300.0, GROUND - 22.0, pal::WHITE);
        astronaut(p, 270.0, GROUND - 30.0, pal::WHITE);
    }
}

fn united_nations(p: &mut Painter) {
    p.rect(0.0, 0.0, HW * 2.0, HH * 2.0, Color::srgb(0.07, 0.11, 0.20), 0.0);
    p.rect(0.0, 60.0, 700.0, 330.0, Color::srgb(0.10, 0.16, 0.28), 0.1);
    // Эмблема.
    let gold = pal::GOLD;
    p.circle(0.0, 120.0, 86.0, gold, 1.0);
    p.circle(0.0, 120.0, 78.0, Color::srgb(0.10, 0.16, 0.28), 1.1);
    for r in [60.0, 40.0, 20.0] {
        p.circle(0.0, 120.0, r, gold, 1.2);
        p.circle(0.0, 120.0, r - 3.0, Color::srgb(0.10, 0.16, 0.28), 1.3);
    }
    p.rect(0.0, 120.0, 124.0, 3.0, gold, 1.4);
    p.rect(0.0, 120.0, 3.0, 124.0, gold, 1.4);
    // Президиум и две делегации.
    p.rect(0.0, -30.0, 420.0, 50.0, Color::srgb(0.30, 0.22, 0.14), 2.0);
    p.rect(0.0, -4.0, 440.0, 8.0, Color::srgb(0.42, 0.30, 0.18), 2.1);
    for (x, color, label) in [(-300.0, pal::US, "USA"), (300.0, pal::USSR, "СССР")] {
        p.rect(x, -70.0, 150.0, 60.0, Color::srgb(0.30, 0.22, 0.14), 3.0);
        p.rect(x, -38.0, 160.0, 8.0, Color::srgb(0.42, 0.30, 0.18), 3.1);
        p.text(x, -70.0, label, 20.0, pal::WHITE, 3.2);
        p.circle(x, -8.0, 16.0, pal::DARK, 2.9);
        p.rect(x, -30.0, 40.0, 30.0, pal::DARK, 2.9);
        p.rect(x + 90.0, 20.0, 3.0, 150.0, pal::STEEL, 2.5);
        p.rect(x + 116.0, 78.0, 50.0, 32.0, color, 2.6);
    }
    // Схема траекторий в руках советского делегата.
    p.rect(215.0, 20.0, 70.0, 50.0, pal::WHITE, 3.5);
    p.circle(196.0, 10.0, 8.0, pal::EARTH, 3.6);
    p.circle(240.0, 34.0, 5.0, pal::MOON, 3.6);
    p.line(v(200.0, 14.0), v(237.0, 32.0), 2.0, pal::USSR, 3.7);
    // Зал.
    let mut dice = Dice(9);
    for row in 0..3 {
        for i in 0..18 {
            let x = -430.0 + i as f32 * 50.0 + row as f32 * 14.0 + dice.range(-4.0, 4.0);
            let y = -150.0 - row as f32 * 34.0;
            p.circle(x, y + 14.0, 10.0, pal::DARK, 5.0 + row as f32);
            p.rect(x, y - 6.0, 28.0, 26.0, pal::DARK, 5.0 + row as f32);
        }
    }
}

/// Обратная сторона Луны с советской техникой.
fn far_side(p: &mut Painter, state: &GameState, landed: bool, text: &Catalog) {
    sky(p, pal::NIGHT, 150, GROUND, 21);
    let modules = state.soviet_modules;
    if state.war != War::Won && modules > 0 {
        let tip = los_tip(text, modules, state.war);
        orbiting_station(p, 0.7, modules, tip, v(-60.0, 150.0), v(170.0, 26.0), 0.25, 0.0, Body::around(1.5));
    }
    moon_ground(p, pal::MOON_DARK, 17);
    if landed {
        lander(p, 60.0, GROUND, 1.6, pal::USSR, 3);
        // Второй оборонный модуль с радаром.
        p.rect(260.0, GROUND + 14.0, 60.0, 28.0, pal::USSR, 8.0);
        p.rect(260.0, GROUND + 34.0, 4.0, 14.0, pal::STEEL, 8.0);
        let radar = p.tri(260.0, GROUND + 40.0, [(-3.0, 0.0), (3.0, 0.0), (0.0, 46.0)], pal::WHITE, 8.1);
        p.commands.entity(radar).insert(Sweep { base: 0.0, amp: 1.2, speed: 2.0 });
        for i in 0..3 {
            p.rect(238.0 + i as f32 * 22.0, GROUND + 16.0, 8.0, 18.0, pal::DARK, 8.1);
        }
        for x in [-40.0, -70.0, 150.0] {
            astronaut(p, x, GROUND - 18.0, Color::srgb(0.95, 0.80, 0.72));
        }
        flag(p, -10.0, GROUND - 6.0, pal::USSR);
    }
}

fn mars_era(p: &mut Painter) {
    sky(p, pal::NIGHT, 170, -HH, 31);
    // Позади остаются Земля и Луна, впереди растёт Марс.
    earth(p, -380.0, -120.0, 34.0);
    far_moon(p, -300.0, -80.0, 12.0);
    p.circle(330.0, 80.0, 74.0, pal::MARS.with_alpha(0.25), 2.0);
    p.circle(330.0, 80.0, 66.0, pal::MARS, 2.1);
    p.ellipse(310.0, 100.0, 30.0, 14.0, Color::srgb(0.62, 0.26, 0.16), 2.2);
    p.ellipse(350.0, 50.0, 22.0, 18.0, Color::srgb(0.62, 0.26, 0.16), 2.2);
    p.ellipse(330.0, 140.0, 22.0, 6.0, pal::WHITE, 2.2);
    for i in 0..16 {
        p.rect(-260.0 + i as f32 * 34.0, -62.0 + i as f32 * 8.5, 14.0, 2.0, pal::LINE, 1.5);
    }
    let ship = p.group(|p| {
        csm(p, 0.0, 0.0, 1.6);
        let glow = p.tri(-74.0, 0.0, [(0.0, -7.0), (0.0, 7.0), (-34.0, 0.0)], pal::AMBER, 5.9);
        p.commands.entity(glow).insert(Flicker::new(0.0));
    });
    p.orbit(&ship, v(0.0, 0.0), v(-40.0, -10.0), v(26.0, 8.0), 0.4, 0.0);
    p.text(-40.0, -70.0, "THE MISSION OF SHADOW", 20.0, pal::AMBER, 6.0);
}

fn cold_peace(p: &mut Painter, text: &Catalog) {
    sky(p, pal::NIGHT, 130, -HH, 41);
    // Луна, разделённая пополам.
    p.circle(0.0, 0.0, 170.0, pal::MOON, 2.0);
    for (x, y, r) in [(-70.0, 60.0, 30.0), (-100.0, -50.0, 22.0), (60.0, 90.0, 18.0), (90.0, -30.0, 36.0), (20.0, -110.0, 16.0)] {
        p.circle(x, y, r, pal::MOON_DARK, 2.1);
        p.circle(x + 2.0, y - 2.0, r * 0.86, pal::MOON, 2.2);
    }
    for i in 0..17 {
        p.rect(0.0, -160.0 + i as f32 * 20.0, 3.0, 10.0, pal::WHITE, 3.0);
    }
    let us = p.circle(-60.0, -70.0, 7.0, pal::US, 3.1);
    let ussr = p.circle(80.0, 30.0, 7.0, pal::USSR, 3.1);
    p.commands.entity(us).insert(Flicker::new(0.0));
    p.commands.entity(ussr).insert(Flicker::new(2.0));
    p.text(-60.0, -92.0, "ТИХО", 13.0, pal::WHITE, 3.2);
    p.text(-270.0, 0.0, "ВИДИМАЯ\nСТОРОНА\nСША", 18.0, pal::US, 3.2);
    p.text(275.0, 0.0, "ОБРАТНАЯ\nСТОРОНА\nСССР", 18.0, pal::USSR, 3.2);
    // Станция уходит за лунный диск и выходит из-за него с другой стороны.
    let body = Body::around(1.5);
    orbit_trace(p, v(0.0, 0.0), v(260.0, 70.0), body, pal::USSR, 3.3);
    orbiting_station(p, 0.5, 5, los_tip(text, 5, War::Peace), v(0.0, 0.0), v(260.0, 70.0), 0.35, 0.0, body);
}

/// Кризис «Война»: перехватчики идут с орбиты, станция держит базу в прицеле.
fn lunar_war(p: &mut Painter, state: &GameState, text: &Catalog) {
    surface(p, state, false, text);
    let lock = dashes(p, &[v(260.0, 170.0), v(-70.0, GROUND + 30.0)], pal::USSR.with_alpha(0.8), 19.0);
    p.tag(&lock, Twinkle(0.0));
    target_mark(p, -70.0, GROUND + 30.0, pal::USSR, text.get("cut.target"), 999.0);
    p.rect(0.0, 0.0, HW * 2.0, HH * 2.0, pal::USSR.with_alpha(0.14), 20.0);
    // Перехватчики идут с орбиты на базу.
    for (i, x) in [-260.0, -60.0, 140.0, 330.0].into_iter().enumerate() {
        let delay = i as f32 * 0.5;
        let missile = p.group(|p| {
            p.line(v(x + 150.0, 300.0), v(x + 162.0, 324.0), 3.0, pal::WHITE, 21.0);
            p.line(v(x + 162.0, 324.0), v(x + 190.0, 380.0), 2.0, pal::FIRE, 21.0);
        });
        p.tag(&missile, Motion { vel: v(-110.0, -220.0), acc: v(0.0, 0.0), delay });
        p.tag(&missile, Reveal { from: delay, to: delay + 1.75 });
        let blast = p.group(|p| {
            p.circle(x - 40.0, GROUND + 6.0, 16.0, pal::FIRE, 22.0);
            p.circle(x - 40.0, GROUND + 6.0, 9.0, pal::AMBER, 22.1);
        });
        p.tag(&blast, Reveal { from: delay + 1.75, to: 600.0 });
        p.tag(&blast, Flicker::new(i as f32));
    }
}

fn stranded(p: &mut Painter, state: &GameState, text: &Catalog) {
    surface(p, state, false, text);
    p.rect(0.0, 0.0, HW * 2.0, HH * 2.0, pal::NIGHT.with_alpha(0.40), 20.0);
    // Четверо смотрят на Землю. Над кораблём мигает сигнал пустых баков.
    let warning = p.text(290.0, GROUND + 120.0, "ТОПЛИВО: 0", 16.0, pal::USSR, 21.0);
    p.commands.entity(warning).insert(Blink);
}

fn impeached(p: &mut Painter) {
    p.rect(0.0, 0.0, HW * 2.0, HH * 2.0, Color::srgb(0.16, 0.14, 0.20), 0.0);
    p.rect(0.0, -190.0, HW * 2.0, 110.0, Color::srgb(0.27, 0.25, 0.31), 1.0);
    // Капитолий.
    let stone = Color::srgb(0.80, 0.79, 0.75);
    p.rect(0.0, -90.0, 520.0, 90.0, stone, 2.0);
    p.rect(0.0, -20.0, 220.0, 60.0, stone, 2.0);
    p.circle(0.0, 20.0, 80.0, stone, 2.0);
    p.rect(0.0, -20.0, 220.0, 60.0, stone, 2.1);
    p.rect(0.0, 110.0, 30.0, 40.0, stone, 2.0);
    p.rect(0.0, 140.0, 4.0, 30.0, pal::STEEL, 2.0);
    for i in 0..12 {
        p.rect(-242.0 + i as f32 * 44.0, -90.0, 10.0, 80.0, Color::srgb(0.55, 0.54, 0.50), 2.2);
    }
    // Толпа с плакатами.
    let mut dice = Dice(77);
    let slogans = ["ГДЕ НАШИ\nНАЛОГИ?", "FCEES =\nЛОЖЬ", "ЛУНА\nИЛИ ХЛЕБ?", "ИМПИЧМЕНТ"];
    for i in 0..22 {
        let x = -440.0 + i as f32 * 42.0 + dice.range(-8.0, 8.0);
        let y = -190.0 + dice.range(-20.0, 20.0);
        p.circle(x, y + 26.0, 10.0, pal::DARK, 5.0);
        p.rect(x, y + 4.0, 24.0, 32.0, pal::DARK, 5.0);
        if i % 5 == 1 {
            let sign = p.group(|p| {
                p.rect(x + 6.0, y + 52.0, 3.0, 50.0, pal::STEEL, 4.9);
                p.rect(x + 6.0, y + 92.0, 96.0, 44.0, pal::WHITE, 5.1);
                p.text(x + 6.0, y + 92.0, slogans[(i / 5) % slogans.len()], 13.0, pal::DARK, 5.2);
            });
            p.orbit(&sign, v(x, y), v(x, y), v(3.0, 5.0), 3.0 + dice.roll(), dice.range(0.0, 6.0));
        }
    }
}



// ═══════════════════════════ НОВЫЕ ЛОКАЦИИ ══════════════════════════════

/// Сборочный цех. Построенные ракеты белые, ракеты третьей смены — призраки.
fn factory(p: &mut Painter, state: &GameState) {
    p.rect(0.0, 0.0, HW * 2.0, HH * 2.0, Color::srgb(0.09, 0.10, 0.13), 0.0);
    // Фермы крыши и кран-балка, которая ездит над цехом.
    for i in 0..10 {
        let x = -HW + 46.0 + i as f32 * 92.0;
        p.line(v(x - 46.0, HH - 10.0), v(x, HH - 60.0), 3.0, pal::LINE, 0.5);
        p.line(v(x, HH - 60.0), v(x + 46.0, HH - 10.0), 3.0, pal::LINE, 0.5);
    }
    p.rect(0.0, HH - 60.0, HW * 2.0, 6.0, pal::LINE, 0.6);
    let crane = p.group(|p| {
        p.rect(0.0, 0.0, 120.0, 12.0, pal::AMBER, 1.0);
        p.rect(0.0, -40.0, 2.0, 70.0, pal::STEEL, 1.0);
        p.rect(0.0, -76.0, 14.0, 6.0, pal::STEEL, 1.0);
    });
    p.orbit(&crane, v(0.0, 0.0), v(0.0, HH - 76.0), v(300.0, 0.0), 0.25, 0.0);
    // Пол с полосой безопасности.
    p.rect(0.0, -HH + 40.0, HW * 2.0, 80.0, Color::srgb(0.14, 0.15, 0.17), 1.0);
    for i in 0..23 {
        p.rect(-HW + 20.0 + i as f32 * 40.0, -172.0, 20.0, 4.0, pal::AMBER, 1.1);
    }
    let mut dice = Dice(74);
    for i in 0..SATURN_LIMIT {
        let x = -330.0 + i as f32 * 94.0;
        if i < state.saturn_built {
            rocket(p, x, -170.0, 0.42);
            // Искры сварки у корпуса.
            for _ in 0..3 {
                let spark = p.rect(x + dice.range(-10.0, 10.0), dice.range(-150.0, -90.0), 3.0, 3.0, pal::AMBER, 9.0);
                p.commands.entity(spark).insert(Twinkle(dice.range(0.0, 6.0)));
            }
        } else {
            p.rect(x, -120.0, 12.0, 100.0, pal::AMBER.with_alpha(0.15), 7.0);
            let mark = p.text(x, -60.0, "?", 26.0, pal::AMBER, 7.1);
            p.commands.entity(mark).insert(Blink);
        }
    }
    let shifts = if state.saturn_built >= SATURN_LIMIT { "3 СМЕНЫ · 24 Ч" } else { "2 СМЕНЫ · 16 Ч" };
    p.rect(300.0, 120.0, 190.0, 46.0, pal::DARK, 2.0);
    p.text(300.0, 120.0, shifts, 17.0, pal::AMBER, 2.1);
    p.text(-300.0, 120.0, "BOEING · ROCKWELL", 15.0, pal::MUTED, 2.1);
}

/// Атлантика после аварии: капсула на воде и советский траулер рядом.
fn accident(p: &mut Painter) {
    p.rect(0.0, 0.0, HW * 2.0, HH * 2.0, Color::srgb(0.12, 0.13, 0.22), 0.0);
    p.rect(0.0, 150.0, HW * 2.0, 200.0, Color::srgb(0.07, 0.08, 0.15), 0.1);
    let mut dice = Dice(19);
    for _ in 0..40 {
        let star = p.rect(dice.range(-HW, HW), dice.range(80.0, HH), 2.0, 2.0, pal::WHITE, 0.2);
        p.commands.entity(star).insert(Twinkle(dice.range(0.0, 6.0)));
    }
    // Остатки ракеты: столб дыма над горизонтом.
    for i in 0..7 {
        let puff = p.circle(-300.0 + i as f32 * 6.0, -30.0 + i as f32 * 32.0, 20.0 + i as f32 * 5.0, pal::STEEL.with_alpha(0.25), 1.0);
        p.commands.entity(puff).insert(Motion { vel: v(4.0, 3.0), acc: Vec2::ZERO, delay: 0.0 });
    }
    p.rect(0.0, -150.0, HW * 2.0, 200.0, pal::SEA, 2.0);
    for i in 0..9 {
        let wave = p.rect(-400.0 + i as f32 * 100.0, -90.0 - (i % 3) as f32 * 40.0, 60.0, 2.0, pal::LINE, 2.1);
        p.commands.entity(wave).insert(Twinkle(i as f32));
    }
    // Капсула покачивается на волне, вокруг пятно маркера.
    p.ellipse(-60.0, -86.0, 90.0, 16.0, Color::srgb(0.55, 0.85, 0.35).with_alpha(0.35), 2.2);
    let capsule = p.group(|p| {
        p.tri(-60.0, -84.0, [(-28.0, 0.0), (28.0, 0.0), (0.0, 38.0)], pal::STEEL, 3.0);
        p.rect(-60.0, -86.0, 58.0, 6.0, pal::DARK, 3.1);
        p.ellipse(10.0, -88.0, 40.0, 8.0, pal::FIRE, 2.9);
        p.ellipse(30.0, -88.0, 22.0, 6.0, pal::WHITE, 2.95);
    });
    p.orbit(&capsule, v(-60.0, -84.0), v(-60.0, -84.0), v(2.0, 4.0), 1.6, 0.0);
    // Траулер с антеннами.
    p.rect(300.0, -58.0, 170.0, 20.0, pal::DARK, 3.0);
    p.tri(385.0, -68.0, [(0.0, 0.0), (0.0, 20.0), (22.0, 20.0)], pal::DARK, 3.0);
    p.rect(270.0, -38.0, 50.0, 22.0, pal::DARK, 3.0);
    for (x, h) in [(250.0, 70.0), (320.0, 54.0)] {
        p.rect(x, -48.0 + h / 2.0, 3.0, h, pal::DARK, 3.0);
        p.ellipse(x, -48.0 + h, 12.0, 5.0, pal::STEEL, 3.1);
        let lamp = p.rect(x, -40.0 + h, 4.0, 4.0, pal::USSR, 3.2);
        p.commands.entity(lamp).insert(Twinkle(x));
    }
}

/// Папка досье на столе. На фотографии — сюжет события.
fn dossier_event(p: &mut Painter, event: Event) {
    p.rect(0.0, 0.0, HW * 2.0, HH * 2.0, Color::srgb(0.17, 0.12, 0.09), 0.0);
    p.circle(-320.0, 260.0, 420.0, pal::AMBER.with_alpha(0.07), 0.1);
    // Папка, лист и скрепка.
    p.rect(-6.0, -6.0, 620.0, 380.0, Color::srgb(0.55, 0.44, 0.26), 1.0);
    p.rect(-250.0, 190.0, 120.0, 24.0, Color::srgb(0.55, 0.44, 0.26), 1.0);
    p.rect(0.0, 0.0, 580.0, 350.0, Color::srgb(0.92, 0.90, 0.84), 1.1);
    p.rect(-250.0, 160.0, 10.0, 40.0, pal::STEEL, 3.0);
    p.text(130.0, 145.0, "СЕКРЕТНО // GLASS EYE", 17.0, pal::USSR, 1.3);
    let mut dice = Dice(event as u32 + 3);
    for i in 0..11 {
        let width = dice.range(150.0, 230.0);
        p.rect(30.0 + width / 2.0, 100.0 - i as f32 * 20.0, width, 4.0, pal::MUTED.with_alpha(0.6), 1.2);
    }
    // Фотография.
    let (cx, cy) = (-140.0, 10.0);
    p.rect(cx, cy, 214.0, 164.0, pal::WHITE, 1.4);
    p.rect(cx, cy, 200.0, 150.0, pal::NIGHT, 1.5);
    match event {
        Event::Bait => {
            p.circle(cx, cy - 230.0, 180.0, pal::EARTH, 1.6);
            dashes(p, &[v(cx - 90.0, cy - 20.0), v(cx, cy + 10.0), v(cx + 90.0, cy - 20.0)], pal::USSR, 1.7);
            p.rect(cx, cy + 20.0, 24.0, 10.0, pal::GOLD, 1.8);
            p.rect(cx - 20.0, cy + 20.0, 14.0, 16.0, pal::US, 1.8);
            p.rect(cx + 20.0, cy + 20.0, 14.0, 16.0, pal::US, 1.8);
            let mark = p.text(cx + 40.0, cy + 50.0, "?", 30.0, pal::USSR, 1.9);
            p.commands.entity(mark).insert(Blink);
        }
        Event::Families => {
            far_moon(p, cx + 60.0, cy + 45.0, 14.0);
            p.rect(cx - 20.0, cy - 40.0, 90.0, 60.0, pal::DARK, 1.6);
            p.tri(cx - 20.0, cy - 10.0, [(-55.0, 0.0), (55.0, 0.0), (0.0, 36.0)], pal::DARK, 1.6);
            let window = p.rect(cx - 40.0, cy - 36.0, 18.0, 18.0, pal::AMBER, 1.7);
            p.commands.entity(window).insert(Twinkle(0.0));
            p.rect(cx + 6.0, cy - 50.0, 16.0, 30.0, pal::LINE, 1.7);
        }
        Event::Blueprints => {
            p.rect(cx, cy, 200.0, 150.0, Color::srgb(0.05, 0.16, 0.30), 1.55);
            station_module(p, cx, cy, 2.6, DEFENCE_MODULE);
        }
        Event::Hearings => {
            let stone = Color::srgb(0.80, 0.79, 0.75);
            p.rect(cx, cy - 40.0, 160.0, 40.0, stone, 1.6);
            p.circle(cx, cy - 6.0, 40.0, stone, 1.6);
            p.rect(cx, cy + 40.0, 10.0, 20.0, stone, 1.6);
            p.text(cx, cy - 40.0, "FCEES ?", 15.0, pal::USSR, 1.7);
        }
        Event::Milk => {
            p.rect(cx - 30.0, cy - 10.0, 44.0, 90.0, pal::WHITE, 1.6);
            p.rect(cx - 30.0, cy + 45.0, 24.0, 20.0, pal::WHITE, 1.6);
            p.rect(cx - 30.0, cy - 10.0, 44.0, 20.0, pal::US, 1.7);
            p.rect(cx + 50.0, cy + 10.0, 70.0, 34.0, pal::AMBER, 1.6);
            p.text(cx + 50.0, cy + 10.0, "$ ↑↑", 18.0, pal::DARK, 1.7);
        }
        Event::Gromov => {
            station(p, cx - 80.0, cy + 30.0, 0.9, DEFENCE_MODULE);
            // Лунный горизонт в нижней части снимка, не выходя за рамку.
            p.rect(cx, cy - 60.0, 200.0, 30.0, pal::MOON, 1.55);
            p.ellipse(cx - 50.0, cy - 64.0, 18.0, 4.0, pal::MOON_DARK, 1.56);
            p.ellipse(cx + 40.0, cy - 58.0, 12.0, 3.0, pal::MOON_DARK, 1.56);
            let call = p.text(cx + 20.0, cy - 30.0, "((·))  ПРИЁМ", 15.0, pal::GREEN, 1.8);
            p.commands.entity(call).insert(Blink);
        }
        Event::Defector => {
            // Ночная венская улица: фонарь и человек с портфелем.
            p.rect(cx, cy - 60.0, 200.0, 30.0, pal::DARK, 1.6);
            p.rect(cx + 50.0, cy - 5.0, 4.0, 90.0, pal::STEEL, 1.6);
            let lamp = p.circle(cx + 50.0, cy + 42.0, 7.0, pal::AMBER, 1.7);
            p.commands.entity(lamp).insert(Flicker::new(0.0));
            p.tri(cx + 50.0, cy + 36.0, [(-4.0, 0.0), (4.0, 0.0), (0.0, -70.0)], pal::AMBER.with_alpha(0.15), 1.65);
            p.circle(cx - 20.0, cy + 8.0, 7.0, pal::MUTED, 1.7);
            p.rect(cx - 20.0, cy - 20.0, 16.0, 40.0, pal::MUTED, 1.7);
            p.rect(cx - 6.0, cy - 36.0, 14.0, 10.0, pal::GOLD, 1.75);
            p.text(cx - 60.0, cy + 55.0, "WIEN", 13.0, pal::MUTED, 1.8);
        }
        Event::N1Fire => {
            // Степь, чёрный столб дыма и зарево над разрушенным столом.
            p.rect(cx, cy - 55.0, 200.0, 40.0, Color::srgb(0.20, 0.17, 0.13), 1.6);
            for i in 0..5 {
                let puff = p.circle(cx + (i % 2) as f32 * 10.0 - 5.0, cy - 30.0 + i as f32 * 15.0, 13.0 + i as f32 * 3.0, pal::DARK.with_alpha(0.8), 1.7);
                p.commands.entity(puff).insert(Twinkle(i as f32));
            }
            let fire = p.circle(cx, cy - 38.0, 16.0, pal::FIRE, 1.75);
            p.commands.entity(fire).insert(Flicker::new(0.0));
            p.line(v(cx + 40.0, cy - 35.0), v(cx + 60.0, cy + 10.0), 3.0, pal::STEEL, 1.8);
            p.text(cx - 60.0, cy + 55.0, "110", 13.0, pal::USSR, 1.8);
        }
        Event::Hotline => {
            // Красный телефон прямой линии.
            p.rect(cx, cy - 20.0, 110.0, 50.0, pal::USSR, 1.6);
            p.ellipse(cx, cy + 16.0, 64.0, 13.0, Color::srgb(0.70, 0.18, 0.16), 1.7);
            p.circle(cx, cy - 20.0, 17.0, pal::WHITE, 1.7);
            p.circle(cx, cy - 20.0, 6.0, pal::USSR, 1.75);
            let ring = p.text(cx, cy + 52.0, "((·))", 18.0, pal::AMBER, 1.8);
            p.commands.entity(ring).insert(Blink);
        }
        Event::Grain => {
            // Пшеница в кадре, сухогруз на горизонте.
            p.rect(cx, cy - 40.0, 200.0, 70.0, Color::srgb(0.55, 0.45, 0.20), 1.6);
            for i in 0..12 {
                let x = cx - 90.0 + i as f32 * 16.0;
                p.line(v(x, cy - 75.0), v(x + 4.0, cy - 10.0), 2.0, pal::GOLD, 1.7);
                p.ellipse(x + 4.0, cy - 6.0, 3.0, 8.0, pal::GOLD, 1.7);
            }
            p.rect(cx + 20.0, cy + 30.0, 110.0, 14.0, pal::DARK, 1.65);
            p.rect(cx + 60.0, cy + 42.0, 20.0, 12.0, pal::DARK, 1.65);
            p.text(cx - 40.0, cy + 58.0, "20 000 000 т", 12.0, pal::GOLD, 1.8);
        }
        Event::Censorship => {
            // Газетная полоса под чёрными плашками и красный штамп.
            p.rect(cx, cy, 170.0, 130.0, pal::WHITE, 1.6);
            p.text(cx, cy + 50.0, "THE POST", 16.0, pal::DARK, 1.7);
            for i in 0..6 {
                p.rect(cx - 10.0 + (i % 2) as f32 * 20.0, cy + 22.0 - i as f32 * 14.0, 120.0, 8.0, pal::DARK, 1.7);
            }
            let stamp = p.text(cx, cy - 20.0, "ЗАПРЕЩЕНО", 22.0, pal::USSR, 1.8);
            p.commands.entity(stamp).insert(Fade::appear(0.6, 0.3));
        }
    }
}

/// Лунная орбита: Apollo 23 у станции снабжения, пунктир к Марсу.
fn mars_window(p: &mut Painter, text: &Catalog) {
    sky(p, pal::NIGHT, 160, -HH, 81);
    p.circle(0.0, -660.0, 520.0, pal::MOON, 2.0);
    p.circle(-120.0, -190.0, 30.0, pal::MOON_DARK, 2.1);
    p.circle(180.0, -170.0, 18.0, pal::MOON_DARK, 2.1);
    // Марс и путь к нему.
    p.circle(380.0, 185.0, 22.0, pal::MARS.with_alpha(0.25), 2.0);
    p.circle(380.0, 185.0, 13.0, pal::MARS, 2.1);
    p.text(380.0, 152.0, "МАРС", 13.0, pal::MARS, 2.2);
    let path: Vec<Vec2> = (0..=16)
        .map(|i| {
            let t = i as f32 / 16.0;
            let (a, c, b) = (v(-100.0, 20.0), v(150.0, 220.0), v(365.0, 180.0));
            a.lerp(c, t).lerp(c.lerp(b, t), t)
        })
        .collect();
    let route = dashes(p, &path, pal::AMBER, 3.0);
    p.tag(&route, Twinkle(0.0));
    // Станция снабжения и пристыкованный корабль.
    supply_hub(p, -175.0, 20.0, 1.0);
    p.hotspot(-175.0, 20.0, 40.0, text.get("tip.hub").to_string());
    csm(p, -100.0, 20.0, 1.2);
    p.hotspot(-100.0, 20.0, 30.0, text.get("tip.ship").to_string());
    p.text(-120.0, -14.0, "SHADOW-6", 12.0, pal::MUTED, 6.0);
    // Советская станция проходит выше.
    let body = Body::around(1.5);
    orbit_trace(p, v(0.0, 40.0), v(360.0, 130.0), body, pal::USSR, 3.2);
    orbiting_station(p, 0.7, 5, los_tip(text, 5, War::Peace), v(0.0, 40.0), v(360.0, 130.0), 0.22, 1.2, body);
}

/// Развязка «Мимо»: корабль на околосолнечной орбите, Луна позади.
fn bypass(p: &mut Painter) {
    sky(p, pal::NIGHT, 170, -HH, 91);
    for (r, color) in [(190.0, pal::FIRE.with_alpha(0.10)), (130.0, pal::AMBER.with_alpha(0.25)), (90.0, Color::srgb(1.0, 0.92, 0.65))] {
        p.circle(500.0, 0.0, r, color, 2.0);
    }
    earth(p, -360.0, -40.0, 26.0);
    far_moon(p, -300.0, 6.0, 9.0);
    let dot = p.rect(-288.0, 20.0, 4.0, 4.0, pal::USSR, 2.5);
    p.commands.entity(dot).insert(Twinkle(0.0));
    // Орбита вокруг Солнца и корабль на ней.
    let (center, radius) = (v(-20.0, -10.0), v(360.0, 160.0));
    let ring: Vec<Vec2> = (0..=48)
        .map(|i| {
            let a = i as f32 / 48.0 * std::f32::consts::TAU;
            center + radius * v(a.cos(), a.sin())
        })
        .collect();
    dashes(p, &ring, pal::LINE, 1.5);
    let ship = p.group(|p| {
        csm(p, 0.0, 0.0, 1.1);
        let glow = p.tri(-48.0, 0.0, [(0.0, -5.0), (0.0, 5.0), (-22.0, 0.0)], pal::AMBER, 5.9);
        p.commands.entity(glow).insert(Flicker::new(0.0));
    });
    p.orbit(&ship, v(0.0, 0.0), center, radius, 0.18, 3.6);
}

/// Развязка «Через экватор»: Mole-1 приходит к советской базе.
fn handshake(p: &mut Painter, text: &Catalog) {
    sky(p, pal::NIGHT, 120, GROUND, 61);
    earth(p, -330.0, 150.0, 44.0);
    passing_station(p, 5, War::Truce, text);
    moon_ground(p, pal::MOON, 9);
    dashes(p, &[v(-HW, GROUND - 34.0), v(HW, GROUND - 34.0)], pal::MOON_LIGHT, 4.5);
    p.text(-380.0, GROUND - 50.0, "ЭКВАТОР", 12.0, pal::MUTED, 4.6);
    // След робота тянется из-за левого края.
    for i in 0..14 {
        p.rect(-HW + 10.0 + i as f32 * 16.0, GROUND - 2.0, 8.0, 3.0, pal::MOON_DARK, 4.4);
    }
    mole(p, -200.0, GROUND - 6.0);
    flag(p, -120.0, GROUND - 12.0, pal::US);
    flag(p, 150.0, GROUND - 12.0, pal::USSR);
    astronaut(p, -14.0, GROUND - 18.0, pal::WHITE);
    for x in [14.0, 60.0, 92.0] {
        astronaut(p, x, GROUND - 18.0, Color::srgb(0.95, 0.80, 0.72));
    }
    p.rect(260.0, GROUND + 14.0, 60.0, 28.0, pal::USSR, 8.0);
    p.rect(260.0, GROUND + 34.0, 4.0, 14.0, pal::STEEL, 8.0);
}

/// Развязка «Подписка»: зал суда, рассекреченное обязательство молчать.
fn tribunal(p: &mut Painter) {
    let wood = Color::srgb(0.22, 0.15, 0.10);
    p.rect(0.0, 0.0, HW * 2.0, HH * 2.0, Color::srgb(0.14, 0.10, 0.08), 0.0);
    for i in 0..12 {
        p.rect(-HW + 40.0 + i as f32 * 80.0, 60.0, 4.0, 380.0, wood, 0.5);
    }
    p.circle(-100.0, 150.0, 48.0, pal::GOLD, 1.0);
    p.circle(-100.0, 150.0, 40.0, wood, 1.1);
    p.text(-100.0, 150.0, "US", 22.0, pal::GOLD, 1.2);
    p.rect(-100.0, 0.0, 440.0, 120.0, Color::srgb(0.34, 0.23, 0.15), 2.0);
    p.rect(-100.0, 62.0, 460.0, 8.0, Color::srgb(0.45, 0.31, 0.19), 2.1);
    // Свидетель: астронавт, давший подписку.
    p.rect(-330.0, -130.0, 70.0, 60.0, Color::srgb(0.34, 0.23, 0.15), 3.0);
    astronaut(p, -330.0, -110.0, pal::WHITE);
    // Документ с грифом.
    p.rect(250.0, 10.0, 220.0, 280.0, Color::srgb(0.93, 0.92, 0.88), 4.0);
    p.text(250.0, 110.0, "ОБЯЗАТЕЛЬСТВО\nО НЕРАЗГЛАШЕНИИ", 14.0, pal::DARK, 4.1);
    for i in 0..8 {
        p.rect(250.0, 60.0 - i as f32 * 18.0, 170.0, 3.0, pal::MUTED, 4.1);
    }
    let stamp = p.text(250.0, -40.0, "РАССЕКРЕЧЕНО", 18.0, pal::USSR, 4.2);
    p.commands.entity(stamp).insert(Fade::appear(0.8, 0.4));
    // Публика.
    let mut dice = Dice(5);
    for i in 0..20 {
        let x = -440.0 + i as f32 * 46.0 + dice.range(-6.0, 6.0);
        p.circle(x, -200.0, 12.0, pal::DARK, 5.0);
        p.rect(x, -230.0, 30.0, 40.0, pal::DARK, 5.0);
    }
}

// ═════════════════════ КРИЗИСЫ И НОВЫЕ РАЗВЯЗКИ ═════════════════════════

/// Солдат в каске. Точка (x, y) — уровень земли.
fn soldier(p: &mut Painter, x: f32, y: f32, z: f32) {
    let olive = Color::srgb(0.20, 0.24, 0.17);
    p.rect(x, y + 14.0, 12.0, 20.0, olive, z);
    p.ellipse(x, y + 29.0, 7.0, 4.5, olive, z);
    p.circle(x, y + 26.0, 5.0, pal::DARK, z - 0.01);
    p.line(v(x + 4.0, y + 8.0), v(x + 12.0, y + 30.0), 2.0, pal::DARK, z + 0.01);
}

/// Бронетранспортёр. Точка (x, y) — уровень земли.
fn apc(p: &mut Painter, x: f32, y: f32, z: f32) {
    let olive = Color::srgb(0.24, 0.28, 0.20);
    p.rect(x, y + 18.0, 110.0, 24.0, olive, z);
    p.tri(x + 55.0, y + 6.0, [(0.0, 0.0), (0.0, 24.0), (16.0, 12.0)], olive, z);
    p.rect(x - 10.0, y + 36.0, 34.0, 12.0, olive, z);
    p.line(v(x + 6.0, y + 40.0), v(x + 40.0, y + 44.0), 3.0, pal::DARK, z);
    for i in 0..4 {
        p.circle(x - 38.0 + i as f32 * 25.0, y + 6.0, 8.0, pal::DARK, z + 0.01);
    }
    p.text(x - 20.0, y + 18.0, "US ARMY", 9.0, pal::MUTED, z + 0.02);
}

/// Кризис власти: ночной Капитолий, войска, прожекторы.
fn uprising(p: &mut Painter, junta: bool) {
    impeached(p);
    p.rect(0.0, 0.0, HW * 2.0, HH * 2.0, pal::NIGHT.with_alpha(0.55), 6.0);
    for (x, base, speed) in [(-380.0, -0.5, 0.6), (380.0, 0.5, 0.8), (0.0, 0.0, 0.4)] {
        let beam = p.tri(x, -HH, [(-6.0, 0.0), (6.0, 0.0), (0.0, 520.0)], pal::WHITE.with_alpha(0.10), 6.5);
        p.commands.entity(beam).insert(Sweep { base, amp: 0.3, speed });
    }
    apc(p, -250.0, -HH + 6.0, 7.0);
    apc(p, 270.0, -HH + 6.0, 7.0);
    for i in 0..9 {
        soldier(p, -120.0 + i as f32 * 30.0, -HH + 6.0, 7.2);
    }
    let sign = if junta { "КОМЕНДАНТСКИЙ ЧАС" } else { "ЧРЕЗВЫЧАЙНОЕ ПОЛОЖЕНИЕ?" };
    let label = p.text(0.0, 200.0, sign, 22.0, pal::USSR, 8.0);
    p.commands.entity(label).insert(Blink);
}

/// Пустая касса: казначейство в сумерках и красный штамп.
fn bankrupt(p: &mut Painter) {
    funding(p);
    p.rect(0.0, 0.0, HW * 2.0, HH * 2.0, pal::NIGHT.with_alpha(0.5), 7.0);
    p.rect(320.0, -150.0, 170.0, 26.0, pal::DARK, 7.1);
    p.text(320.0, -150.0, "$ 0", 20.0, pal::USSR, 7.2);
    p.rect(-110.0, 20.0, 360.0, 64.0, pal::USSR, 7.3);
    p.rect(-110.0, 20.0, 350.0, 54.0, pal::NIGHT, 7.31);
    let stamp = p.text(-110.0, 20.0, "СЧЕТА НЕ ОПЛАЧЕНЫ", 26.0, pal::USSR, 7.32);
    p.commands.entity(stamp).insert(Fade::appear(0.4, 0.3));
}

/// Командный пункт NORAD: карта мира, траектории ракет, ДЕФКОН 1.
fn war_room(p: &mut Painter) {
    p.rect(0.0, 0.0, HW * 2.0, HH * 2.0, Color::srgb(0.03, 0.05, 0.06), 0.0);
    let screen = Color::srgb(0.02, 0.10, 0.10);
    p.rect(0.0, 40.0, 820.0, 360.0, pal::LINE, 1.0);
    p.rect(0.0, 40.0, 808.0, 348.0, screen, 1.1);
    let grid = Color::srgb(0.05, 0.22, 0.20);
    for i in 0..17 {
        p.rect(-400.0 + i as f32 * 50.0, 40.0, 1.0, 348.0, grid, 1.2);
    }
    for i in 0..8 {
        p.rect(0.0, -130.0 + i as f32 * 50.0, 808.0, 1.0, grid, 1.2);
    }
    // Материки контурами из эллипсов: Америка слева, Евразия справа.
    let land = Color::srgb(0.10, 0.42, 0.34);
    for (x, y, rx, ry) in [
        (-250.0, 110.0, 95.0, 60.0),
        (-215.0, 30.0, 45.0, 40.0),
        (-170.0, -60.0, 30.0, 55.0),
        (150.0, 120.0, 170.0, 55.0),
        (90.0, 50.0, 60.0, 40.0),
        (230.0, 50.0, 70.0, 35.0),
        (30.0, -40.0, 45.0, 55.0),
    ] {
        p.ellipse(x, y, rx, ry, land, 1.3);
    }
    for (x, y, label, color) in [(-215.0, 40.0, "ВАШИНГТОН", pal::US), (150.0, 115.0, "МОСКВА", pal::USSR), (210.0, 75.0, "БАЙКОНУР", pal::USSR)] {
        let dot = p.circle(x, y, 5.0, color, 1.5);
        p.commands.entity(dot).insert(Flicker::new(x));
        p.text(x, y - 16.0, label, 11.0, color, 1.5);
    }
    // Траектории: дуги через полюс.
    let arc = |a: Vec2, b: Vec2| -> Vec<Vec2> {
        let c = (a + b) / 2.0 + v(0.0, 150.0);
        (0..=14).map(|i| { let t = i as f32 / 14.0; a.lerp(c, t).lerp(c.lerp(b, t), t) }).collect()
    };
    for (a, b, color) in [(v(-215.0, 40.0), v(210.0, 75.0), pal::US), (v(150.0, 115.0), v(-215.0, 40.0), pal::USSR), (v(160.0, 100.0), v(-250.0, 100.0), pal::USSR)] {
        let path = dashes(p, &arc(a, b), color, 1.6);
        p.tag(&path, Twinkle(a.x));
    }
    // Табло боевой готовности.
    for i in 0..5 {
        let level = 5 - i;
        let x = -160.0 + i as f32 * 80.0;
        let lit = level == 1;
        let color = if lit { pal::USSR } else { pal::LINE };
        p.rect(x, -185.0, 66.0, 40.0, color, 2.0);
        let label = p.text(x, -185.0, &level.to_string(), 24.0, if lit { pal::WHITE } else { pal::MUTED }, 2.1);
        if lit {
            p.commands.entity(label).insert(Blink);
        }
    }
    p.text(-300.0, -185.0, "DEFCON", 18.0, pal::AMBER, 2.1);
    p.rect(330.0, -185.0, 90.0, 40.0, pal::DARK, 2.0);
    p.rect(330.0, -185.0, 70.0, 26.0, Color::srgb(0.12, 0.10, 0.08), 2.1);
    p.circle(312.0, -185.0, 5.0, pal::GOLD, 2.2);
    p.circle(348.0, -185.0, 5.0, pal::GOLD, 2.2);
}

/// Кремль: зал Политбюро и два макета на столе — крепость и Марс.
fn politburo(p: &mut Painter) {
    let wall = Color::srgb(0.34, 0.08, 0.07);
    p.rect(0.0, 0.0, HW * 2.0, HH * 2.0, wall, 0.0);
    for i in 0..9 {
        p.rect(-400.0 + i as f32 * 100.0, 60.0, 10.0, 380.0, Color::srgb(0.28, 0.06, 0.05), 0.1);
    }
    // Портрет в раме и люстра.
    p.rect(0.0, 160.0, 120.0, 140.0, pal::GOLD, 0.5);
    p.rect(0.0, 160.0, 104.0, 124.0, Color::srgb(0.15, 0.12, 0.10), 0.6);
    p.circle(0.0, 175.0, 26.0, Color::srgb(0.30, 0.26, 0.22), 0.7);
    p.rect(0.0, 130.0, 60.0, 40.0, Color::srgb(0.20, 0.18, 0.16), 0.7);
    p.text(0.0, 72.0, "КРЕМЛЬ · ПОЛИТБЮРО ЦК", 15.0, pal::GOLD, 0.8);
    // Длинный стол в перспективе и стулья.
    trapezoid(p, 0.0, -HH, 760.0, 260.0, 230.0, Color::srgb(0.30, 0.20, 0.12), 1.0);
    trapezoid(p, 0.0, -HH, 700.0, 230.0, 220.0, Color::srgb(0.16, 0.30, 0.18), 1.1);
    for i in 0..6 {
        let k = i as f32 / 5.0;
        let y = -200.0 + k * 170.0;
        let half = 350.0 - k * 220.0 + 30.0;
        for side in [-1.0, 1.0] {
            p.rect(side * half, y, 30.0 - k * 14.0, 46.0 - k * 22.0, Color::srgb(0.12, 0.08, 0.06), 1.2 + k * 0.01);
        }
    }
    // Слева крепость Ветрова, справа Марс Лаврова.
    p.circle(-140.0, -60.0, 46.0, pal::STEEL, 2.0);
    p.rect(-140.0, -82.0, 100.0, 46.0, Color::srgb(0.16, 0.30, 0.18), 2.01);
    p.rect(-140.0, -60.0, 92.0, 6.0, pal::DARK, 2.02);
    flag(p, -132.0, -26.0, pal::USSR);
    p.text(-140.0, -110.0, "КРЕПОСТЬ", 13.0, pal::WHITE, 2.1);
    p.circle(140.0, -40.0, 36.0, pal::MARS, 2.0);
    p.ellipse(130.0, -30.0, 14.0, 8.0, Color::srgb(0.62, 0.26, 0.16), 2.01);
    p.rect(140.0, -85.0, 4.0, 18.0, pal::GOLD, 2.0);
    p.text(140.0, -110.0, "МАРС", 13.0, pal::WHITE, 2.1);
    // Ветров с погонами и Лавров.
    for (x, gold) in [(-330.0, true), (330.0, false)] {
        p.circle(x, 40.0, 20.0, Color::srgb(0.85, 0.72, 0.62), 3.0);
        p.rect(x, -20.0, 70.0, 100.0, if gold { Color::srgb(0.20, 0.26, 0.20) } else { pal::DARK }, 3.0);
        if gold {
            for side in [-1.0, 1.0] {
                p.rect(x + side * 30.0, 26.0, 18.0, 6.0, pal::GOLD, 3.1);
            }
        }
    }
    p.text(-330.0, -90.0, "ВЕТРОВ", 13.0, pal::GOLD, 3.2);
    p.text(330.0, -90.0, "ЛАВРОВ", 13.0, pal::GOLD, 3.2);
}

/// «Два флага у Марса»: американский и советский корабли летят рядом.
fn joint_mars(p: &mut Painter) {
    sky(p, pal::NIGHT, 170, -HH, 33);
    earth(p, -380.0, -120.0, 34.0);
    far_moon(p, -300.0, -80.0, 12.0);
    p.circle(330.0, 80.0, 74.0, pal::MARS.with_alpha(0.25), 2.0);
    p.circle(330.0, 80.0, 66.0, pal::MARS, 2.1);
    p.ellipse(310.0, 100.0, 30.0, 14.0, Color::srgb(0.62, 0.26, 0.16), 2.2);
    p.ellipse(330.0, 140.0, 22.0, 6.0, pal::WHITE, 2.2);
    let route = dashes(p, &[v(-300.0, -70.0), v(-60.0, 0.0), v(250.0, 70.0)], pal::GOLD.with_alpha(0.6), 1.5);
    p.tag(&route, Twinkle(0.0));
    let convoy = p.group(|p| {
        csm(p, 0.0, 26.0, 1.3);
        station_module(p, 0.0, -24.0, 1.4, 3);
        p.rect(-24.0, 0.0, 2.0, 50.0, pal::STEEL, 5.9);
        for (y, color) in [(26.0, pal::US), (-24.0, pal::USSR)] {
            let glow = p.tri(-58.0, y, [(0.0, -6.0), (0.0, 6.0), (-28.0, 0.0)], pal::AMBER, 5.9);
            p.commands.entity(glow).insert(Flicker::new(y));
            p.rect(30.0, y + 14.0, 14.0, 8.0, color, 6.3);
        }
    });
    p.orbit(&convoy, v(0.0, 0.0), v(-40.0, -10.0), v(24.0, 8.0), 0.4, 0.0);
    p.text(-40.0, -90.0, "СОЮЗ — АПОЛЛОН · 2", 20.0, pal::GOLD, 6.0);
}

/// «Крепость на песке»: законсервированная советская база.
fn collapse(p: &mut Painter, state: &GameState, text: &Catalog) {
    far_side(p, state, true, text);
    p.rect(0.0, 0.0, HW * 2.0, HH * 2.0, pal::NIGHT.with_alpha(0.55), 20.0);
    let mut dice = Dice(81);
    for _ in 0..40 {
        p.rect(dice.range(-HW, HW), dice.range(-HH, GROUND), 2.0, 2.0, pal::WHITE.with_alpha(0.35), 20.5);
    }
    let lamp = p.circle(260.0, GROUND + 50.0, 4.0, pal::AMBER, 21.0);
    p.commands.entity(lamp).insert(Blink);
    p.rect(60.0, GROUND + 120.0, 300.0, 34.0, pal::DARK, 21.0);
    p.text(60.0, GROUND + 120.0, "ЗАКОНСЕРВИРОВАНО · 1981", 16.0, pal::AMBER, 21.1);
}

/// «Лунный договор»: Луна под эмблемой ООН, обе базы на общем канале.
fn treaty(p: &mut Painter, text: &Catalog) {
    cold_peace(p, text);
    let un = Color::srgb(0.36, 0.62, 0.85);
    // Граница по меридиану стёрта: договор делит Луну на всех.
    p.rect(0.0, 0.0, 6.0, 340.0, pal::MOON, 3.05);
    for r in [200.0, 190.0] {
        p.circle(0.0, 0.0, r, un.with_alpha(0.14), 1.9);
    }
    // Лавровая ветвь: листья по дуге с двух сторон.
    for i in 0..9 {
        let a = -1.2 + i as f32 * 0.3;
        for side in [-1.0, 1.0] {
            p.ellipse(side * 195.0 * a.cos(), 195.0 * a.sin(), 9.0, 4.0, un, 2.5);
        }
    }
    dashes(p, &[v(-60.0, -70.0), v(80.0, 30.0)], pal::GREEN, 3.3);
    p.text(0.0, -205.0, "ДОГОВОР О ЛУНЕ · ЖЕНЕВА", 18.0, un, 3.4);
}

/// «Хозяин Луны» и «Железная Луна»: обломки станции сгорают метеорами.
fn moon_victory(p: &mut Painter, state: &GameState, iron: bool, text: &Catalog) {
    surface(p, state, false, text);
    let mut dice = Dice(64);
    for i in 0..12 {
        let (x, y) = (dice.range(-HW, HW), dice.range(40.0, HH - 20.0));
        let len = dice.range(18.0, 46.0);
        let streak = p.line(v(x, y), v(x + len, y + len * 0.5), 2.0, pal::FIRE, 2.6);
        p.commands.entity(streak).insert(Twinkle(i as f32));
        p.circle(x, y, 2.5, pal::AMBER, 2.7);
    }
    flag(p, -180.0, GROUND - 6.0, pal::US);
    if iron {
        p.rect(0.0, 0.0, HW * 2.0, HH * 2.0, Color::srgb(0.08, 0.09, 0.07).with_alpha(0.45), 20.0);
        for (x, base) in [(-400.0, -0.4), (400.0, 0.4)] {
            let beam = p.tri(x, GROUND - 40.0, [(-5.0, 0.0), (5.0, 0.0), (0.0, 420.0)], pal::WHITE.with_alpha(0.10), 20.5);
            p.commands.entity(beam).insert(Sweep { base, amp: 0.25, speed: 0.5 });
        }
        p.rect(-150.0, GROUND + 6.0, 20.0, 10.0, Color::srgb(0.20, 0.24, 0.17), 21.0);
        p.rect(0.0, 200.0, 420.0, 34.0, pal::DARK, 21.0);
        p.text(0.0, 200.0, "ВОЕННАЯ АДМИНИСТРАЦИЯ «ТИХО»", 16.0, pal::AMBER, 21.1);
    }
}

/// «Красная Луна»: «Тихо» эвакуирована, над ней советские флаги.
fn red_moon(p: &mut Painter, state: &GameState, text: &Catalog) {
    surface(p, state, false, text);
    p.rect(0.0, 0.0, HW * 2.0, HH * 2.0, pal::USSR.with_alpha(0.10), 20.0);
    for x in [-300.0, -60.0, 180.0, 380.0] {
        flag(p, x, GROUND - 24.0, pal::USSR);
    }
    for x in [-20.0, 10.0, 40.0] {
        astronaut(p, x, GROUND - 30.0, Color::srgb(0.95, 0.80, 0.72));
    }
    p.rect(0.0, 200.0, 300.0, 34.0, pal::DARK, 21.0);
    p.text(0.0, 200.0, "ТИХО · ЭВАКУИРОВАНО", 16.0, pal::USSR, 21.1);
}

/// Точки ядерных вспышек на диске Земли: Байконур и Флорида первыми.
const BLASTS: [(f32, f32); 14] = [
    (0.45, 0.35), (-0.55, 0.15), (0.30, 0.45), (-0.45, 0.30), (0.55, 0.20), (-0.30, 0.05), (0.20, 0.30),
    (-0.60, 0.35), (0.62, 0.40), (0.05, 0.50), (-0.15, 0.40), (0.40, 0.05), (-0.40, -0.05), (0.15, 0.15),
];

/// Земля в лунном небе. На ней вспыхивают взрывы: два или все сразу.
fn earth_from_moon(p: &mut Painter, total: bool) {
    sky(p, pal::NIGHT, 140, GROUND, 97);
    let (ex, ey, r) = (40.0, 100.0, 110.0);
    earth(p, ex, ey, r);
    let count = if total { BLASTS.len() } else { 2 };
    for (i, (dx, dy)) in BLASTS.iter().take(count).enumerate() {
        let glow = p.circle(ex + dx * r, ey + dy * r, 9.0, pal::WHITE, 2.6);
        p.commands.entity(glow).insert(Twinkle(i as f32 * 1.3));
        p.circle(ex + dx * r, ey + dy * r, 18.0, pal::FIRE.with_alpha(0.35), 2.55);
    }
    if total {
        let mut dice = Dice(5);
        for _ in 0..10 {
            p.ellipse(ex + dice.range(-0.8, 0.8) * r, ey + dice.range(-0.6, 0.7) * r, dice.range(30.0, 60.0), dice.range(10.0, 22.0), pal::STEEL.with_alpha(0.55), 2.7);
        }
    }
    moon_ground(p, pal::MOON, 23);
    for (i, x) in [-260.0, -230.0, -200.0, -170.0].into_iter().enumerate() {
        astronaut(p, x, GROUND - 16.0 - (i % 2) as f32 * 6.0, pal::WHITE);
    }
    if total {
        for x in [200.0, 230.0, 260.0] {
            astronaut(p, x, GROUND - 18.0, Color::srgb(0.95, 0.80, 0.72));
        }
    }
}

// ══════════════════════════ АНИМАЦИИ МЕЖДУ ХОДАМИ ═══════════════════════

/// Рисует анимацию и возвращает её длительность в секундах.
pub fn draw_cutscene(p: &mut Painter, cutscene: Cutscene, state: &GameState, text: &Catalog) -> f32 {
    match cutscene {
        Cutscene::Chapter(book) => chapter(p, book, text),
        Cutscene::Launch(_) => saturn_launch(p),
        Cutscene::Abort(_) => abort(p, text),
        Cutscene::Landing(mission) => landing(p, mission, state, text),
        Cutscene::N1Launch(number) => n1_launch(p, number, text),
        Cutscene::SovietModule(number) => docking(p, number, text),
        Cutscene::Intercept => intercept(p, text),
        Cutscene::SovietLanding => soviet_landing(p, state, text),
        Cutscene::Strike => strike(p, text),
        Cutscene::Crackdown => crackdown(p, text),
        Cutscene::Flash => flash(p, state, text),
    }
}

/// Заставка книги: номер, название и эпиграф из трилогии.
fn chapter(p: &mut Painter, book: u8, text: &Catalog) -> f32 {
    let duration = 4.8;
    sky(p, pal::NIGHT, 90, -HH, 100 + book as u32);
    // Линия раскрывается от центра в стороны.
    for side in [-1.0, 1.0] {
        let half = p.rect(0.0, 22.0, 220.0, 2.0, pal::AMBER, 2.0);
        p.commands.entity(half).insert(Slide::new(v(side * 110.0, 0.0), 0.9, 0.3));
    }
    let title = p.text(0.0, 60.0, text.get(&format!("chapter.{book}.title")), 28.0, pal::TEXT, 3.0);
    p.commands.entity(title).insert(Fade::appear(0.5, 0.6));
    let epigraph = p.text(0.0, -20.0, text.get(&format!("chapter.{book}.text")), 18.0, pal::MUTED, 3.0);
    p.commands.entity(epigraph).insert((Fade::appear(1.3, 0.8), TextLayout::new_with_justify(JustifyText::Center)));
    let years = p.text(0.0, -110.0, text.get(&format!("chapter.{book}.years")), 15.0, pal::AMBER, 3.0);
    p.commands.entity(years).insert(Fade::appear(2.0, 0.6));
    curtain(p, duration);
    duration
}

/// Ночная площадка 39-A без ракеты: общий фон старта и аварии.
fn night_pad(p: &mut Painter) {
    sky(p, pal::NIGHT, 90, -150.0, 11);
    far_moon(p, 320.0, 160.0, 30.0);
    p.rect(0.0, -165.0, HW * 2.0, 30.0, pal::SEA, 2.0);
    p.rect(0.0, -212.0, HW * 2.0, 66.0, Color::srgb(0.10, 0.13, 0.12), 3.0);
    p.rect(-30.0, -176.0, 260.0, 12.0, pal::STEEL, 5.0);
    tower(p, 50.0, -170.0, 250.0);
}

/// Старт Saturn V: отсчёт, зажигание, отрыв, дымный след.
fn saturn_launch(p: &mut Painter) -> f32 {
    let duration = 5.6;
    let (x, base) = (-40.0, -170.0);
    let (ignite, lift, acc) = (1.9, 2.3, 120.0);
    night_pad(p);
    letterbox(p, 40.0);
    // Обратный отсчёт: каждая цифра проявляется и гаснет.
    for (i, digit) in ["3", "2", "1"].into_iter().enumerate() {
        let from = 0.3 + i as f32 * 0.55;
        let label = p.text(-300.0, 60.0, digit, 120.0, pal::AMBER, 12.0);
        p.commands.entity(label).insert(Fade::window(from, from + 0.55, 0.12));
    }
    let ship = p.group(|p| {
        rocket(p, x, base, 1.0);
        let fire = flame(p, x, base, 1.6);
        p.tag(&fire, Reveal { from: ignite, to: 99.0 });
    });
    p.tag(&ship, Motion { vel: Vec2::ZERO, acc: v(0.0, acc), delay: lift });
    let glow = p.ellipse(x, base, 130.0, 34.0, pal::FIRE.with_alpha(0.55), 6.0);
    p.commands.entity(glow).insert(Fade::keys(&[(ignite, 0.0), (ignite + 0.3, 1.0), (lift + 1.2, 0.5), (lift + 2.5, 0.0)]));
    ground_smoke(p, x, base - 2.0, ignite, 16, 130.0, 5);
    exhaust_trail(p, x, base, lift, acc, duration, 7);
    light(p, pal::AMBER.with_alpha(0.07), &[(ignite, 0.0), (ignite + 0.3, 1.0), (lift + 2.0, 0.0)]);
    p.shake(ignite, lift + 1.8, 5.0);
    curtain(p, duration);
    duration
}

/// Авария на старте: ракета взрывается, башня САС уносит капсулу,
/// капсула раскрывает парашют. Пунктир показывает путь экипажа.
fn abort(p: &mut Painter, text: &Catalog) -> f32 {
    let duration = 6.6;
    let (x, base) = (-40.0, -170.0);
    let (ignite, lift, acc, boom): (f32, f32, f32, f32) = (0.5, 0.9, 70.0, 2.6);
    let rise = 0.5 * acc * (boom - lift).powi(2);
    night_pad(p);
    letterbox(p, 40.0);
    let ship = p.group(|p| {
        rocket(p, x, base, 1.0);
        let fire = flame(p, x, base, 1.6);
        p.tag(&fire, Reveal { from: ignite, to: 99.0 });
    });
    p.tag(&ship, Motion { vel: Vec2::ZERO, acc: v(0.0, acc), delay: lift });
    p.tag(&ship, Reveal { from: 0.0, to: boom });
    ground_smoke(p, x, base - 2.0, ignite, 12, 110.0, 15);
    exhaust_trail(p, x, base, lift, acc, boom, 17);

    // Взрыв на месте второй ступени.
    let blast = v(x, base + rise + 100.0);
    for (radius, color, z) in [(60.0, pal::FIRE, 10.0), (36.0, pal::AMBER, 10.1), (16.0, pal::WHITE, 10.2)] {
        let ball = p.circle(blast.x, blast.y, radius, color, z);
        p.commands.entity(ball).insert((
            Reveal { from: boom, to: 99.0 },
            Grow(0.6),
            Fade::keys(&[(boom + 0.3, 1.0), (boom + 2.2, 0.0)]),
        ));
    }
    let mut dice = Dice(23);
    for _ in 0..22 {
        let angle = dice.range(0.0, std::f32::consts::TAU);
        let speed = dice.range(60.0, 220.0);
        let piece = p.rect(blast.x, blast.y, dice.range(4.0, 12.0), dice.range(3.0, 6.0), pal::STEEL, 9.5);
        p.commands.entity(piece).insert((
            Reveal { from: boom, to: 99.0 },
            Motion { vel: v(angle.cos(), angle.sin()) * speed, acc: v(0.0, -120.0), delay: boom },
            Fade::keys(&[(boom + 1.5, 1.0), (boom + 3.0, 0.0)]),
        ));
    }

    // Башня САС уводит капсулу по дуге, затем парашют.
    let capsule_start = v(x, base + rise + 207.0);
    let (vel, gravity, burn) = (v(170.0, 60.0), v(0.0, -80.0), 1.3);
    let at = |t: f32| capsule_start + vel * t + gravity * (0.5 * t * t);
    let arc: Vec<Vec2> = (0..=13).map(|i| at(i as f32 * 0.1)).collect();
    let path = dashes(p, &arc, pal::AMBER, 9.0);
    p.tag(&path, Reveal { from: boom, to: 99.0 });
    let pod = |p: &mut Painter, c: Vec2| {
        p.tri(c.x, c.y - 7.0, [(-7.0, 0.0), (7.0, 0.0), (0.0, 12.0)], pal::STEEL, 11.0);
    };
    let escape = p.group(|p| {
        pod(p, capsule_start);
        p.rect(capsule_start.x, capsule_start.y + 14.0, 2.0, 16.0, pal::STEEL, 11.0);
        let jet = p.tri(capsule_start.x, capsule_start.y + 8.0, [(-3.0, 0.0), (3.0, 0.0), (-16.0, -12.0)], pal::FIRE, 10.9);
        p.commands.entity(jet).insert(Flicker::new(0.0));
    });
    p.tag(&escape, Reveal { from: boom, to: boom + burn });
    p.tag(&escape, Motion { vel, acc: gravity, delay: boom });
    let landing_point = at(burn);
    let chute = p.group(|p| {
        pod(p, landing_point);
        p.ellipse(landing_point.x, landing_point.y + 40.0, 30.0, 14.0, pal::FIRE, 11.0);
        p.rect(landing_point.x, landing_point.y + 32.0, 60.0, 4.0, pal::NIGHT, 11.1);
        p.line(landing_point + v(-26.0, 32.0), landing_point + v(-4.0, 4.0), 1.0, pal::WHITE, 10.9);
        p.line(landing_point + v(26.0, 32.0), landing_point + v(4.0, 4.0), 1.0, pal::WHITE, 10.9);
    });
    p.tag(&chute, Reveal { from: boom + burn, to: 99.0 });
    p.tag(&chute, Motion { vel: v(25.0, -45.0), acc: Vec2::ZERO, delay: boom + burn });
    flash_label(p, -250.0, 150.0, text.get("cut.alive"), 22.0, pal::GREEN, boom + burn + 0.3, duration - 0.2);

    light(p, pal::FIRE.with_alpha(0.25), &[(boom, 0.0), (boom + 0.1, 1.0), (boom + 1.4, 0.0)]);
    p.shake(boom, boom + 1.5, 10.0);
    curtain(p, duration);
    duration
}

/// Посадка у Тихо: пунктир от точки входа к рамке цели, касание, пыль.
fn landing(p: &mut Painter, mission: Mission, state: &GameState, text: &Catalog) -> f32 {
    let duration = 4.8;
    let touchdown = 2.4;
    sky(p, pal::NIGHT, 120, GROUND, 3);
    earth(p, -330.0, 150.0, 44.0);
    moon_ground(p, pal::MOON, 5);
    for delivered in state.delivered.iter().filter(|&m| m != mission) {
        base_module(p, delivered);
    }
    let (scale, crew, hull) = match mission {
        Mission::Apollo22 => (1.5, 4, pal::WHITE),
        Mission::Reserve => (1.0, 0, pal::GOLD),
        _ => (1.0, 2, pal::STEEL),
    };
    let goal = v(330.0, GROUND - 6.0);
    let start = goal + v(-180.0, 330.0);
    let path = dashes(p, &[start, goal], pal::AMBER.with_alpha(0.7), 7.5);
    p.tag(&path, Fade::keys(&[(touchdown, 1.0), (touchdown + 0.3, 0.0)]));
    target_mark(p, goal.x, goal.y + 10.0, pal::AMBER, text.get("cut.target"), touchdown);
    let ship = p.group(|p| {
        lander(p, start.x, start.y, scale, hull, crew);
        let fire = flame(p, start.x, start.y + 6.0 + 6.0 * scale, 0.6 * scale);
        p.tag(&fire, Reveal { from: 0.0, to: touchdown + 0.1 });
    });
    p.tag(&ship, Slide::new(goal - start, touchdown, 0.0));
    lunar_dust(p, goal, touchdown);
    flash_label(p, goal.x - 40.0, goal.y + 140.0, text.get("cut.contact"), 22.0, pal::GREEN, touchdown, duration - 0.3);
    p.shake(touchdown, touchdown + 0.35, 3.0);
    curtain(p, duration);
    duration
}

/// Пыль из-под опор в момент касания.
fn lunar_dust(p: &mut Painter, at: Vec2, from: f32) {
    let mut dice = Dice(8);
    for i in 0..12 {
        let side = if i % 2 == 0 { -1.0 } else { 1.0 };
        let start = from + i as f32 * 0.03;
        let dust = p.circle(at.x + side * 20.0, at.y + 2.0, dice.range(6.0, 12.0), pal::MOON_LIGHT.with_alpha(0.55), 9.0);
        p.commands.entity(dust).insert((
            Reveal { from: start, to: 99.0 },
            Motion { vel: v(side * dice.range(50.0, 120.0), dice.range(0.0, 14.0)), acc: Vec2::ZERO, delay: start },
            Grow(0.4),
            Fade::keys(&[(start + 0.8, 1.0), (start + 2.0, 0.0)]),
        ));
    }
}

/// Советская сверхтяжёлая Н-1. Точка (x, y) — основание. Корпус сужается
/// кверху; снизу видна часть из тридцати сопел первой ступени.
fn n1(p: &mut Painter, x: f32, y: f32, s: f32) {
    let z = 8.0;
    let light = Color::srgb(0.86, 0.87, 0.85);
    let shade = Color::srgb(0.66, 0.68, 0.68);
    // Три ступени-трапеции: высота низа, высота верха, ширина низа и верха.
    for (bottom, top, wide, narrow) in [(0.0, 120.0, 76.0, 52.0), (120.0, 190.0, 52.0, 38.0), (190.0, 228.0, 38.0, 30.0)] {
        trapezoid(p, x, y + bottom * s, wide * s, narrow * s, (top - bottom) * s, light, z);
        p.rect(x, y + top * s, narrow * s, 5.0 * s, pal::DARK, z + 0.2);
    }
    // Тень по правому борту и тёмный пояс у основания.
    trapezoid(p, x + 16.0 * s, y, 14.0 * s, 8.0 * s, 120.0 * s, shade, z + 0.05);
    p.rect(x, y + 10.0 * s, 74.0 * s, 8.0 * s, shade, z + 0.1);
    // Головной блок с лунными кораблями и башня спасения.
    p.rect(x, y + 258.0 * s, 26.0 * s, 60.0 * s, light, z + 0.1);
    p.rect(x, y + 236.0 * s, 26.0 * s, 4.0 * s, pal::DARK, z + 0.2);
    p.tri(x, y + 288.0 * s, [(-13.0 * s, 0.0), (13.0 * s, 0.0), (0.0, 24.0 * s)], light, z + 0.1);
    p.rect(x, y + 322.0 * s, 2.5 * s, 22.0 * s, pal::STEEL, z + 0.1);
    p.text(x - 14.0 * s, y + 66.0 * s, "С\nС\nС\nР", 11.0 * s, pal::USSR, z + 0.3);
    for i in 0..12 {
        let nx = x + (i as f32 - 5.5) * 6.0 * s;
        p.rect(nx, y - 4.0 * s, 4.5 * s, 8.0 * s, pal::DARK, z);
    }
}

fn trapezoid(p: &mut Painter, x: f32, y: f32, bottom: f32, top: f32, height: f32, color: Color, z: f32) {
    let (b, t) = (bottom / 2.0, top / 2.0);
    p.tri(x, y, [(-b, 0.0), (b, 0.0), (t, height)], color, z);
    p.tri(x, y, [(-b, 0.0), (t, height), (-t, height)], color, z);
}

/// Пламя Н-1: сопла загораются от центра к краям, затем сливаются в факел.
fn n1_flames(p: &mut Painter, x: f32, y: f32, s: f32, ignite: f32) -> Group {
    p.group(|p| {
        for i in 0..12 {
            let offset = (i as f32 - 5.5) * 6.0 * s;
            let from = ignite + (offset.abs() / (6.0 * s)) * 0.06;
            let jet = p.tri(x + offset, y - 8.0 * s, [(-3.0 * s, 0.0), (3.0 * s, 0.0), (0.0, -34.0 * s)], pal::AMBER, 7.1);
            p.commands.entity(jet).insert((Flicker::new(i as f32), Reveal { from, to: 99.0 }));
        }
        let plume = ignite + 0.45;
        for (wide, long, color, z) in [(44.0, 170.0, pal::FIRE, 7.0), (28.0, 110.0, pal::AMBER, 7.05), (14.0, 60.0, pal::WHITE, 7.08)] {
            let fire = p.tri(x, y - 8.0 * s, [(-wide * s, 0.0), (wide * s, 0.0), (0.0, -long * s)], color, z);
            p.commands.entity(fire).insert((Flicker::new(z), Reveal { from: plume, to: 99.0 }));
        }
    })
}

/// Байконур на рассвете: небо от ночной синевы к оранжевой полосе.
fn steppe_dawn(p: &mut Painter, horizon: f32) {
    let top = Color::srgb(0.04, 0.06, 0.15).to_srgba();
    let low = Color::srgb(0.86, 0.46, 0.26).to_srgba();
    let bands = 16;
    let height = (HH - horizon) / bands as f32;
    for i in 0..bands {
        let k = (i as f32 / (bands - 1) as f32).powf(2.2);
        let color = Color::srgb(
            low.red + (top.red - low.red) * k,
            low.green + (top.green - low.green) * k,
            low.blue + (top.blue - low.blue) * k,
        );
        p.rect(0.0, horizon + height * (i as f32 + 0.5), HW * 2.0, height + 1.0, color, 0.0);
    }
    let mut dice = Dice(1975);
    for _ in 0..40 {
        let star = p.rect(dice.range(-HW, HW), dice.range(90.0, HH), 2.0, 2.0, pal::WHITE.with_alpha(0.6), 0.5);
        p.commands.entity(star).insert(Twinkle(dice.range(0.0, 6.0)));
    }
    // Солнце встаёт слева, за горизонтом.
    for (r, alpha) in [(130.0, 0.08), (70.0, 0.18), (34.0, 0.9)] {
        p.circle(-330.0, horizon, r, Color::srgb(1.0, 0.72, 0.38).with_alpha(alpha), 1.0);
    }
    // Степь и силуэты монтажно-испытательного корпуса.
    let ground = HH + horizon;
    p.rect(0.0, horizon - ground / 2.0, HW * 2.0, ground, Color::srgb(0.20, 0.17, 0.13), 3.0);
    p.rect(0.0, horizon - 1.0, HW * 2.0, 2.0, Color::srgb(0.42, 0.32, 0.22), 3.1);
    let silhouette = Color::srgb(0.10, 0.09, 0.10);
    p.rect(330.0, horizon + 22.0, 150.0, 44.0, silhouette, 2.0);
    p.rect(300.0, horizon + 52.0, 60.0, 16.0, silhouette, 2.0);
    p.rect(-200.0, horizon + 40.0, 6.0, 80.0, silhouette, 2.0);
}

/// Мачта громоотвода у стола Н-1. Точка (x, y) — основание.
fn lightning_mast(p: &mut Painter, x: f32, y: f32, h: f32) {
    p.rect(x, y + h / 2.0, 4.0, h, Color::srgb(0.30, 0.30, 0.32), 4.0);
    for i in 0..(h / 40.0) as i32 {
        let level = y + 20.0 + i as f32 * 40.0;
        p.rect(x, level, 12.0, 2.0, Color::srgb(0.30, 0.30, 0.32), 4.0);
    }
    let lamp = p.rect(x, y + h + 4.0, 5.0, 5.0, pal::USSR, 4.1);
    p.commands.entity(lamp).insert(Twinkle(x));
}

/// Старт Н-1 с Байконура. Первый пуск — кинематографичный: кинорамка,
/// отвод башни обслуживания, команды стартового расчёта, зажигание
/// тридцати двигателей, медленный отрыв и дымный столб. Следующие пуски
/// показаны короче, чтобы не затягивать ход.
fn n1_launch(p: &mut Painter, number: u8, text: &Catalog) -> f32 {
    let full = number == 1;
    let (ignite, lift, acc, duration) = if full { (3.2, 4.3, 64.0, 9.2) } else { (0.6, 1.3, 140.0, 4.8) };
    let (x, base, scale) = (0.0, -170.0, 0.75);
    let horizon = -130.0;
    steppe_dawn(p, horizon);
    letterbox(p, 46.0);

    // Стартовый стол, газоотводный лоток и мачты громоотводов.
    p.rect(x, base - 16.0, 220.0, 30.0, Color::srgb(0.42, 0.40, 0.37), 5.0);
    p.rect(x, base - 22.0, 90.0, 18.0, pal::DARK, 5.1);
    for mx in [-190.0, 190.0] {
        lightning_mast(p, x + mx, base, 330.0);
    }
    if full {
        let gantry = p.group(|p| tower(p, x + 62.0, base, 270.0));
        p.tag(&gantry, Slide::new(v(340.0, 0.0), 1.8, 0.7));
        flash_label(p, 0.0, 170.0, text.get("cut.n1.place"), 24.0, pal::WHITE, 0.4, 2.6);
    }

    // Команды стартового расчёта.
    let calls: &[(&str, f32, f32)] = if full {
        &[("cut.n1.key", 1.0, 2.0), ("cut.n1.drain", 2.0, 3.0), ("cut.n1.ignition", 3.0, 4.2), ("cut.n1.liftoff", 4.3, 5.9)]
    } else {
        &[("cut.n1.ignition", 0.3, 1.2), ("cut.n1.liftoff", 1.3, 2.6)]
    };
    for &(key, from, to) in calls {
        flash_label(p, -280.0, 90.0, text.get(key), 30.0, pal::AMBER, from, to);
    }

    let ship = p.group(|p| {
        n1(p, x, base, scale);
        n1_flames(p, x, base, scale, ignite);
    });
    p.tag(&ship, Motion { vel: Vec2::ZERO, acc: v(0.0, acc), delay: lift });

    // Зарево у стола и свет на всю сцену.
    let glow = p.ellipse(x, base, 200.0, 50.0, pal::FIRE.with_alpha(0.55), 6.0);
    p.commands.entity(glow).insert(Fade::keys(&[(ignite, 0.0), (ignite + 0.4, 1.0), (lift + 1.5, 0.6), (lift + 3.0, 0.0)]));
    light(p, pal::AMBER.with_alpha(0.10), &[(ignite, 0.0), (ignite + 0.5, 1.0), (lift + 1.5, 0.5), (lift + 3.5, 0.0)]);
    ground_smoke(p, x, base - 4.0, ignite + 0.1, if full { 26 } else { 14 }, 170.0, 31);
    exhaust_trail(p, x, base, lift, acc, duration, 33);
    p.shake(ignite, lift + if full { 2.6 } else { 1.4 }, if full { 7.0 } else { 5.0 });
    curtain(p, duration);
    duration
}

/// Стыковка модуля: пунктир подхода, рамка стыковочного узла, вспышка касания.
fn docking(p: &mut Painter, number: u8, text: &Catalog) -> f32 {
    let duration = 3.8;
    let (start, travel) = (0.3, 1.8);
    let arrive = start + travel;
    sky(p, pal::NIGHT, 170, -HH, 61);
    p.circle(0.0, -760.0, 620.0, pal::MOON, 2.0);
    p.circle(-180.0, -200.0, 34.0, pal::MOON_DARK, 2.1);
    p.circle(160.0, -170.0, 22.0, pal::MOON_DARK, 2.1);
    let left = -200.0;
    station(p, left, 40.0, 2.0, number - 1);
    let target = left + (number as f32 - 1.0) * 64.0;
    let distance = 520.0;
    let port = v(target - 30.0, 40.0);
    let path = dashes(p, &[v(target + distance - 30.0, 40.0), port], pal::USSR.with_alpha(0.7), 5.5);
    p.tag(&path, Fade::keys(&[(arrive - 0.2, 1.0), (arrive, 0.0)]));
    if number > 1 {
        target_mark(p, port.x, port.y, pal::USSR, "", arrive);
    }
    let module = p.group(|p| {
        station_module(p, target + distance, 40.0, 2.0, number);
        let jet = p.tri(target + distance + 34.0, 40.0, [(0.0, -5.0), (0.0, 5.0), (26.0, 0.0)], pal::AMBER, 5.8);
        p.commands.entity(jet).insert((Flicker::new(0.0), Reveal { from: 0.0, to: arrive - 0.4 }));
    });
    p.tag(&module, Slide::new(v(-distance, 0.0), travel, start));
    let spark = p.circle(port.x, port.y, 10.0, pal::WHITE, 9.0);
    p.commands.entity(spark).insert((Reveal { from: arrive, to: 99.0 }, Grow(0.8), Fade::keys(&[(arrive, 1.0), (arrive + 0.4, 0.0)])));
    let label = text.get("cut.docked");
    flash_label(p, target, 130.0, label, 22.0, pal::USSR, arrive, duration - 0.3);
    curtain(p, duration);
    duration
}

/// Перехват: захват цели, пуск, попадание.
fn intercept(p: &mut Painter, text: &Catalog) -> f32 {
    let duration = 4.2;
    let (launch, hit) = (1.0, 2.2);
    sky(p, pal::NIGHT, 170, -HH, 51);
    p.circle(0.0, -640.0, 520.0, pal::MOON, 2.0);
    station(p, -380.0, 60.0, 1.2, DEFENCE_MODULE);
    let (from, to) = (v(-250.0, 46.0), v(240.0, 70.0));
    let lock = dashes(p, &[from, to], pal::USSR, 5.0);
    p.tag(&lock, Fade::keys(&[(0.1, 0.0), (0.3, 1.0), (hit, 1.0), (hit + 0.1, 0.0)]));
    target_mark(p, to.x, to.y, pal::USSR, "", hit);
    flash_label(p, 0.0, 160.0, text.get("cut.lock"), 24.0, pal::USSR, 0.2, launch + 0.4);
    let ship = p.group(|p| csm(p, to.x, to.y, 1.5));
    p.tag(&ship, Reveal { from: 0.0, to: hit });
    let missile = p.group(|p| {
        p.rect(from.x, from.y, 22.0, 5.0, pal::WHITE, 7.0);
        let jet = p.tri(from.x - 12.0, from.y, [(0.0, -4.0), (0.0, 4.0), (-24.0, 0.0)], pal::FIRE, 6.9);
        p.commands.entity(jet).insert(Flicker::new(0.0));
    });
    p.tag(&missile, Motion { vel: (to - from) / (hit - launch), acc: Vec2::ZERO, delay: launch });
    p.tag(&missile, Reveal { from: launch, to: hit });
    for (radius, color, z) in [(34.0, pal::FIRE, 8.0), (20.0, pal::AMBER, 8.1), (9.0, pal::WHITE, 8.2)] {
        let blast = p.circle(to.x, to.y, radius, color, z);
        p.commands.entity(blast).insert((Reveal { from: hit, to: 99.0 }, Grow(0.5), Fade::keys(&[(hit + 0.8, 1.0), (hit + 2.0, 0.3)])));
    }
    let mut dice = Dice(13);
    for _ in 0..18 {
        let angle = dice.range(0.0, std::f32::consts::TAU);
        let piece = p.rect(to.x, to.y, dice.range(3.0, 10.0), 4.0, pal::STEEL, 7.5);
        p.commands.entity(piece).insert((
            Reveal { from: hit, to: 99.0 },
            Motion { vel: v(angle.cos(), angle.sin()) * dice.range(20.0, 90.0), acc: Vec2::ZERO, delay: hit },
        ));
    }
    light(p, pal::FIRE.with_alpha(0.2), &[(hit, 0.0), (hit + 0.1, 1.0), (hit + 1.0, 0.0)]);
    p.shake(hit, hit + 1.2, 9.0);
    curtain(p, duration);
    duration
}

/// Посадка советского корабля на обратной стороне.
fn soviet_landing(p: &mut Painter, state: &GameState, text: &Catalog) -> f32 {
    let duration = 4.8;
    let touchdown = 2.5;
    far_side(p, state, false, text);
    let goal = v(60.0, GROUND);
    let start = goal + v(220.0, 330.0);
    let path = dashes(p, &[start, goal], pal::USSR.with_alpha(0.7), 7.5);
    p.tag(&path, Fade::keys(&[(touchdown, 1.0), (touchdown + 0.3, 0.0)]));
    target_mark(p, goal.x, goal.y + 14.0, pal::USSR, "", touchdown);
    let ship = p.group(|p| {
        lander(p, start.x, start.y, 1.6, pal::USSR, 3);
        let fire = flame(p, start.x, start.y + 10.0, 1.0);
        p.tag(&fire, Reveal { from: 0.0, to: touchdown + 0.1 });
    });
    p.tag(&ship, Slide::new(goal - start, touchdown, 0.0));
    lunar_dust(p, goal, touchdown);
    flash_label(p, goal.x, goal.y + 160.0, text.get("cut.contact"), 22.0, pal::USSR, touchdown, duration - 0.3);
    p.shake(touchdown, touchdown + 0.4, 4.0);
    curtain(p, duration);
    duration
}

/// Заставка на весь экран: центр в середине окна, размер 1280×720.
pub fn draw_title(p: &mut Painter, text: &Catalog) {
    p.rect(0.0, 0.0, 1280.0, 720.0, pal::NIGHT, 0.0);
    let mut dice = Dice(1973);
    for _ in 0..240 {
        let star = p.rect(dice.range(-640.0, 640.0), dice.range(-360.0, 360.0), 2.0, 2.0, pal::WHITE, 1.0);
        p.commands.entity(star).insert(Twinkle(dice.range(0.0, std::f32::consts::TAU)));
    }
    // Лунный горизонт, над ним Земля и тень советской станции.
    p.circle(0.0, -1180.0, 1000.0, pal::MOON, 3.0);
    for (x, y, r) in [(-300.0, -250.0, 60.0), (180.0, -230.0, 40.0), (420.0, -300.0, 70.0), (-520.0, -320.0, 50.0)] {
        p.ellipse(x, y, r, r * 0.3, pal::MOON_LIGHT, 3.1);
        p.ellipse(x, y - 3.0, r * 0.88, r * 0.24, pal::MOON_DARK, 3.2);
    }
    earth(p, -500.0, 270.0, 56.0);
    let body = Body { ecc: 0.1, far_z: 2.5, far_up: true, shrink: 0.4, tilt: 0.5 };
    orbit_trace(p, v(0.0, -100.0), v(520.0, 40.0), body, pal::USSR, 3.3);
    orbiting_station(p, 1.0, 5, los_tip(text, 5, War::Peace), v(0.0, -100.0), v(520.0, 40.0), 0.18, 0.6, body);
    lander(p, 330.0, -238.0, 1.2, pal::WHITE, 2);
    flag(p, 270.0, -222.0, pal::US);
}

/// Проект «Копьё»: перехватчик с Земли бьёт по советской станции.
/// Зеркало перехвата: та же рамка цели, пунктир и вспышка попадания.
fn strike(p: &mut Painter, text: &Catalog) -> f32 {
    let duration = 4.6;
    let (launch, hit) = (0.6, 2.4);
    sky(p, pal::NIGHT, 170, -HH, 52);
    p.circle(0.0, -640.0, 520.0, pal::MOON, 2.0);
    let target = v(160.0, 80.0);
    let left = target.x - station_middle(1.1, 5);
    let hull = p.group(|p| station(p, left, target.y, 1.1, 5));
    p.tag(&hull, Reveal { from: 0.0, to: hit });
    target_mark(p, target.x, target.y, pal::US, "ЛОС", hit);
    let from = v(-HW - 20.0, -60.0);
    let path = dashes(p, &[from, target], pal::US.with_alpha(0.8), 5.0);
    p.tag(&path, Fade::keys(&[(0.1, 0.0), (0.3, 1.0), (hit, 1.0), (hit + 0.1, 0.0)]));
    flash_label(p, -200.0, 170.0, text.get("cut.lock"), 22.0, pal::US, 0.2, launch + 0.6);
    let lance = p.group(|p| {
        p.rect(from.x, from.y, 26.0, 6.0, pal::WHITE, 7.0);
        p.tri(from.x + 13.0, from.y, [(0.0, -3.0), (0.0, 3.0), (8.0, 0.0)], pal::US, 7.0);
        let jet = p.tri(from.x - 13.0, from.y, [(0.0, -4.0), (0.0, 4.0), (-26.0, 0.0)], pal::FIRE, 6.9);
        p.commands.entity(jet).insert(Flicker::new(0.0));
    });
    p.tag(&lance, Motion { vel: (target - from) / (hit - launch), acc: Vec2::ZERO, delay: launch });
    p.tag(&lance, Reveal { from: launch, to: hit });
    for (radius, color, z) in [(60.0, pal::FIRE, 8.0), (36.0, pal::AMBER, 8.1), (16.0, pal::WHITE, 8.2)] {
        let blast = p.circle(target.x, target.y, radius, color, z);
        p.commands.entity(blast).insert((Reveal { from: hit, to: 99.0 }, Grow(0.5), Fade::keys(&[(hit + 0.8, 1.0), (hit + 2.0, 0.2)])));
    }
    // Модули разлетаются и кувыркаются к Луне.
    let mut dice = Dice(17);
    for i in 0..26 {
        let angle = dice.range(0.0, std::f32::consts::TAU);
        let color = if i % 5 == 0 { pal::USSR } else { pal::STEEL };
        let piece = p.rect(target.x, target.y, dice.range(4.0, 16.0), dice.range(3.0, 8.0), color, 7.5);
        p.commands.entity(piece).insert((
            Reveal { from: hit, to: 99.0 },
            Motion { vel: v(angle.cos(), angle.sin()) * dice.range(30.0, 130.0), acc: v(0.0, -40.0), delay: hit },
        ));
    }
    flash_label(p, target.x, target.y + 120.0, text.get("cut.hit"), 24.0, pal::US, hit + 0.2, duration - 0.3);
    light(p, pal::WHITE.with_alpha(0.25), &[(hit, 0.0), (hit + 0.08, 1.0), (hit + 1.0, 0.0)]);
    p.shake(hit, hit + 1.2, 9.0);
    curtain(p, duration);
    duration
}

/// Чрезвычайное положение: бронетехника въезжает на Пенсильвания-авеню.
fn crackdown(p: &mut Painter, text: &Catalog) -> f32 {
    let duration = 4.4;
    p.rect(0.0, 0.0, HW * 2.0, HH * 2.0, Color::srgb(0.05, 0.05, 0.09), 0.0);
    // Купол Капитолия силуэтом.
    let stone = Color::srgb(0.30, 0.30, 0.34);
    p.rect(0.0, -40.0, 520.0, 90.0, stone, 1.0);
    p.circle(0.0, 40.0, 80.0, stone, 1.0);
    p.rect(0.0, 130.0, 24.0, 40.0, stone, 1.0);
    for i in 0..12 {
        let window = p.rect(-220.0 + i as f32 * 40.0, -40.0, 10.0, 30.0, pal::AMBER.with_alpha(0.7), 1.1);
        p.commands.entity(window).insert(Fade::keys(&[(0.6 + i as f32 * 0.1, 1.0), (0.8 + i as f32 * 0.1, 0.0)]));
    }
    p.rect(0.0, -170.0, HW * 2.0, 150.0, Color::srgb(0.10, 0.10, 0.12), 2.0);
    letterbox(p, 40.0);
    for (x, base) in [(-300.0, -0.3), (300.0, 0.3)] {
        let beam = p.tri(x, -170.0, [(-6.0, 0.0), (6.0, 0.0), (0.0, 460.0)], pal::WHITE.with_alpha(0.12), 3.0);
        p.commands.entity(beam).insert(Sweep { base, amp: 0.35, speed: 1.2 });
    }
    for (row, delay) in [(0.0, 0.2), (1.0, 0.6)] {
        let y = -200.0 + row * 30.0;
        let column = p.group(|p| {
            apc(p, -HW - 80.0, y, 5.0 - row);
            apc(p, -HW - 230.0, y, 5.0 - row);
        });
        p.tag(&column, Slide::new(v(560.0 + row * 120.0, 0.0), 2.6, delay));
    }
    flash_label(p, 0.0, 200.0, text.get("cut.curfew"), 26.0, pal::USSR, 1.6, duration - 0.2);
    p.shake(0.2, 3.0, 2.0);
    curtain(p, duration);
    duration
}

/// Вспышки на Земле, как их видно с Луны: две или все сразу.
fn flash(p: &mut Painter, state: &GameState, text: &Catalog) -> f32 {
    let duration = 5.2;
    let total = state.location == Location::Armageddon;
    sky(p, pal::NIGHT, 140, GROUND, 97);
    let (ex, ey, r) = (40.0, 100.0, 110.0);
    earth(p, ex, ey, r);
    moon_ground(p, pal::MOON, 23);
    let count = if total { BLASTS.len() } else { 2 };
    for (i, (dx, dy)) in BLASTS.iter().take(count).enumerate() {
        let at = 0.8 + i as f32 * if total { 0.22 } else { 1.1 };
        for (radius, color) in [(22.0, pal::FIRE.with_alpha(0.5)), (9.0, pal::WHITE)] {
            let blast = p.circle(ex + dx * r, ey + dy * r, radius, color, 2.6);
            p.commands.entity(blast).insert((Reveal { from: at, to: 99.0 }, Grow(0.3), Fade::keys(&[(at, 1.0), (at + 2.5, 0.5)])));
        }
        light(p, pal::WHITE.with_alpha(0.12), &[(at, 0.0), (at + 0.05, 1.0), (at + 0.4, 0.0)]);
    }
    for (i, x) in [-260.0, -230.0, -200.0, -170.0].into_iter().enumerate() {
        astronaut(p, x, GROUND - 16.0 - (i % 2) as f32 * 6.0, pal::WHITE);
    }
    if total {
        flash_label(p, 0.0, -170.0, text.get("cut.silence"), 22.0, pal::USSR, 3.8, duration - 0.2);
    }
    curtain(p, duration);
    duration
}
