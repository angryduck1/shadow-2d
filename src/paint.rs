//! Рисование сцен из простых фигур и компоненты анимации.
//!
//! Картинок в игре нет: каждая сцена собирается из прямоугольников,
//! кругов, треугольников и надписей. `Painter` прячет детали движка,
//! а компоненты ниже оживляют фигуры без кода на каждую сцену.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::text::TextBounds;
use bevy::window::PrimaryWindow;

/// Палитра. Все цвета сцен берутся отсюда.
pub mod pal {
    use bevy::prelude::Color;
    pub const NIGHT: Color = Color::srgb(0.027, 0.043, 0.078);
    pub const PANEL: Color = Color::srgb(0.051, 0.082, 0.141);
    pub const LINE: Color = Color::srgb(0.141, 0.204, 0.310);
    pub const TEXT: Color = Color::srgb(0.839, 0.886, 0.941);
    pub const MUTED: Color = Color::srgb(0.486, 0.557, 0.659);
    pub const US: Color = Color::srgb(0.353, 0.635, 1.0);
    pub const USSR: Color = Color::srgb(1.0, 0.42, 0.369);
    pub const AMBER: Color = Color::srgb(1.0, 0.757, 0.271);
    pub const GREEN: Color = Color::srgb(0.388, 0.839, 0.545);
    pub const WHITE: Color = Color::srgb(0.93, 0.94, 0.95);
    pub const STEEL: Color = Color::srgb(0.62, 0.66, 0.72);
    pub const DARK: Color = Color::srgb(0.11, 0.12, 0.15);
    pub const MOON: Color = Color::srgb(0.56, 0.59, 0.64);
    pub const MOON_DARK: Color = Color::srgb(0.36, 0.39, 0.44);
    pub const MOON_LIGHT: Color = Color::srgb(0.70, 0.73, 0.77);
    pub const GOLD: Color = Color::srgb(0.85, 0.68, 0.30);
    pub const FIRE: Color = Color::srgb(1.0, 0.55, 0.15);
    pub const SEA: Color = Color::srgb(0.06, 0.12, 0.22);
    pub const EARTH: Color = Color::srgb(0.18, 0.42, 0.78);
    pub const LAND: Color = Color::srgb(0.30, 0.62, 0.40);
    pub const MARS: Color = Color::srgb(0.80, 0.36, 0.22);
}

/// Метка всего, что относится к текущей сцене и стирается при её смене.
#[derive(Component)]
pub struct SceneEntity;

/// Секунды с момента появления текущей сцены.
#[derive(Resource, Default)]
pub struct SceneClock(pub f32);

/// Мерцание: прозрачность спрайта колеблется. Поле задаёт сдвиг фазы.
#[derive(Component, Clone)]
pub struct Twinkle(pub f32);

/// Мигание надписи.
#[derive(Component, Clone)]
pub struct Blink;

/// Движение со скоростью и ускорением, начиная с момента `delay`.
#[derive(Component, Clone)]
pub struct Motion {
    pub vel: Vec2,
    pub acc: Vec2,
    pub delay: f32,
}

/// Сдвиг на `delta` за `duration` секунд, начиная с момента `delay`.
/// Движение замедляется к концу и завершается точно в срок, поэтому
/// по нему можно привязать вспышки, надписи и пыль.
#[derive(Component, Clone)]
pub struct Slide {
    pub delta: Vec2,
    pub duration: f32,
    pub delay: f32,
    done: f32,
}

impl Slide {
    pub fn new(delta: Vec2, duration: f32, delay: f32) -> Self {
        Self { delta, duration, delay, done: 0.0 }
    }
}

/// Кубическое замедление: быстрый старт, мягкая остановка.
pub fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

/// Прозрачность по ключевым точкам «время → доля исходной прозрачности».
/// Между точками значение меняется линейно, до первой и после последней
/// держится крайнее. Так строятся появление, исчезновение и затемнения.
#[derive(Component, Clone)]
pub struct Fade {
    keys: Vec<(f32, f32)>,
    base: Option<f32>,
}

impl Fade {
    pub fn keys(keys: &[(f32, f32)]) -> Self {
        Self { keys: keys.to_vec(), base: None }
    }

    /// Появляется в `from`, держится и исчезает к `to`; `edge` — длина перехода.
    pub fn window(from: f32, to: f32, edge: f32) -> Self {
        Self::keys(&[(from, 0.0), (from + edge, 1.0), (to - edge, 1.0), (to, 0.0)])
    }

    /// Проявляется за `edge` секунд начиная с `from`.
    pub fn appear(from: f32, edge: f32) -> Self {
        Self::keys(&[(from, 0.0), (from + edge, 1.0)])
    }

    fn at(&self, time: f32) -> f32 {
        let (first, last) = (self.keys[0], self.keys[self.keys.len() - 1]);
        if time <= first.0 {
            return first.1;
        }
        for pair in self.keys.windows(2) {
            let ((t0, a0), (t1, a1)) = (pair[0], pair[1]);
            if time <= t1 {
                let k = if t1 > t0 { (time - t0) / (t1 - t0) } else { 1.0 };
                return a0 + (a1 - a0) * k;
            }
        }
        last.1
    }
}

/// Тряска камеры в промежутке времени сцены. Сила затухает к концу.
#[derive(Resource, Default, Clone, Copy)]
pub struct CameraShake {
    pub from: f32,
    pub to: f32,
    pub amp: f32,
}

/// Время небесных тел. Идёт отдельно от времени сцены: клавиши `[` и `]`
/// ускоряют и замедляют орбиты, не трогая остальные анимации.
#[derive(Resource)]
pub struct OrbitClock {
    pub t: f32,
    pub warp: f32,
}

impl Default for OrbitClock {
    fn default() -> Self {
        Self { t: 0.0, warp: 1.0 }
    }
}

/// Пределы ускорения времени на орбите.
const WARP_MIN: f32 = 0.25;
const WARP_MAX: f32 = 16.0;

/// Движение по эллипсу вокруг точки. `offset` держит форму группы фигур.
/// Без `body` это простое покачивание по времени сцены: краны, купюры,
/// плакаты. С `body` — небесное тело со своей физикой.
#[derive(Component, Clone)]
pub struct Orbit {
    pub center: Vec2,
    pub radius: Vec2,
    pub speed: f32,
    pub phase: f32,
    pub offset: Vec2,
    pub body: Option<Body>,
    /// Исходные слой, поворот и масштаб фигуры: от них считается перспектива.
    rest: Option<(f32, Quat, Vec3)>,
}

/// Физика небесной орбиты.
///
/// Наклонная круговая орбита на экране видна эллипсом. Дальняя половина
/// уходит за планету (слой `far_z` ниже диска) и уменьшается, ближняя
/// проходит перед ней. Эксцентриситет `ecc` подчиняет скорость второму
/// закону Кеплера: у ближней к фокусу точки тело летит быстрее. `tilt`
/// плавно наклоняет корпус по ходу движения.
#[derive(Clone, Copy)]
pub struct Body {
    pub ecc: f32,
    pub far_z: f32,
    /// Дальняя половина орбиты — верхняя (вид сверху) или нижняя (вид с поверхности).
    pub far_up: bool,
    pub shrink: f32,
    pub tilt: f32,
}

impl Body {
    /// Орбита вокруг планеты, видимая чуть сверху: верх эллипса за диском.
    pub fn around(far_z: f32) -> Self {
        Self { ecc: 0.12, far_z, far_up: true, shrink: 0.35, tilt: 0.5 }
    }

    /// Станция над горизонтом, видимая с поверхности: нижняя дуга под грунтом.
    pub fn overhead(far_z: f32) -> Self {
        Self { ecc: 0.05, far_z, far_up: false, shrink: 0.3, tilt: 0.6 }
    }
}

/// Точка на орбите: положение относительно фокуса, направление движения
/// и глубина от −1 (ближе всего) до 1 (дальше всего).
pub fn orbit_point(radius: Vec2, mean_anomaly: f32, ecc: f32, far_up: bool) -> (Vec2, Vec2, f32) {
    // Уравнение Кеплера E − e·sin E = M решается методом Ньютона:
    // при e < 0,3 четырёх шагов хватает с запасом.
    let mut e = mean_anomaly + ecc * mean_anomaly.sin();
    for _ in 0..4 {
        e -= (e - ecc * e.sin() - mean_anomaly) / (1.0 - ecc * e.cos());
    }
    let (sin, cos) = e.sin_cos();
    let point = Vec2::new(radius.x * (cos - ecc), radius.y * sin);
    let tangent = Vec2::new(-radius.x * sin, radius.y * cos).normalize_or_zero();
    let depth = if far_up { sin } else { -sin };
    (point, tangent, depth)
}

/// Подсказка при наведении курсора: станции, спутники, корабли.
#[derive(Component, Clone)]
pub struct Inspect {
    pub text: String,
    pub radius: f32,
}

/// Всплывающая подсказка интерфейса и её текст.
#[derive(Component)]
pub struct Tooltip;

#[derive(Component)]
pub struct TooltipText;

/// Дрожание масштаба: пламя и вспышки. Исходный масштаб фигуры
/// запоминается при первом кадре, чтобы дрожание его не стирало.
#[derive(Component, Clone)]
pub struct Flicker {
    phase: f32,
    base: Option<Vec3>,
}

impl Flicker {
    pub fn new(phase: f32) -> Self {
        Self { phase, base: None }
    }
}

/// Качание вокруг собственной точки привязки: прожекторы и радары.
#[derive(Component, Clone)]
pub struct Sweep {
    pub base: f32,
    pub amp: f32,
    pub speed: f32,
}

/// Фигура видна только в промежутке времени сцены.
#[derive(Component, Clone)]
pub struct Reveal {
    pub from: f32,
    pub to: f32,
}

/// Фигура растёт: дым и взрывы.
#[derive(Component, Clone)]
pub struct Grow(pub f32);

/// Фигуры одной группы: их номера и исходные места.
pub type Group = Vec<(Entity, Vec2)>;

/// Инструмент рисования. Координаты отсчитываются от `origin`.
pub struct Painter<'a, 'w, 's> {
    pub commands: &'a mut Commands<'w, 's>,
    meshes: &'a mut Assets<Mesh>,
    materials: &'a mut Assets<ColorMaterial>,
    font: Handle<Font>,
    /// Обычное начертание для длинных абзацев.
    body: Handle<Font>,
    circle: Handle<Mesh>,
    pub origin: Vec2,
    track: Option<Group>,
}

impl<'a, 'w, 's> Painter<'a, 'w, 's> {
    pub fn new(
        commands: &'a mut Commands<'w, 's>,
        meshes: &'a mut Assets<Mesh>,
        materials: &'a mut Assets<ColorMaterial>,
        (font, body): (Handle<Font>, Handle<Font>),
        origin: Vec2,
    ) -> Self {
        // Много вершин: большие диски планет остаются круглыми.
        let circle = meshes.add(Circle::new(1.0).mesh().resolution(128));
        Self { commands, meshes, materials, font, body, circle, origin, track: None }
    }

    fn place(&self, x: f32, y: f32, z: f32) -> Transform {
        Transform::from_xyz(self.origin.x + x, self.origin.y + y, z)
    }

    fn remember(&mut self, entity: Entity, x: f32, y: f32) {
        if let Some(track) = &mut self.track {
            track.push((entity, Vec2::new(x, y)));
        }
    }

    /// Прямоугольник с центром в точке (x, y).
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color, z: f32) -> Entity {
        let transform = self.place(x, y, z);
        let id = self
            .commands
            .spawn((Sprite::from_color(color, Vec2::new(w, h)), transform, SceneEntity))
            .id();
        self.remember(id, x, y);
        id
    }

    /// Отрезок заданной толщины: повёрнутый прямоугольник.
    pub fn line(&mut self, a: Vec2, b: Vec2, thickness: f32, color: Color, z: f32) -> Entity {
        let mid = (a + b) / 2.0;
        let delta = b - a;
        let transform = self
            .place(mid.x, mid.y, z)
            .with_rotation(Quat::from_rotation_z(delta.y.atan2(delta.x)));
        let size = Vec2::new(delta.length(), thickness);
        let id = self.commands.spawn((Sprite::from_color(color, size), transform, SceneEntity)).id();
        self.remember(id, mid.x, mid.y);
        id
    }

    pub fn ellipse(&mut self, x: f32, y: f32, rx: f32, ry: f32, color: Color, z: f32) -> Entity {
        let transform = self.place(x, y, z).with_scale(Vec3::new(rx, ry, 1.0));
        let material = self.materials.add(ColorMaterial::from(color));
        let id = self
            .commands
            .spawn((Mesh2d(self.circle.clone()), MeshMaterial2d(material), transform, SceneEntity))
            .id();
        self.remember(id, x, y);
        id
    }

    pub fn circle(&mut self, x: f32, y: f32, r: f32, color: Color, z: f32) -> Entity {
        self.ellipse(x, y, r, r, color, z)
    }

    /// Треугольник. Вершины заданы относительно точки привязки (x, y):
    /// вокруг неё фигура вращается и дрожит.
    pub fn tri(&mut self, x: f32, y: f32, points: [(f32, f32); 3], color: Color, z: f32) -> Entity {
        let [a, b, c] = points.map(|(px, py)| Vec2::new(px, py));
        let mesh = self.meshes.add(Triangle2d::new(a, b, c));
        let material = self.materials.add(ColorMaterial::from(color));
        let transform = self.place(x, y, z);
        let id = self
            .commands
            .spawn((Mesh2d(mesh), MeshMaterial2d(material), transform, SceneEntity))
            .id();
        self.remember(id, x, y);
        id
    }

    pub fn text(&mut self, x: f32, y: f32, text: &str, size: f32, color: Color, z: f32) -> Entity {
        let transform = self.place(x, y, z);
        let font = TextFont { font: self.font.clone(), font_size: size, ..default() };
        let id = self
            .commands
            .spawn((Text2d::new(text), font, TextColor(color), transform, SceneEntity))
            .id();
        self.remember(id, x, y);
        id
    }

    /// Надпись, привязанная к точке не центром, а краем или углом.
    pub fn label(&mut self, x: f32, y: f32, text: &str, size: f32, color: Color, z: f32, anchor: Anchor) -> Entity {
        let id = self.text(x, y, text, size, color, z);
        self.commands.entity(id).insert(anchor);
        id
    }

    /// Абзац обычным начертанием: левый верхний угол в (x, y), перенос по словам.
    pub fn paragraph(&mut self, x: f32, y: f32, text: &str, size: f32, color: Color, z: f32, width: f32) -> Entity {
        let transform = self.place(x, y, z);
        let font = TextFont { font: self.body.clone(), font_size: size, ..default() };
        let id = self
            .commands
            .spawn((
                Text2d::new(text),
                font,
                TextColor(color),
                TextBounds::new_horizontal(width),
                Anchor::TopLeft,
                transform,
                SceneEntity,
            ))
            .id();
        self.remember(id, x, y);
        id
    }

    /// Невидимая точка с подсказкой. Внутри группы движется вместе с ней.
    pub fn hotspot(&mut self, x: f32, y: f32, radius: f32, text: String) -> Entity {
        let id = self.rect(x, y, 1.0, 1.0, Color::NONE, 0.0);
        self.commands.entity(id).insert(Inspect { text, radius });
        id
    }

    /// Рисует группу фигур и возвращает их список, чтобы оживить целиком.
    pub fn group(&mut self, draw: impl FnOnce(&mut Self)) -> Group {
        let outer = self.track.replace(Vec::new());
        draw(self);
        let group = self.track.take().unwrap_or_default();
        self.track = outer.map(|mut outer| {
            outer.extend(group.iter().copied());
            outer
        });
        group
    }

    /// Навешивает один и тот же компонент на каждую фигуру группы.
    pub fn tag<B: Bundle + Clone>(&mut self, group: &Group, bundle: B) {
        for (entity, _) in group {
            self.commands.entity(*entity).insert(bundle.clone());
        }
    }

    /// Трясёт камеру в промежутке времени сцены с силой `amp` пикселей.
    pub fn shake(&mut self, from: f32, to: f32, amp: f32) {
        self.commands.insert_resource(CameraShake { from, to, amp });
    }

    /// Пускает группу по эллипсу, сохраняя взаимное расположение фигур.
    pub fn orbit(&mut self, group: &Group, anchor: Vec2, center: Vec2, radius: Vec2, speed: f32, phase: f32) {
        for (entity, position) in group {
            self.commands.entity(*entity).insert(Orbit {
                center: self.origin + center,
                radius,
                speed,
                phase,
                offset: *position - anchor,
                body: None,
                rest: None,
            });
        }
    }

    /// Пускает группу по небесной орбите с перспективой и законами Кеплера.
    #[allow(clippy::too_many_arguments)]
    pub fn orbit_body(&mut self, group: &Group, anchor: Vec2, center: Vec2, radius: Vec2, speed: f32, phase: f32, body: Body) {
        for (entity, position) in group {
            self.commands.entity(*entity).insert(Orbit {
                center: self.origin + center,
                radius,
                speed,
                phase,
                offset: *position - anchor,
                body: Some(body),
                rest: None,
            });
        }
    }
}

/// Пунктир орбиты: дальняя половина бледнее и уходит за планету.
pub fn orbit_trace(p: &mut Painter, center: Vec2, radius: Vec2, body: Body, color: Color, near_z: f32) {
    let steps = 72;
    for i in (0..steps).step_by(2) {
        let at = |k: i32| {
            let m = k as f32 / steps as f32 * std::f32::consts::TAU;
            orbit_point(radius, m, body.ecc, body.far_up)
        };
        let ((a, _, depth), (b, _, _)) = (at(i), at(i + 1));
        let (alpha, z) = if depth > 0.0 { (0.18, body.far_z - 0.05) } else { (0.45, near_z) };
        p.line(center + a, center + b, 1.5, color.with_alpha(alpha), z);
    }
}

/// Простой генератор псевдослучайных чисел: звёзды всегда на своих местах.
pub struct Dice(pub u32);

impl Dice {
    /// Число от 0 до 1.
    pub fn roll(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        ((self.0 >> 16) & 0x7fff) as f32 / 32_767.0
    }

    pub fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.roll()
    }
}

// ═══════════════════════════ СИСТЕМЫ АНИМАЦИИ ═══════════════════════════

pub fn tick_clock(time: Res<Time>, mut clock: ResMut<SceneClock>, mut orbit: ResMut<OrbitClock>) {
    clock.0 += time.delta_secs();
    orbit.t += time.delta_secs() * orbit.warp;
}

/// `[` замедляет время на орбите вдвое, `]` ускоряет.
pub fn orbit_warp_keys(keys: Res<ButtonInput<KeyCode>>, mut orbit: ResMut<OrbitClock>) {
    if keys.just_pressed(KeyCode::BracketLeft) {
        orbit.warp = (orbit.warp / 2.0).max(WARP_MIN);
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        orbit.warp = (orbit.warp * 2.0).min(WARP_MAX);
    }
}

pub fn animate_twinkle(clock: Res<SceneClock>, mut stars: Query<(&Twinkle, &mut Sprite)>) {
    for (twinkle, mut sprite) in &mut stars {
        let alpha = 0.55 + 0.45 * (clock.0 * 2.2 + twinkle.0).sin();
        sprite.color.set_alpha(alpha);
    }
}

pub fn animate_blink(clock: Res<SceneClock>, mut labels: Query<&mut TextColor, With<Blink>>) {
    for mut color in &mut labels {
        color.0.set_alpha(0.55 + 0.45 * (clock.0 * 3.0).sin());
    }
}

pub fn animate_motion(
    time: Res<Time>,
    clock: Res<SceneClock>,
    mut movers: Query<(&mut Motion, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (mut motion, mut transform) in &mut movers {
        if clock.0 < motion.delay {
            continue;
        }
        let acc = motion.acc;
        motion.vel += acc * dt;
        transform.translation += (motion.vel * dt).extend(0.0);
    }
}

pub fn animate_slide(clock: Res<SceneClock>, mut sliders: Query<(&mut Slide, &mut Transform)>) {
    for (mut slide, mut transform) in &mut sliders {
        let target = ease_out((clock.0 - slide.delay) / slide.duration.max(0.001));
        let step = target - slide.done;
        if step != 0.0 {
            transform.translation += (slide.delta * step).extend(0.0);
            slide.done = target;
        }
    }
}

pub fn animate_orbit(clock: Res<SceneClock>, sky: Res<OrbitClock>, mut bodies: Query<(&mut Orbit, &mut Transform)>) {
    for (mut orbit, mut transform) in &mut bodies {
        let Some(body) = orbit.body else {
            let angle = clock.0 * orbit.speed + orbit.phase;
            let point = orbit.center + orbit.radius * Vec2::new(angle.cos(), angle.sin()) + orbit.offset;
            transform.translation.x = point.x;
            transform.translation.y = point.y;
            continue;
        };
        let (z, rotation, scale) =
            *orbit.rest.get_or_insert((transform.translation.z, transform.rotation, transform.scale));
        let anomaly = sky.t * orbit.speed + orbit.phase;
        let (point, tangent, depth) = orbit_point(orbit.radius, anomaly, body.ecc, body.far_up);
        let tangent = tangent * orbit.speed.signum();
        // Перспектива: дальше — мельче. Наклон по ходу движения без скачков:
        // он равен нулю на концах эллипса и наибольший на его склонах.
        let k = 1.0 - body.shrink * (depth + 1.0) / 2.0;
        let turn = Quat::from_rotation_z(body.tilt * tangent.x * tangent.y);
        let offset = turn * (orbit.offset * k).extend(0.0);
        let position = orbit.center + point + offset.truncate();
        // За планетой фигура уходит под её диск, сохраняя порядок слоёв группы.
        let layer = if depth > 0.0 { body.far_z + z * 0.01 } else { z };
        transform.translation = position.extend(layer);
        transform.rotation = turn * rotation;
        transform.scale = scale * Vec3::new(k, k, 1.0);
    }
}

/// Показывает подсказку у курсора, если он над станцией или спутником.
#[allow(clippy::type_complexity)]
pub fn inspect_hover(
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    spots: Query<(&Inspect, &GlobalTransform)>,
    sky: Res<OrbitClock>,
    mut tooltip: Query<(&mut Node, &mut Visibility), With<Tooltip>>,
    mut label: Query<&mut Text, With<TooltipText>>,
) {
    let Ok((mut node, mut visibility)) = tooltip.single_mut() else { return };
    let cursor = windows.single().ok().and_then(|w| w.cursor_position());
    let world = cursor.and_then(|c| {
        let (camera, place) = cameras.single().ok()?;
        camera.viewport_to_world_2d(place, c).ok()
    });
    let hit = world.and_then(|w| {
        spots
            .iter()
            .map(|(spot, place)| (spot, place.translation().truncate().distance(w) / place.scale().x.max(0.1)))
            .filter(|(spot, distance)| *distance <= spot.radius)
            .min_by(|a, b| a.1.total_cmp(&b.1))
    });
    let (Some((spot, _)), Some(cursor)) = (hit, cursor) else {
        if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    node.left = Val::Px(cursor.x + 16.0);
    node.top = Val::Px(cursor.y + 12.0);
    *visibility = Visibility::Inherited;
    let content = spot.text.replace("{w}", &format_warp(sky.warp));
    if let Ok(mut text) = label.single_mut() {
        if text.0 != content {
            text.0 = content;
        }
    }
}

/// Множитель времени без лишних нулей: «×0,25», «×4».
pub fn format_warp(warp: f32) -> String {
    let mut out = format!("{warp:.2}").trim_end_matches('0').trim_end_matches('.').to_string();
    if out.contains('.') {
        out = out.replace('.', ",");
    }
    out
}

pub fn animate_flicker(clock: Res<SceneClock>, mut flames: Query<(&mut Flicker, &mut Transform)>) {
    for (mut flicker, mut transform) in &mut flames {
        let base = *flicker.base.get_or_insert(transform.scale);
        let wave = (clock.0 * 38.0 + flicker.phase).sin();
        transform.scale = base * Vec3::new(1.0 + 0.10 * wave, 1.0 - 0.22 * wave, 1.0);
    }
}

pub fn animate_sweep(clock: Res<SceneClock>, mut beams: Query<(&Sweep, &mut Transform)>) {
    for (sweep, mut transform) in &mut beams {
        let angle = sweep.base + sweep.amp * (clock.0 * sweep.speed).sin();
        transform.rotation = Quat::from_rotation_z(angle);
    }
}

pub fn animate_reveal(clock: Res<SceneClock>, mut shapes: Query<(&Reveal, &mut Visibility)>) {
    for (reveal, mut visibility) in &mut shapes {
        let visible = clock.0 >= reveal.from && clock.0 < reveal.to;
        *visibility = if visible { Visibility::Inherited } else { Visibility::Hidden };
    }
}

pub fn animate_grow(time: Res<Time>, clock: Res<SceneClock>, mut shapes: Query<(&Grow, &mut Transform, Option<&Reveal>)>) {
    for (grow, mut transform, reveal) in &mut shapes {
        if reveal.is_some_and(|r| clock.0 < r.from) {
            continue;
        }
        let factor = 1.0 + grow.0 * time.delta_secs();
        transform.scale.x *= factor;
        transform.scale.y *= factor;
    }
}

/// Меняет прозрачность спрайтов, надписей и фигур с `Fade`.
pub fn animate_fade(
    clock: Res<SceneClock>,
    mut sprites: Query<(&mut Fade, &mut Sprite)>,
    mut labels: Query<(&mut Fade, &mut TextColor), Without<Sprite>>,
    mut shapes: Query<(&mut Fade, &MeshMaterial2d<ColorMaterial>), (Without<Sprite>, Without<TextColor>)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for (mut fade, mut sprite) in &mut sprites {
        let base = *fade.base.get_or_insert(sprite.color.alpha());
        sprite.color.set_alpha(base * fade.at(clock.0));
    }
    for (mut fade, mut color) in &mut labels {
        let base = *fade.base.get_or_insert(color.0.alpha());
        color.0.set_alpha(base * fade.at(clock.0));
    }
    for (mut fade, handle) in &mut shapes {
        let Some(material) = materials.get_mut(&handle.0) else { continue };
        // Непрозрачный материал игнорирует альфу: переводим его в смешивание.
        material.alpha_mode = bevy::sprite::AlphaMode2d::Blend;
        let base = *fade.base.get_or_insert(material.color.alpha());
        material.color.set_alpha(base * fade.at(clock.0));
    }
}

/// Трясёт камеру: старт ракеты, взрыв, перехват. Вне промежутка камера
/// стоит в начале координат.
pub fn shake_camera(
    clock: Res<SceneClock>,
    shake: Res<CameraShake>,
    mut cameras: Query<&mut Transform, With<Camera2d>>,
) {
    let t = clock.0;
    let offset = if t >= shake.from && t < shake.to {
        let fall = 1.0 - (t - shake.from) / (shake.to - shake.from);
        let amp = shake.amp * fall;
        Vec2::new((t * 61.0).sin() + 0.5 * (t * 23.0).cos(), (t * 47.0).cos() + 0.5 * (t * 31.0).sin()) * amp / 1.5
    } else {
        Vec2::ZERO
    };
    for mut transform in &mut cameras {
        transform.translation.x = offset.x;
        transform.translation.y = offset.y;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fade_interpolates_between_keys() {
        let fade = Fade::window(1.0, 3.0, 0.5);
        assert_eq!(fade.at(0.0), 0.0);
        assert_eq!(fade.at(1.25), 0.5);
        assert_eq!(fade.at(2.0), 1.0);
        assert_eq!(fade.at(9.0), 0.0);
    }

    #[test]
    fn kepler_orbit_is_faster_near_focus() {
        let radius = Vec2::new(100.0, 40.0);
        // Шаг средней аномалии одинаков, а путь у фокуса длиннее.
        let step = |m: f32| (orbit_point(radius, m + 0.01, 0.3, true).0 - orbit_point(radius, m, 0.3, true).0).length();
        assert!(step(0.0) > step(std::f32::consts::PI));
        let (_, _, far) = orbit_point(radius, std::f32::consts::FRAC_PI_2, 0.0, true);
        assert!(far > 0.99, "верх эллипса — дальняя сторона");
    }

    #[test]
    fn warp_is_printed_compactly() {
        assert_eq!(format_warp(4.0), "4");
        assert_eq!(format_warp(0.25), "0,25");
        assert_eq!(format_warp(0.5), "0,5");
    }

    #[test]
    fn ease_out_ends_exactly() {
        assert_eq!(ease_out(0.0), 0.0);
        assert_eq!(ease_out(1.0), 1.0);
        assert_eq!(ease_out(5.0), 1.0);
    }
}
