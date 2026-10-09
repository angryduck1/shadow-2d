//! Правила игры. Модуль не знает о движке: его можно тестировать отдельно.
//!
//! Доктрина партии — «победа любой ценой». Война на Луне, бунт и пустая
//! казна не обрывают игру сами: каждая такая беда становится кризисом
//! с выбором. Поражение наступает только в крайних развязках или когда
//! игрок сам решает сдаться.

use crate::content::Catalog;

// ════════════════════════════════ ДАННЫЕ ════════════════════════════════

/// Изменения показателей от одного решения.
#[derive(Debug, Clone, Copy)]
pub struct Effects {
    pub budget: i32,
    pub secrecy: i32,
    pub unrest: i32,
    pub tension: i32,
    pub base: i32,
}

impl Effects {
    pub const NONE: Effects = Effects { budget: 0, secrecy: 0, unrest: 0, tension: 0, base: 0 };
}

/// Шесть плановых ракет Saturn V и резервный рейс. Порядок вариантов —
/// порядок запусков.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mission {
    Apollo18,
    Apollo19,
    Apollo20,
    Apollo21,
    Apollo22,
    Apollo23,
    /// Грузовой рейс на ракете третьей смены. В плане его нет.
    Reserve,
}

impl Mission {
    pub const ALL: [Mission; 7] = [
        Mission::Apollo18,
        Mission::Apollo19,
        Mission::Apollo20,
        Mission::Apollo21,
        Mission::Apollo22,
        Mission::Apollo23,
        Mission::Reserve,
    ];

    /// Префикс ключей в файле диалогов: `a18.name`, `a18.report`.
    pub fn key(self) -> &'static str {
        match self {
            Mission::Apollo18 => "a18",
            Mission::Apollo19 => "a19",
            Mission::Apollo20 => "a20",
            Mission::Apollo21 => "a21",
            Mission::Apollo22 => "a22",
            Mission::Apollo23 => "a23",
            Mission::Reserve => "reserve",
        }
    }

    /// Стоимость запуска в условных миллионах.
    pub fn cost(self) -> i32 {
        match self {
            Mission::Apollo18 | Mission::Apollo21 | Mission::Apollo23 | Mission::Reserve => 20,
            Mission::Apollo19 | Mission::Apollo20 => 25,
            Mission::Apollo22 => 30,
        }
    }

    pub fn next(self) -> Option<Mission> {
        match self {
            Mission::Apollo18 => Some(Mission::Apollo19),
            Mission::Apollo19 => Some(Mission::Apollo20),
            Mission::Apollo20 => Some(Mission::Apollo21),
            Mission::Apollo21 => Some(Mission::Apollo22),
            Mission::Apollo22 => Some(Mission::Apollo23),
            Mission::Apollo23 | Mission::Reserve => None,
        }
    }

    /// Сколько плановых миссий осталось, считая эту.
    fn remaining(self) -> u8 {
        match self {
            Mission::Reserve => 1,
            planned => Mission::Apollo23 as u8 - planned as u8 + 1,
        }
    }

    fn bit(self) -> u8 {
        1 << self as u8
    }

    /// Последствия двух решений на месте. Первое всегда рискованнее.
    fn effects(self) -> [Effects; 2] {
        let n = Effects::NONE;
        match self {
            Mission::Apollo18 => [Effects { base: 10, secrecy: -8, ..n }, Effects { base: 4, ..n }],
            Mission::Apollo19 => [Effects { base: 10, tension: 8, ..n }, Effects { base: 4, ..n }],
            Mission::Apollo20 => {
                [Effects { base: 10, secrecy: -10, tension: 5, ..n }, Effects { base: 4, ..n }]
            }
            Mission::Apollo21 => [
                Effects { base: 10, tension: -10, budget: -5, ..n },
                Effects { base: 4, tension: 8, ..n },
            ],
            Mission::Apollo22 => [
                Effects { base: 6, tension: 12, unrest: -15, ..n },
                Effects { base: 6, tension: -5, ..n },
            ],
            Mission::Apollo23 => [Effects { base: 12, tension: 10, ..n }, Effects { base: 8, ..n }],
            Mission::Reserve => [n, n],
        }
    }
}

/// Доставленные миссии: по биту на миссию. Состояние партии остаётся
/// без выделений памяти, поэтому копируется и хешируется дёшево.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Missions(u8);

impl Missions {
    pub fn contains(self, mission: Mission) -> bool {
        self.0 & mission.bit() != 0
    }

    pub fn with(self, mission: Mission) -> Self {
        Self(self.0 | mission.bit())
    }

    pub fn without(self, mission: Mission) -> Self {
        Self(self.0 & !mission.bit())
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Миссии в порядке запусков.
    pub fn iter(self) -> impl Iterator<Item = Mission> {
        Mission::ALL.into_iter().filter(move |m| self.contains(*m))
    }
}

/// Сколько модулей в советской станции и какой из них несёт ракеты.
/// Станцию собирают пять пусков Н-1.
pub const SOVIET_MODULES: u8 = 5;
pub const DEFENCE_MODULE: u8 = 4;

/// Saturn V: шесть по плану и ещё две, если заводы работают в три смены.
/// Так у США появляется численный перевес над пятью Н-1.
pub const SATURN_PLAN: u8 = 6;
pub const SATURN_LIMIT: u8 = 8;
const THIRD_SHIFT_COST: i32 = 40;

const LAUNCH_SECRECY_THRESHOLD: i32 = 30;
/// Напряжённость, при которой советская оборона бьёт по кораблям на трассе.
const TENSION_LIMIT: i32 = 80;
/// Слепая зона радара, о которой рассказал перебежчик, поднимает порог.
const TENSION_LIMIT_BLIND: i32 = 95;
const UNREST_LIMIT: i32 = 100;
/// До этого уровня чрезвычайное положение сбивает недовольство.
const UNREST_AFTER_CRACKDOWN: i32 = 55;
/// Пороги развязок после высадки СССР.
const MARS_BASE: i32 = 100;
/// Ракета, оставленная в резерве, снижает требования к базе для Марса.
const MARS_BASE_RESERVE: i32 = 90;
const OPEN_BASE: i32 = 88;
/// Чтобы уйти мимо Луны, нужна база, способная продержаться без Земли.
const BYPASS_BASE: i32 = 60;
const WAR_TENSION: i32 = 60;
/// Удар по станции при такой напряжённости переносит войну на Землю.
const BRINK_TENSION: i32 = 100;
/// Экстренный заём Пентагона, когда платить подрядчикам нечем.
const WAR_LOAN: i32 = 60;
/// Дольше трёх месяцев подряд старт не откладывают: окно к Луне закрывается.
const MAX_RUSH: u8 = 3;
/// Доверие Москвы копится до этого уровня.
const DETENTE_MAX: u8 = 2;
/// Риск аварии на старте в процентах. Каждый месяц отсрочки снижает его.
const ACCIDENT_RISK: u32 = 12;
const ACCIDENT_RISK_PER_DELAY: u32 = 4;
const ACCIDENT_RISK_MIN: u32 = 2;
/// Шанс события из досье после каждой миссии, в процентах.
const EVENT_CHANCE: u32 = 45;

// ══════════════════════════════ СОБЫТИЯ ═════════════════════════════════

/// Эпизоды из глав трилогии. Каждый случается не больше одного раза,
/// когда выполнено его условие и выпал шанс.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Event {
    /// Книга I, гл. 8: Госдеп продаёт КГБ легенду о противоспутниковом оружии.
    Bait,
    /// Книга I: семьи астронавтов не знают, где те находятся.
    Families,
    /// Книга II, гл. 6: перехваченные чертежи четвёртого модуля.
    Blueprints,
    /// Книга II: сенатор Хейл ведёт расследование FCEES.
    Hearings,
    /// Книга II, гл. 4: инфляция и забастовки.
    Milk,
    /// Книга II, гл. 10: командир советской станции на связи.
    Gromov,
    /// Инженер советского ОКБ просит убежища и знает слепую зону радара.
    Defector,
    /// Н-1 взрывается на старте: Москва теряет ракету и полгода.
    N1Fire,
    /// Генеральный секретарь звонит по прямой линии.
    Hotline,
    /// Неурожай в СССР: Москва хочет купить американское зерно.
    Grain,
    /// Только при чрезвычайном положении: генералы требуют закрыть газеты.
    Censorship,
}

impl Event {
    pub const ALL: [Event; 11] = [
        Event::Bait,
        Event::Families,
        Event::Blueprints,
        Event::Hearings,
        Event::Milk,
        Event::Gromov,
        Event::Defector,
        Event::N1Fire,
        Event::Hotline,
        Event::Grain,
        Event::Censorship,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Event::Bait => "bait",
            Event::Families => "families",
            Event::Blueprints => "blueprints",
            Event::Hearings => "hale",
            Event::Milk => "milk",
            Event::Gromov => "gromov",
            Event::Defector => "defector",
            Event::N1Fire => "n1fire",
            Event::Hotline => "hotline",
            Event::Grain => "grain",
            Event::Censorship => "censorship",
        }
    }

    fn bit(self) -> u16 {
        1 << self as u16
    }
}

/// Записи досье GLASS EYE: ключи `lore.<имя>.name` и `lore.<имя>.text`.
/// Порядок — порядок показа в архиве.
pub const DOSSIER: [&str; 19] = [
    "glasseye", "fcees", "brenner", "sabatier", "snap", "dustclean", "n1", "los", "callahan",
    "delaney", "hale", "gromov", "sokolov", "hotline", "mccord", "lance", "ackerman", "politburo",
    "mars",
];

// ══════════════════════════════ СОСТОЯНИЕ ═══════════════════════════════

/// Узлы графа сцен. Тип `Copy`: состояние хранит значение, а не ссылку.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Location {
    Briefing,
    FundingOffice,
    DesignBureau,
    /// Заводы Boeing и Rockwell: сколько ракет строить.
    Factory,
    LaunchPad,
    Surface,
    UnitedNations,
    /// Авария на старте: корабль спасён системой САС, нужна легенда.
    Accident,
    /// Эпизод из досье между миссиями.
    Event(Event),
    /// СССР сел на Луну раньше, чем там поселились американцы.
    SovietFirst,
    /// Плановые миссии закончились, а ракеты третьей смены остались.
    SpareFlight,
    /// Финал после смены цели: Apollo 23 может уйти мимо Луны.
    MarsWindow,
    /// Последний кризис: советский корабль садится на обратной стороне.
    SovietLanding,
    /// Недовольство достигло предела: подавить бунт или подчиниться.
    Uprising,
    /// Казна пуста: военный заём или закрытие проекта.
    Bankrupt,
    /// Выстрел сделан: война на Луне идёт, но партия продолжается.
    MoonWar,
    /// Война переходит на Землю: ядерный порог.
    Brink,
    /// Спор в Политбюро: крепость на обратной стороне или марсианская гонка.
    Politburo,
    // Девятнадцать развязок:
    MarsEra,
    JointMars,
    Bypass,
    Handshake,
    OpenBase,
    SovietCollapse,
    MoonTreaty,
    ColdPeace,
    Outpost,
    MoonVictory,
    IronMoon,
    SecondPlace,
    RedMoon,
    Stranded,
    Cancelled,
    Impeached,
    Tribunal,
    Tactical,
    Armageddon,
}

/// Чем кончилась партия: цвет развязки на экране и в галерее.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Victory,
    /// Цель достигнута, но цена слишком высока.
    Pyrrhic,
    Defeat,
}

impl Outcome {
    pub fn key(self) -> &'static str {
        match self {
            Outcome::Victory => "ui.outcome.victory",
            Outcome::Pyrrhic => "ui.outcome.pyrrhic",
            Outcome::Defeat => "ui.outcome.defeat",
        }
    }
}

impl Location {
    /// Порядок развязок — их номера в игре и в галерее.
    pub const ENDINGS: [Location; 19] = [
        Location::MarsEra,
        Location::JointMars,
        Location::Bypass,
        Location::Handshake,
        Location::OpenBase,
        Location::SovietCollapse,
        Location::MoonTreaty,
        Location::ColdPeace,
        Location::Outpost,
        Location::MoonVictory,
        Location::IronMoon,
        Location::SecondPlace,
        Location::RedMoon,
        Location::Stranded,
        Location::Cancelled,
        Location::Impeached,
        Location::Tribunal,
        Location::Tactical,
        Location::Armageddon,
    ];

    /// Суффикс ключей в файле диалогов: `title.pad`, `text.pad`.
    pub fn key(self) -> &'static str {
        match self {
            Location::Briefing => "briefing",
            Location::FundingOffice => "funding",
            Location::DesignBureau => "design",
            Location::Factory => "factory",
            Location::LaunchPad => "pad",
            Location::Surface => "surface",
            Location::UnitedNations => "un",
            Location::Accident => "accident",
            Location::Event(event) => event.key(),
            Location::SovietFirst => "first",
            Location::SpareFlight => "spare",
            Location::MarsWindow => "window",
            Location::SovietLanding => "landing",
            Location::Uprising => "uprising",
            Location::Bankrupt => "bankrupt",
            Location::MoonWar => "war",
            Location::Brink => "brink",
            Location::Politburo => "politburo",
            Location::MarsEra => "mars",
            Location::JointMars => "joint",
            Location::Bypass => "bypass",
            Location::Handshake => "handshake",
            Location::OpenBase => "open",
            Location::SovietCollapse => "collapse",
            Location::MoonTreaty => "treaty",
            Location::ColdPeace => "coldpeace",
            Location::Outpost => "outpost",
            Location::MoonVictory => "victory",
            Location::IronMoon => "iron",
            Location::SecondPlace => "second",
            Location::RedMoon => "redmoon",
            Location::Stranded => "stranded",
            Location::Cancelled => "cancelled",
            Location::Impeached => "impeached",
            Location::Tribunal => "tribunal",
            Location::Tactical => "tactical",
            Location::Armageddon => "armageddon",
        }
    }

    /// Развязка по ключу: так галерея читает сохранённый список.
    pub fn ending_from_key(key: &str) -> Option<Location> {
        Location::ENDINGS.into_iter().find(|ending| ending.key() == key)
    }

    /// Номер развязки, считая с нуля.
    pub fn ending_index(self) -> Option<usize> {
        Location::ENDINGS.iter().position(|&ending| ending == self)
    }

    pub fn is_ending(self) -> bool {
        self.outcome().is_some()
    }

    /// Исход развязки. У остальных мест исхода нет.
    pub fn outcome(self) -> Option<Outcome> {
        use Location::*;
        Some(match self {
            MarsEra | JointMars | Bypass | Handshake | OpenBase | SovietCollapse | MoonTreaty
            | ColdPeace => Outcome::Victory,
            Outpost | MoonVictory | IronMoon => Outcome::Pyrrhic,
            SecondPlace | RedMoon | Stranded | Cancelled | Impeached | Tribunal | Tactical
            | Armageddon => Outcome::Defeat,
            _ => return None,
        })
    }

    /// Сцены решающих минут: под них звучит музыка кульминации.
    pub fn is_climax(self) -> bool {
        self.is_ending()
            || matches!(
                self,
                Location::SovietFirst
                    | Location::MarsWindow
                    | Location::SovietLanding
                    | Location::Accident
                    | Location::Uprising
                    | Location::Bankrupt
                    | Location::MoonWar
                    | Location::Brink
                    | Location::Politburo
            )
    }

    /// Подготовка проекта до первого пуска.
    pub fn is_preparation(self) -> bool {
        matches!(self, Location::Briefing | Location::FundingOffice | Location::DesignBureau | Location::Factory)
    }

    /// Заголовок сцены. У развязки перед названием стоит её номер.
    pub fn title(self, text: &Catalog) -> String {
        let name = text.get(&format!("title.{}", self.key()));
        match self.ending_index() {
            Some(index) => {
                let number = text
                    .get("ui.ending")
                    .replace("{i}", &(index + 1).to_string())
                    .replace("{n}", &Location::ENDINGS.len().to_string());
                format!("{number}. {name}")
            }
            None => name.to_string(),
        }
    }
}

/// Короткая сцена-анимация между ходами.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cutscene {
    /// Заставка книги трилогии: 1, 2 или 3.
    Chapter(u8),
    Launch(Mission),
    /// Авария на старте и спасение экипажа башней САС.
    Abort(Mission),
    Landing(Mission),
    /// Старт Н-1 с Байконура с модулем станции под этим номером.
    N1Launch(u8),
    /// Номер нового модуля советской станции, считая с единицы.
    SovietModule(u8),
    Intercept,
    SovietLanding,
    /// Проект «Копьё»: американский перехватчик бьёт по советской станции.
    Strike,
    /// Чрезвычайное положение: войска на улицах Вашингтона.
    Crackdown,
    /// Вспышки ядерных взрывов на Земле, как их видно с Луны.
    Flash,
}

/// Итог хода: текст сводки и анимации, которые нужно показать.
#[derive(Debug, Default)]
pub struct Turn {
    pub report: String,
    pub cutscenes: Vec<Cutscene>,
}

impl Turn {
    fn say(text: &str) -> Self {
        Self { report: text.to_string(), cutscenes: Vec::new() }
    }

    fn add(&mut self, text: &str) {
        if !self.report.is_empty() {
            self.report.push('\n');
        }
        self.report.push_str(text);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    First,
    Second,
}

impl Choice {
    fn number(self) -> u8 {
        match self {
            Choice::First => 1,
            Choice::Second => 2,
        }
    }
}

/// Состояние войны на Луне.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum War {
    /// Ни одного выстрела.
    Peace,
    /// Перемирие: советская оборона больше не стреляет.
    Truce,
    /// Советская станция уничтожена, Луна принадлежит США.
    Won,
}

/// Какой путь выбрала Москва в споре Политбюро.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stance {
    /// Спор ещё не решён.
    Unknown,
    /// Совместная марсианская программа.
    Partner,
    /// Крепость на обратной стороне любой ценой.
    Fortress,
}

/// Всё изменяемое состояние партии. Тип `Copy`: в нём нет кучи.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GameState {
    pub location: Location,
    /// Миссия, которая стоит на очереди или только что села.
    pub mission: Mission,
    pub delivered: Missions,
    /// Готовые к пуску ракеты и сколько всего построено.
    pub saturn_v: u8,
    pub saturn_built: u8,
    pub budget: i32,
    pub secrecy: i32,
    pub unrest: i32,
    pub tension: i32,
    pub base: i32,
    /// Месяцев с июня 1973 года.
    pub months: u32,
    pub soviet_modules: u8,
    /// СССР закрепился на Луне раньше постоянной американской смены.
    pub soviet_first: bool,
    /// После высадки СССР Америка назвала новой целью Марс.
    pub pivot: bool,
    /// Открыт канал связи с командиром советской станции.
    pub gromov: bool,
    /// Президент ввёл чрезвычайное положение: Конгресс распущен.
    pub autocracy: bool,
    pub war: War,
    /// Доверие Москвы, от 0 до `DETENTE_MAX`. Растёт от мирных жестов.
    pub detente: u8,
    pub stance: Stance,
    funded: bool,
    designed: bool,
    fcees: bool,
    disclosed: bool,
    un_pending: bool,
    landing_pending: bool,
    uprising_pending: bool,
    pending_event: Option<Event>,
    events_seen: u16,
    /// Все плановые миссии слетали или потеряны.
    campaign_over: bool,
    spare_done: bool,
    mars_reserve: bool,
    /// Аварию на старте выдали за взрыв макета.
    cover_up: bool,
    /// Перебежчик показал слепую зону советского радара.
    blind_spot: bool,
    /// Следующий советский пуск сорвётся: авария Н-1 или зерновая сделка.
    soviet_setback: bool,
    /// Чётность отсрочек: каждая вторая дарит Москве модуль.
    odd_delay: bool,
    /// Месяцев отсрочки с последнего пуска: они снижают риск аварии.
    rush: u8,
    /// Состояние генератора случайных событий. Хранится в партии,
    /// чтобы одна и та же партия всегда разыгрывалась одинаково.
    rng: u32,
}

impl Default for GameState {
    fn default() -> Self {
        Self::new()
    }
}

impl GameState {
    pub fn new() -> Self {
        Self::with_seed(1973)
    }

    /// Новая партия с заданным зерном случайных событий.
    pub fn with_seed(seed: u32) -> Self {
        Self {
            location: Location::Briefing,
            mission: Mission::Apollo18,
            delivered: Missions::default(),
            saturn_v: SATURN_PLAN,
            saturn_built: SATURN_PLAN,
            budget: 0,
            secrecy: 50,
            unrest: 0,
            tension: 10,
            base: 0,
            months: 0,
            soviet_modules: 0,
            soviet_first: false,
            pivot: false,
            gromov: false,
            autocracy: false,
            war: War::Peace,
            detente: 0,
            stance: Stance::Unknown,
            funded: false,
            designed: false,
            fcees: false,
            disclosed: false,
            un_pending: false,
            landing_pending: false,
            uprising_pending: false,
            pending_event: None,
            events_seen: 0,
            campaign_over: false,
            spare_done: false,
            mars_reserve: false,
            cover_up: false,
            blind_spot: false,
            soviet_setback: false,
            odd_delay: false,
            rush: 0,
            rng: seed.max(1),
        }
    }

    /// Состояние для витрины галереи: развязка на фоне достроенных баз.
    pub fn showcase(location: Location) -> Self {
        let mut state = Self::new();
        state.location = location;
        state.soviet_modules = SOVIET_MODULES;
        state.saturn_built = SATURN_LIMIT;
        state.saturn_v = 0;
        state.mission = Mission::Apollo23;
        state.delivered = Mission::ALL[..6].iter().fold(Missions::default(), |set, &m| set.with(m));
        match location {
            Location::Cancelled => {
                state.delivered = Missions::default().with(Mission::Apollo18).with(Mission::Apollo19);
                state.saturn_v = 4;
                state.soviet_modules = 2;
            }
            Location::Stranded | Location::Outpost => {
                state.delivered = state.delivered.without(Mission::Apollo23);
            }
            Location::MoonVictory | Location::IronMoon => state.war = War::Won,
            _ => {}
        }
        state.autocracy = matches!(location, Location::IronMoon | Location::Armageddon);
        state
    }

    /// Оборонный модуль на орбите и станция цела.
    pub fn defence_active(&self) -> bool {
        self.soviet_modules >= DEFENCE_MODULE && self.war != War::Won
    }

    /// Собьёт ли советская оборона корабль на трассе Земля — Луна.
    fn can_intercept(&self) -> bool {
        let limit = if self.blind_spot { TENSION_LIMIT_BLIND } else { TENSION_LIMIT };
        self.defence_active() && self.war != War::Truce && self.tension >= limit
    }

    /// Ракет для новых пусков нет: дальше только финальный кризис.
    pub fn final_phase(&self) -> bool {
        self.saturn_v == 0 || (self.campaign_over && self.spare_done)
    }

    /// Окно к Луне закрывается: дальше откладывать нельзя.
    pub fn window_closing(&self) -> bool {
        self.location == Location::LaunchPad && self.rush >= MAX_RUSH
    }

    /// Календарная дата: отсчёт от июня 1973 года.
    pub fn date(&self) -> String {
        const NAMES: [&str; 12] = [
            "январь", "февраль", "март", "апрель", "май", "июнь", "июль", "август", "сентябрь",
            "октябрь", "ноябрь", "декабрь",
        ];
        let total = 5 + self.months; // июнь имеет индекс 5
        format!("{} {}", NAMES[(total % 12) as usize], 1973 + total / 12)
    }

    pub fn title(&self, text: &Catalog) -> String {
        self.location.title(text)
    }

    /// Текст текущей сцены, собранный из файла диалогов и состояния.
    pub fn description(&self, text: &Catalog) -> String {
        let mission = self.mission.key();
        let name = text.get(&format!("{mission}.name"));
        let line = |out: &mut String, key: &str| {
            out.push('\n');
            out.push_str(text.get(key));
        };
        match self.location {
            Location::LaunchPad => {
                let mut out = text
                    .get("text.pad")
                    .replace("{name}", name)
                    .replace("{cargo}", text.get(&format!("{mission}.cargo")))
                    .replace("{cost}", &self.mission.cost().to_string());
                match self.war {
                    War::Won => line(&mut out, "pad.clear"),
                    War::Truce if self.defence_active() => line(&mut out, "pad.truce"),
                    _ if self.defence_active() => line(&mut out, "pad.defence"),
                    _ => {}
                }
                if self.rush >= MAX_RUSH {
                    line(&mut out, "pad.window");
                }
                out
            }
            Location::Surface => text.get(&format!("{mission}.report")).to_string(),
            Location::Accident => text.get("text.accident").replace("{name}", name),
            Location::SpareFlight => text.get("text.spare").replace("{left}", &self.saturn_v.to_string()),
            Location::Bankrupt => text.get("text.bankrupt").replace("{cost}", &self.mission.cost().to_string()),
            // После ранней высадки СССР финальный кризис звучит иначе.
            Location::SovietLanding if self.soviet_first => text.get("text.landing.race").to_string(),
            Location::Uprising if self.autocracy => text.get("text.uprising.junta").to_string(),
            Location::MoonWar if self.final_phase() => {
                let mut out = text.get("text.war").to_string();
                line(&mut out, "war.decisive");
                out
            }
            other => text.get(&format!("text.{}", other.key())).to_string(),
        }
    }

    /// Послесловие развязки: что стало с миром через годы.
    pub fn epilogue<'a>(&self, text: &'a Catalog) -> Option<&'a str> {
        self.location.is_ending().then(|| text.get(&format!("epi.{}", self.location.key())))
    }

    /// Два варианта для сцены. У развязок вариантов нет.
    pub fn options(&self, text: &Catalog) -> Option<[String; 2]> {
        if self.location.is_ending() {
            return None;
        }
        let label = |n: u8| {
            let key = match self.location {
                Location::Surface => format!("{}.opt{n}", self.mission.key()),
                Location::LaunchPad if n == 2 && self.rush >= MAX_RUSH => "opt2.pad.skip".to_string(),
                Location::Uprising if self.autocracy => format!("opt{n}.uprising.junta"),
                other => format!("opt{n}.{}", other.key()),
            };
            text.get(&key).to_string()
        };
        Some([label(1), label(2)])
    }

    /// Открытые записи досье в порядке показа.
    pub fn dossier(&self) -> Vec<&'static str> {
        DOSSIER.into_iter().filter(|key| self.knows(key)).collect()
    }

    /// Открытые записи досье битами: сравнение до и после хода без списков.
    fn dossier_mask(&self) -> u32 {
        DOSSIER
            .iter()
            .enumerate()
            .filter(|(_, key)| self.knows(key))
            .fold(0, |mask, (i, _)| mask | 1 << i)
    }

    fn seen(&self, event: Event) -> bool {
        self.events_seen & event.bit() != 0
    }

    fn knows(&self, key: &str) -> bool {
        let has = |mission: Mission| self.delivered.contains(mission);
        match key {
            "glasseye" => true,
            "fcees" => self.fcees,
            "brenner" => self.saturn_built > self.saturn_v,
            "sabatier" => has(Mission::Apollo18),
            "snap" => has(Mission::Apollo19),
            "dustclean" => has(Mission::Apollo20),
            "n1" => self.soviet_modules >= 1,
            "los" => self.soviet_modules >= 2,
            "callahan" => self.seen(Event::Bait),
            "delaney" => self.seen(Event::Families) || has(Mission::Apollo22),
            "hale" => self.seen(Event::Hearings),
            "gromov" => self.seen(Event::Gromov) || self.defence_active(),
            "sokolov" => self.seen(Event::Defector),
            "hotline" => self.seen(Event::Hotline),
            "mccord" => self.autocracy,
            "lance" => self.war != War::Peace || matches!(self.location, Location::MoonWar | Location::Brink),
            "ackerman" => has(Mission::Apollo23),
            "politburo" => self.stance != Stance::Unknown,
            "mars" => self.pivot || self.mars_reserve,
            _ => false,
        }
    }

    /// Применяет выбор игрока. Возвращает сводку и список анимаций.
    /// Если ход открыл новые записи досье, сводка о них сообщает.
    pub fn apply(&mut self, choice: Choice, text: &Catalog) -> Turn {
        let known = self.dossier_mask();
        let mut turn = self.resolve(choice, text);
        let fresh = self.dossier_mask() & !known;
        if fresh != 0 {
            let names: Vec<String> = DOSSIER
                .iter()
                .enumerate()
                .filter(|(i, _)| fresh & 1 << i != 0)
                .map(|(_, key)| format!("«{}»", text.get(&format!("lore.{key}.name"))))
                .collect();
            turn.add(&text.get("dossier.new").replace("{names}", &names.join(", ")));
        }
        turn
    }

    fn resolve(&mut self, choice: Choice, text: &Catalog) -> Turn {
        let n = choice.number();
        match self.location {
            Location::Briefing => {
                self.location = match choice {
                    Choice::First => Location::FundingOffice,
                    Choice::Second => Location::DesignBureau,
                };
                Turn::say(text.get(&format!("res{n}.briefing")))
            }
            Location::FundingOffice => {
                match choice {
                    Choice::First => {
                        self.fcees = true;
                        self.change(Effects { budget: 220, secrecy: -20, ..Effects::NONE });
                    }
                    Choice::Second => {
                        self.change(Effects { budget: 150, secrecy: -5, ..Effects::NONE });
                    }
                }
                self.finish_preparation(true);
                Turn::say(text.get(&format!("res{n}.funding")))
            }
            Location::DesignBureau => {
                match choice {
                    Choice::First => self.change(Effects { budget: -10, ..Effects::NONE }),
                    Choice::Second => {
                        self.change(Effects { budget: -30, base: 15, ..Effects::NONE })
                    }
                }
                self.finish_preparation(false);
                Turn::say(text.get(&format!("res{n}.design")))
            }
            Location::Factory => {
                match choice {
                    // Третья смена: две лишние ракеты, но об огромном заказе
                    // начинают говорить в профсоюзах.
                    Choice::First => {
                        self.change(Effects {
                            budget: -THIRD_SHIFT_COST,
                            secrecy: -8,
                            unrest: 5,
                            ..Effects::NONE
                        });
                        let extra = SATURN_LIMIT - self.saturn_built;
                        self.saturn_v += extra;
                        self.saturn_built += extra;
                    }
                    Choice::Second => self.change(Effects { secrecy: 5, ..Effects::NONE }),
                }
                self.months += 4;
                self.location = Location::LaunchPad;
                Turn::say(text.get(&format!("res{n}.factory")))
            }
            Location::LaunchPad => match choice {
                Choice::First => self.launch(text),
                Choice::Second if self.rush >= MAX_RUSH => self.skip_mission(text),
                Choice::Second => self.delay(text),
            },
            Location::Surface => self.resolve_surface(choice, text),
            Location::UnitedNations => {
                self.un_pending = false;
                match choice {
                    Choice::First => {
                        self.add_detente();
                        self.change(Effects { budget: 20, unrest: -10, tension: -20, ..Effects::NONE })
                    }
                    Choice::Second => {
                        self.change(Effects { unrest: 20, tension: 15, ..Effects::NONE })
                    }
                }
                self.settle(Turn::say(text.get(&format!("res{n}.un"))))
            }
            Location::Accident => self.resolve_accident(choice, text),
            Location::Event(event) => {
                self.event_effects(event, choice);
                self.settle(Turn::say(text.get(&format!("res{n}.{}", event.key()))))
            }
            Location::SovietFirst => {
                match choice {
                    // Сменить игру: Луна разделена, цель теперь Марс.
                    Choice::First => {
                        self.pivot = true;
                        self.change(Effects { tension: -15, unrest: 5, ..Effects::NONE });
                    }
                    // Продолжать гонку: Конгресс даёт деньги, Москва нервничает.
                    Choice::Second => self.change(Effects {
                        budget: 30,
                        unrest: -10,
                        tension: 15,
                        ..Effects::NONE
                    }),
                }
                self.settle(Turn::say(text.get(&format!("res{n}.first"))))
            }
            Location::SpareFlight => match choice {
                Choice::First => self.spare_flight(text),
                Choice::Second => {
                    self.spare_done = true;
                    self.mars_reserve = true;
                    let mut turn = Turn::say(text.get("res2.spare"));
                    self.proceed(&mut turn);
                    turn
                }
            },
            Location::MarsWindow => self.resolve_window(choice, text),
            Location::SovietLanding => self.resolve_landing(choice, text),
            Location::Uprising => self.resolve_uprising(choice, text),
            Location::Bankrupt => match choice {
                // Деньги Пентагона: пуск состоится, но страна видит, куда уходит оборонный бюджет.
                Choice::First => {
                    let anger = if self.autocracy { 5 } else { 12 };
                    self.change(Effects { budget: WAR_LOAN, tension: 8, unrest: anger, ..Effects::NONE });
                    self.settle(Turn::say(text.get("res1.bankrupt")))
                }
                Choice::Second => {
                    self.location = self.broke_ending();
                    Turn::say(text.get("res2.bankrupt"))
                }
            },
            Location::MoonWar => self.resolve_war(choice, text),
            Location::Brink => self.resolve_brink(choice, text),
            Location::Politburo => {
                let key = match choice {
                    // Совместный Марс возможен, только если Москва нам уже верит.
                    Choice::First if self.gromov || self.detente >= 1 => {
                        self.stance = Stance::Partner;
                        self.change(Effects { tension: -10, ..Effects::NONE });
                        "res1.politburo"
                    }
                    Choice::First => {
                        self.stance = Stance::Fortress;
                        self.change(Effects { tension: 5, ..Effects::NONE });
                        "res1.politburo.rejected"
                    }
                    Choice::Second => {
                        self.stance = Stance::Fortress;
                        self.change(Effects { tension: 10, unrest: -5, ..Effects::NONE });
                        "res2.politburo"
                    }
                };
                self.settle(Turn::say(text.get(key)))
            }
            // Развязки: ввод выбора на них не принимается.
            _ => Turn::say(text.get("over")),
        }
    }

    /// Складывает изменения и удерживает шкалы в границах 0–100.
    fn change(&mut self, e: Effects) {
        self.budget += e.budget;
        self.secrecy = (self.secrecy + e.secrecy).clamp(0, 100);
        self.unrest = (self.unrest + e.unrest).clamp(0, 100);
        self.tension = (self.tension + e.tension).clamp(0, 100);
        self.base = (self.base + e.base).clamp(0, 100);
    }

    fn add_detente(&mut self) {
        self.detente = (self.detente + 1).min(DETENTE_MAX);
    }

    /// Обычное завершение хода: проверка бунта и переход к следующей сцене.
    fn settle(&mut self, mut turn: Turn) -> Turn {
        self.check_unrest();
        self.proceed(&mut turn);
        turn
    }

    /// Бросок с вероятностью `percent` процентов.
    fn roll(&mut self, percent: u32) -> bool {
        self.dice() % 100 < percent
    }

    fn dice(&mut self) -> u32 {
        self.rng = self.rng.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        (self.rng >> 16) & 0x7fff
    }

    /// Финансы и проект обязательны оба; порядок выбирает игрок.
    /// Затем заводы решают, сколько ракет строить.
    fn finish_preparation(&mut self, funding: bool) {
        if funding {
            self.funded = true;
            self.months += 6;
        } else {
            self.designed = true;
            self.months += 8;
        }
        self.location = match (self.funded, self.designed) {
            (true, true) => Location::Factory,
            (true, false) => Location::DesignBureau,
            (false, _) => Location::FundingOffice,
        };
    }

    fn launch(&mut self, text: &Catalog) -> Turn {
        let cost = self.mission.cost();
        if self.budget < cost {
            // Пустая касса — не конец: можно взять деньги Пентагона.
            self.location = Location::Bankrupt;
            return Turn::say(text.get("launch.broke"));
        }
        self.budget -= cost;
        self.saturn_v = self.saturn_v.saturating_sub(1);

        // Спешка опасна: каждый месяц отсрочки снижает риск аварии.
        let risk = ACCIDENT_RISK
            .saturating_sub(self.rush as u32 * ACCIDENT_RISK_PER_DELAY)
            .max(ACCIDENT_RISK_MIN);
        self.rush = 0;

        if self.can_intercept() {
            // Корабль сбит. Это война, но не конец партии.
            let mut turn = Turn {
                report: text.get("launch.lost").to_string(),
                cutscenes: vec![Cutscene::Launch(self.mission), Cutscene::Intercept],
            };
            self.advance_mission();
            self.start_war(&mut turn, text);
            return turn;
        }

        if self.roll(risk) {
            self.location = Location::Accident;
            return Turn {
                report: text.get("launch.abort").to_string(),
                cutscenes: vec![Cutscene::Abort(self.mission)],
            };
        }

        let mut turn = Turn::say(text.get("launch.ok"));
        turn.cutscenes = vec![Cutscene::Launch(self.mission), Cutscene::Landing(self.mission)];
        if !self.disclosed && self.secrecy < LAUNCH_SECRECY_THRESHOLD {
            self.disclosed = true;
            self.change(Effects { unrest: 25, ..Effects::NONE });
            turn.add(text.get("launch.leak"));
        }
        self.location = Location::Surface;
        turn
    }

    fn broke_ending(&self) -> Location {
        if self.delivered.contains(Mission::Apollo22) {
            Location::Stranded
        } else {
            Location::Cancelled
        }
    }

    fn delay(&mut self, text: &Catalog) -> Turn {
        self.months += 1;
        self.rush += 1;
        self.change(Effects { budget: -5, secrecy: 10, ..Effects::NONE });
        let mut turn = Turn::say(text.get("delay.ok"));
        if self.disclosed {
            self.change(Effects { unrest: 5, ..Effects::NONE });
            turn.add(text.get("delay.bored"));
        }
        // Соперник не ждёт: каждая вторая задержка дарит ему модуль.
        self.odd_delay = !self.odd_delay;
        if !self.odd_delay && self.months >= 17 {
            self.soviet_turn(&mut turn, text);
        }
        self.settle(turn)
    }

    /// Окно закрылось: миссию снимают с графика, ракета остаётся на заводе.
    fn skip_mission(&mut self, text: &Catalog) -> Turn {
        self.rush = 0;
        self.months += 1;
        self.change(Effects { secrecy: 5, ..Effects::NONE });
        let name = text.get(&format!("{}.name", self.mission.key()));
        let mut turn = Turn::say(&text.get("skip.ok").replace("{name}", name));
        self.advance_mission();
        if self.months >= 17 {
            self.soviet_turn(&mut turn, text);
        }
        self.settle(turn)
    }

    fn resolve_surface(&mut self, choice: Choice, text: &Catalog) -> Turn {
        let [first, second] = self.mission.effects();
        self.change(match choice {
            Choice::First => first,
            Choice::Second => second,
        });
        self.change(Effects { base: 8, ..Effects::NONE }); // сам груз тоже усиливает базу
        self.delivered = self.delivered.with(self.mission);
        self.months += 3;

        let key = format!("{}.res{}", self.mission.key(), choice.number());
        let mut turn = Turn::say(text.get(&key));

        // Событие квартала разыгрывается до ответа Москвы.
        self.draw_event();
        // Ход соперника: с Apollo 19 каждая наша миссия отвечается модулем.
        if self.mission != Mission::Apollo18 {
            self.soviet_turn(&mut turn, text);
        }
        // После огласки страна платит недовольством за каждый квартал.
        // Цензура чрезвычайного положения глушит половину гнева.
        if self.disclosed {
            let anger = match (self.fcees, self.autocracy) {
                (true, false) => 15,
                (true, true) => 8,
                (false, false) => 4,
                (false, true) => 2,
            };
            self.change(Effects { unrest: anger, ..Effects::NONE });
        }
        self.advance_mission();
        self.settle(turn)
    }

    /// После аварии экипаж жив, но легенду нужно выбрать сейчас.
    fn resolve_accident(&mut self, choice: Choice, text: &Catalog) -> Turn {
        match choice {
            Choice::First => {
                self.cover_up = true;
                // Пока о проекте молчат, легенде о макете верят.
                let doubt = if self.disclosed { 15 } else { 0 };
                self.change(Effects { secrecy: 5, unrest: doubt, ..Effects::NONE });
            }
            Choice::Second => {
                self.disclosed = true;
                self.change(Effects { secrecy: -25, unrest: 10, tension: -5, ..Effects::NONE });
            }
        }
        let mut turn = Turn::say(text.get(&format!("res{}.accident", choice.number())));
        self.months += 2;
        // Если запасной ракеты нет, груз этой миссии потерян.
        if self.saturn_v < self.mission.remaining() {
            let name = text.get(&format!("{}.name", self.mission.key()));
            turn.add(&text.get("accident.lost").replace("{name}", name));
            self.advance_mission();
        } else {
            turn.add(text.get("accident.retry"));
        }
        if self.months >= 17 {
            self.soviet_turn(&mut turn, text);
        }
        self.settle(turn)
    }

    /// Грузовой рейс на ракете третьей смены.
    fn spare_flight(&mut self, text: &Catalog) -> Turn {
        let mission = Mission::Reserve;
        if self.budget < mission.cost() {
            self.spare_done = true;
            let mut turn = Turn::say(text.get("spare.broke"));
            self.proceed(&mut turn);
            return turn;
        }
        self.budget -= mission.cost();
        self.saturn_v -= 1;
        if self.can_intercept() {
            self.spare_done = true;
            let mut turn = Turn {
                report: text.get("launch.lost").to_string(),
                cutscenes: vec![Cutscene::Launch(mission), Cutscene::Intercept],
            };
            self.start_war(&mut turn, text);
            return turn;
        }
        self.change(Effects { base: 12, tension: 6, ..Effects::NONE });
        self.delivered = self.delivered.with(mission);
        self.months += 2;
        let mut turn = Turn::say(text.get("res1.spare"));
        turn.cutscenes = vec![Cutscene::Launch(mission), Cutscene::Landing(mission)];
        self.proceed(&mut turn);
        turn
    }

    /// Финал после смены цели: уйти мимо Луны или остаться и делить её.
    fn resolve_window(&mut self, choice: Choice, text: &Catalog) -> Turn {
        let calm = self.tension < WAR_TENSION;
        let mut turn = Turn::say(text.get(&format!("res{}.window", choice.number())));
        self.location = match choice {
            // Увести Apollo 23 к Солнцу мимо советского гарнизона.
            Choice::First if !self.delivered.contains(Mission::Apollo23) => Location::SecondPlace,
            Choice::First if self.can_intercept() => {
                turn.add(text.get("window.lost"));
                turn.cutscenes.push(Cutscene::Intercept);
                self.start_war(&mut turn, text);
                return turn;
            }
            Choice::First if self.base >= BYPASS_BASE => Location::Bypass,
            Choice::First => Location::SecondPlace,
            // Остаться и делить Луну.
            Choice::Second if self.stance == Stance::Partner && calm => Location::JointMars,
            Choice::Second if self.gromov && calm => Location::Handshake,
            Choice::Second if self.base >= OPEN_BASE => Location::ColdPeace,
            Choice::Second => Location::SecondPlace,
        };
        turn
    }

    /// Последний кризис: открыть базу миру или поставить ультиматум.
    fn resolve_landing(&mut self, choice: Choice, text: &Catalog) -> Turn {
        let mars_base = if self.mars_reserve { MARS_BASE_RESERVE } else { MARS_BASE };
        let calm = self.tension < WAR_TENSION;
        let mut turn = Turn::say(text.get(&format!("res{}.landing", choice.number())));
        self.location = match choice {
            // Открыть базу миру: исход зависит от того, что успели построить
            // и как договорились с Москвой.
            Choice::First if self.stance == Stance::Partner && calm && self.base >= BYPASS_BASE => {
                Location::JointMars
            }
            // На Марс уходят только с крепкой базой и спокойным тылом.
            Choice::First if self.base >= mars_base && calm => Location::MarsEra,
            // Канал с Громовым превращает открытую базу в совместную.
            Choice::First if self.gromov && calm => Location::Handshake,
            Choice::First if self.base >= OPEN_BASE => Location::OpenBase,
            Choice::First => Location::Outpost,
            // Ультиматум при высокой напряжённости — это война. Но не проигрыш.
            Choice::Second if !calm => {
                self.start_war(&mut turn, text);
                return turn;
            }
            // Москва, выбравшая крепость, надрывается в гонке, которую не потянет.
            Choice::Second if self.stance == Stance::Fortress && self.base >= OPEN_BASE => {
                Location::SovietCollapse
            }
            Choice::Second => Location::ColdPeace,
        };
        turn
    }

    /// Чрезвычайное положение всегда доступно: доктрина не знает отступления.
    fn resolve_uprising(&mut self, choice: Choice, text: &Catalog) -> Turn {
        match choice {
            Choice::First => {
                let key = if self.autocracy { "res1.uprising.junta" } else { "res1.uprising" };
                self.autocracy = true;
                self.unrest = self.unrest.min(UNREST_AFTER_CRACKDOWN);
                self.change(Effects { secrecy: 20, tension: 10, budget: 30, ..Effects::NONE });
                let mut turn = Turn::say(text.get(key));
                turn.cutscenes.push(Cutscene::Crackdown);
                self.proceed(&mut turn);
                turn
            }
            Choice::Second => {
                // Если страна узнала о скрытой аварии, президента судят, а не отстраняют.
                self.location = if self.cover_up { Location::Tribunal } else { Location::Impeached };
                let key = if self.autocracy { "res2.uprising.junta" } else { "res2.uprising" };
                Turn::say(text.get(key))
            }
        }
    }

    /// Первый выстрел сделан. Нация сплачивается, Москва готовит ответ.
    fn start_war(&mut self, turn: &mut Turn, text: &Catalog) {
        self.change(Effects { unrest: -15, tension: 10, ..Effects::NONE });
        self.location = Location::MoonWar;
        turn.add(text.get("war.begins"));
    }

    /// Победа в войне: кто держит Луну, решает строй, а не флаг.
    fn victory_ending(&self) -> Location {
        if self.autocracy {
            Location::IronMoon
        } else {
            Location::MoonVictory
        }
    }

    fn resolve_war(&mut self, choice: Choice, text: &Catalog) -> Turn {
        let decisive = self.final_phase();
        match choice {
            // Проект «Копьё»: оружие, о котором лгали КГБ, существует.
            Choice::First => {
                self.war = War::Won;
                self.change(Effects { tension: 25, unrest: -10, secrecy: -10, ..Effects::NONE });
                let mut turn = Turn::say(text.get("res1.war"));
                turn.cutscenes.push(Cutscene::Strike);
                if self.tension >= BRINK_TENSION {
                    self.location = Location::Brink;
                    turn.add(text.get("war.brink"));
                    return turn;
                }
                if decisive {
                    self.location = self.victory_ending();
                    return turn;
                }
                self.settle(turn)
            }
            // Перемирие: условия зависят от того, верит ли нам Москва.
            Choice::Second => {
                let trusted = self.gromov || self.detente >= 1;
                self.change(Effects { tension: -25, ..Effects::NONE });
                let key = if trusted {
                    "res2.war"
                } else {
                    // Москва диктует: часть базы демонтируют под присмотром станции.
                    self.change(Effects { base: -15, unrest: 15, ..Effects::NONE });
                    "res2.war.dictated"
                };
                let turn = Turn::say(text.get(key));
                if decisive {
                    self.location = if trusted { Location::MoonTreaty } else { Location::RedMoon };
                    return turn;
                }
                self.war = War::Truce;
                self.settle(turn)
            }
        }
    }

    /// Ядерный порог. Тотальная война возможна только у хунты:
    /// у республики остаются сдержки, у генералов — нет.
    fn resolve_brink(&mut self, choice: Choice, text: &Catalog) -> Turn {
        match choice {
            Choice::First => {
                self.location = if self.autocracy { Location::Armageddon } else { Location::Tactical };
                Turn { report: text.get("res1.brink").to_string(), cutscenes: vec![Cutscene::Flash] }
            }
            Choice::Second => {
                self.change(Effects { tension: -30, ..Effects::NONE });
                let turn = Turn::say(text.get("res2.brink"));
                if self.final_phase() {
                    self.location = self.victory_ending();
                    return turn;
                }
                self.settle(turn)
            }
        }
    }

    /// Следующая плановая миссия или конец кампании.
    fn advance_mission(&mut self) {
        match self.mission.next() {
            Some(next) => self.mission = next,
            None => self.campaign_over = true,
        }
    }

    /// Куда идти дальше: срочные кризисы раньше событий, события раньше пусков.
    fn proceed(&mut self, turn: &mut Turn) {
        if self.location.is_ending() {
            return;
        }
        if self.uprising_pending {
            self.uprising_pending = false;
            self.location = Location::Uprising;
            return;
        }
        if self.landing_pending {
            self.landing_pending = false;
            turn.cutscenes.extend([Cutscene::Chapter(3), Cutscene::SovietLanding]);
            self.location = Location::SovietFirst;
            return;
        }
        if self.un_pending {
            self.location = Location::UnitedNations;
            return;
        }
        if let Some(event) = self.pending_event.take() {
            self.location = Location::Event(event);
            return;
        }
        if !self.campaign_over && self.saturn_v > 0 {
            self.location = Location::LaunchPad;
            return;
        }
        if self.saturn_v > 0 && !self.spare_done {
            self.location = Location::SpareFlight;
            return;
        }
        // Ракет больше нет. Если советской станции уже нет, исход решён.
        if self.war == War::Won {
            self.location = self.victory_ending();
            return;
        }
        // Перед последним кризисом Москва выбирает свой путь.
        if self.stance == Stance::Unknown {
            self.location = Location::Politburo;
            return;
        }
        self.location = if self.soviet_first && self.pivot {
            Location::MarsWindow
        } else {
            if !self.soviet_first {
                turn.cutscenes.extend([Cutscene::Chapter(3), Cutscene::SovietLanding]);
            }
            Location::SovietLanding
        };
    }

    /// С вероятностью выбирает событие из тех, что ещё не случались.
    /// Список готовых событий не собирается в память: их не больше одиннадцати.
    fn draw_event(&mut self) {
        if !self.roll(EVENT_CHANCE) {
            return;
        }
        let ready = |state: &Self, event: &Event| !state.seen(*event) && state.event_ready(*event);
        let count = Event::ALL.iter().filter(|e| ready(self, e)).count();
        if count == 0 {
            return;
        }
        let pick = self.dice() as usize % count;
        let Some(event) = Event::ALL.into_iter().filter(|e| ready(self, e)).nth(pick) else { return };
        self.events_seen |= event.bit();
        self.pending_event = Some(event);
    }

    fn event_ready(&self, event: Event) -> bool {
        match event {
            // Обмануть КГБ можно, пока Москва не объявила о проекте.
            Event::Bait => self.soviet_modules == 0,
            Event::Families => !self.delivered.is_empty(),
            Event::Blueprints => (2..DEFENCE_MODULE).contains(&self.soviet_modules),
            // При чрезвычайном положении Сенат распущен: слушать некому.
            Event::Hearings => self.disclosed && !self.autocracy,
            Event::Milk => self.disclosed && self.fcees,
            Event::Gromov => self.defence_active(),
            Event::Defector => (1..=DEFENCE_MODULE).contains(&self.soviet_modules),
            Event::N1Fire => (1..SOVIET_MODULES).contains(&self.soviet_modules),
            Event::Hotline => self.soviet_modules >= 1 && self.tension >= 40,
            Event::Grain => self.disclosed,
            Event::Censorship => self.autocracy,
        }
    }

    fn event_effects(&mut self, event: Event, choice: Choice) {
        let n = Effects::NONE;
        let effects = match (event, choice) {
            // Москва смотрит на околоземную орбиту, а не на Луну.
            (Event::Bait, Choice::First) => Effects { secrecy: 15, tension: 5, ..n },
            (Event::Bait, Choice::Second) => Effects { budget: 10, ..n },
            (Event::Families, Choice::First) => Effects { secrecy: -8, unrest: -5, ..n },
            (Event::Families, Choice::Second) => Effects { secrecy: 5, unrest: 5, ..n },
            (Event::Blueprints, Choice::First) => Effects { tension: -10, secrecy: -5, ..n },
            (Event::Blueprints, Choice::Second) => Effects { base: 6, budget: -10, ..n },
            (Event::Hearings, Choice::First) => Effects { unrest: -15, budget: -20, ..n },
            (Event::Hearings, Choice::Second) => Effects { unrest: 10, secrecy: 10, ..n },
            (Event::Milk, Choice::First) => Effects { unrest: -15, budget: -30, ..n },
            (Event::Milk, Choice::Second) => Effects { unrest: 8, budget: 10, ..n },
            (Event::Gromov, Choice::First) => Effects { tension: -15, secrecy: -10, ..n },
            (Event::Gromov, Choice::Second) => Effects { tension: 5, ..n },
            (Event::Defector, Choice::First) => Effects { tension: 10, secrecy: -5, ..n },
            (Event::Defector, Choice::Second) => Effects { tension: -10, unrest: 5, ..n },
            (Event::N1Fire, Choice::First) => Effects { tension: -12, unrest: 5, ..n },
            (Event::N1Fire, Choice::Second) => Effects { tension: 8, unrest: -10, ..n },
            (Event::Hotline, Choice::First) => Effects { tension: -20, secrecy: -8, unrest: 5, ..n },
            (Event::Hotline, Choice::Second) => Effects { tension: 10, unrest: -5, ..n },
            (Event::Grain, Choice::First) => Effects { budget: 25, tension: -8, unrest: 5, ..n },
            (Event::Grain, Choice::Second) => Effects { tension: 10, unrest: -8, ..n },
            (Event::Censorship, Choice::First) => Effects { unrest: -20, secrecy: 15, tension: 5, ..n },
            (Event::Censorship, Choice::Second) => Effects { unrest: 10, secrecy: -10, ..n },
        };
        self.change(effects);
        match (event, choice) {
            (Event::Gromov, Choice::First) => self.gromov = true,
            (Event::Defector, Choice::First) => self.blind_spot = true,
            (Event::Defector, Choice::Second) | (Event::Hotline, Choice::First) => self.add_detente(),
            // Сгоревшая Н-1 стоит Москве одного пуска; соболезнования ещё и греют отношения.
            (Event::N1Fire, Choice::First) => {
                self.add_detente();
                self.soviet_setback = true;
            }
            // Проданное зерно Москва оплачивает паузой в пусках.
            (Event::N1Fire, Choice::Second) | (Event::Grain, Choice::First) => self.soviet_setback = true,
            _ => {}
        }
    }

    /// Добавляет модуль советской станции, если она ещё не достроена.
    fn soviet_turn(&mut self, turn: &mut Turn, text: &Catalog) {
        if self.war == War::Won || self.soviet_modules >= SOVIET_MODULES {
            return;
        }
        if self.soviet_setback {
            self.soviet_setback = false;
            turn.add(&format!("{} {}", text.get("moscow"), text.get("soviet.setback")));
            return;
        }
        self.soviet_modules += 1;
        let number = self.soviet_modules;
        let tension = if number == DEFENCE_MODULE { 25 } else { 10 };
        self.change(Effects { tension, ..Effects::NONE });
        if number == 1 {
            // Первый модуль сопровождается заявлением КГБ. Начинается книга вторая.
            turn.cutscenes.push(Cutscene::Chapter(2));
            self.disclosed = true;
            self.un_pending = true;
            self.change(Effects { unrest: 10, ..Effects::NONE });
            if self.cover_up {
                // КГБ показывает снимки спасённой капсулы: ложь о макете раскрыта.
                self.change(Effects { unrest: 20, ..Effects::NONE });
                turn.add(text.get("cover.exposed"));
            }
        }
        let news = text.get(&format!("soviet.{number}"));
        turn.add(&format!("{} {news}", text.get("moscow")));
        turn.cutscenes.extend([Cutscene::N1Launch(number), Cutscene::SovietModule(number)]);

        // Станция достроена, а американцев на Луне всё ещё нет: СССР садится
        // первым. Это не конец партии, а новый кризис.
        if number == SOVIET_MODULES && !self.delivered.contains(Mission::Apollo22) {
            self.soviet_first = true;
            self.landing_pending = true;
        }
    }

    /// Недовольство на пределе больше не обрывает партию: ход ведёт
    /// к кризису власти, где игрок решает, подавить бунт или уйти.
    fn check_unrest(&mut self) {
        if self.unrest >= UNREST_LIMIT && !self.location.is_ending() {
            self.uprising_pending = true;
        }
    }
}

// ════════════════════════════════ ТЕСТЫ ═════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::MISSING;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn date_starts_in_june_1973() {
        let mut state = GameState::new();
        assert_eq!(state.date(), "июнь 1973");
        state.months = 7;
        assert_eq!(state.date(), "январь 1974");
    }

    /// Сколько партий разыгрывает проверка достижимости.
    const GAMES: u32 = 120_000;
    /// Сколько разных зёрен случайных событий в них участвует.
    const SEEDS: u32 = 40;

    /// Разыгрывает много партий и проверяет: каждая заканчивается, каждая
    /// развязка и каждое событие достижимы, все виды анимаций встречаются,
    /// ни один текст не потерян.
    ///
    /// Полный перебор графа больше невозможен: без ранних поражений в нём
    /// сотни миллионов состояний. Поэтому решения выбирает детерминированный
    /// «игрок» со своей склонностью к первому варианту — от осторожного
    /// до безрассудного. Каждый найденный маршрут — настоящая партия, её
    /// можно повторить: `SHADOW_SEED=<зерно> SHADOW_AUTOPLAY="s <маршрут>"`.
    /// Самый короткий маршрут к каждой развязке печатается:
    /// `cargo test every_ending -- --nocapture`.
    #[test]
    fn every_ending_is_reachable() {
        let text = Catalog::embedded_only();
        let mut routes: HashMap<Location, (u32, String)> = HashMap::new();
        let mut events: HashSet<Event> = HashSet::new();
        let mut cutscenes: HashSet<std::mem::Discriminant<Cutscene>> = HashSet::new();
        // Тексты сцены зависят от небольшого набора полей: каждый вариант
        // проверяется один раз, а не в каждой партии.
        let mut checked = HashSet::new();
        let mut dice = 0x5eed_u32;
        let mut route = String::new();

        for game in 0..GAMES {
            let seed = game % SEEDS + 1;
            // Склонность к первому варианту: от 5 до 95 процентов.
            let bias = 5 + (game / SEEDS) % 10 * 10;
            let mut state = GameState::with_seed(seed);
            route.clear();
            loop {
                let variant = (
                    state.location,
                    state.mission,
                    state.rush >= MAX_RUSH,
                    state.war,
                    state.defence_active(),
                    state.soviet_first,
                    state.autocracy,
                    state.final_phase(),
                );
                if checked.insert(variant) {
                    let title = state.title(&text);
                    assert!(!title.contains(MISSING), "нет заголовка {:?}", state.location);
                    assert!(!state.description(&text).contains(MISSING), "нет текста {:?}", state.location);
                    if let Some(epilogue) = state.epilogue(&text) {
                        assert_ne!(epilogue, MISSING, "нет эпилога {:?}", state.location);
                    }
                    if let Some(options) = state.options(&text) {
                        assert!(options.iter().all(|o| o != MISSING), "нет варианта {:?}", state.location);
                    }
                }
                if let Location::Event(event) = state.location {
                    events.insert(event);
                }
                if state.location.is_ending() {
                    let best = routes.entry(state.location).or_insert_with(|| (seed, route.clone()));
                    if route.len() < best.1.len() {
                        *best = (seed, route.clone());
                    }
                    break;
                }
                assert!(route.len() < 300, "партия не заканчивается: {route}");
                dice = dice.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                let (choice, digit) =
                    if (dice >> 16) % 100 < bias { (Choice::First, '1') } else { (Choice::Second, '2') };
                let turn = state.apply(choice, &text);
                assert!(!turn.report.contains(MISSING), "нет текста сводки: {}", turn.report);
                cutscenes.extend(turn.cutscenes.iter().map(std::mem::discriminant));
                route.push(digit);
            }
        }

        for ending in Location::ENDINGS {
            let (seed, route) =
                routes.get(&ending).unwrap_or_else(|| panic!("недостижима развязка {ending:?}"));
            println!("{ending:?} (зерно {seed}): {route}");
        }
        for event in Event::ALL {
            assert!(events.contains(&event), "событие {event:?} никогда не случается");
        }
        assert_eq!(cutscenes.len(), 11, "не все анимации встречаются в игре");
    }

    /// У каждой развязки есть название, текст, послесловие и подсказка для галереи.
    #[test]
    fn endings_have_all_texts() {
        let text = Catalog::embedded_only();
        for ending in Location::ENDINGS {
            for prefix in ["title", "text", "epi", "hint"] {
                let key = format!("{prefix}.{}", ending.key());
                assert_ne!(text.get(&key), MISSING, "нет текста {key}");
            }
            assert_eq!(Location::ending_from_key(ending.key()), Some(ending));
        }
    }

    /// Высадка СССР раньше Apollo 22 больше не обрывает партию.
    #[test]
    fn soviet_landing_first_is_not_game_over() {
        let text = Catalog::embedded_only();
        let mut state = GameState::new();
        state.location = Location::LaunchPad;
        state.budget = 500;
        state.months = 20;
        state.soviet_modules = SOVIET_MODULES - 1;
        state.odd_delay = true;
        let turn = state.apply(Choice::Second, &text);
        assert_eq!(state.location, Location::SovietFirst);
        assert!(turn.cutscenes.contains(&Cutscene::SovietLanding));
        state.apply(Choice::First, &text);
        assert_eq!(state.location, Location::LaunchPad, "после смены цели миссии продолжаются");
    }

    /// Бунт ведёт к кризису власти, а чрезвычайное положение оставляет игрока в игре.
    #[test]
    fn unrest_leads_to_crackdown_not_game_over() {
        let text = Catalog::embedded_only();
        let mut state = GameState::new();
        state.location = Location::UnitedNations;
        state.un_pending = true;
        state.unrest = 95;
        state.apply(Choice::Second, &text);
        assert_eq!(state.location, Location::Uprising);
        let turn = state.apply(Choice::First, &text);
        assert!(state.autocracy);
        assert!(turn.cutscenes.contains(&Cutscene::Crackdown));
        assert_eq!(state.location, Location::LaunchPad);
        assert!(state.unrest <= UNREST_AFTER_CRACKDOWN);
    }

    /// Перехват на трассе начинает войну, но кампания продолжается.
    #[test]
    fn moon_war_is_not_game_over() {
        let text = Catalog::embedded_only();
        let mut state = GameState::new();
        state.location = Location::LaunchPad;
        state.mission = Mission::Apollo21;
        state.budget = 500;
        state.soviet_modules = DEFENCE_MODULE;
        state.tension = 85;
        let turn = state.apply(Choice::First, &text);
        assert_eq!(state.location, Location::MoonWar);
        assert!(turn.cutscenes.contains(&Cutscene::Intercept));
        state.gromov = true;
        state.apply(Choice::Second, &text);
        assert_eq!(state.war, War::Truce);
        assert_eq!(state.location, Location::LaunchPad);
        assert_eq!(state.mission, Mission::Apollo22, "груз сбитого корабля потерян");
    }

    /// Пустая касса — кризис, а не конец: заём Пентагона возвращает на старт.
    #[test]
    fn empty_treasury_offers_war_loan() {
        let text = Catalog::embedded_only();
        let mut state = GameState::new();
        state.location = Location::LaunchPad;
        state.budget = 5;
        state.apply(Choice::First, &text);
        assert_eq!(state.location, Location::Bankrupt);
        state.apply(Choice::First, &text);
        assert_eq!(state.location, Location::LaunchPad);
        assert!(state.budget >= state.mission.cost());
    }

    /// Три отсрочки подряд закрывают окно: вторым вариантом миссию снимают.
    #[test]
    fn launch_window_limits_delays() {
        let text = Catalog::embedded_only();
        let mut state = GameState::new();
        state.location = Location::LaunchPad;
        state.budget = 500;
        for _ in 0..MAX_RUSH {
            state.apply(Choice::Second, &text);
        }
        assert!(state.window_closing());
        state.apply(Choice::Second, &text);
        assert_eq!(state.mission, Mission::Apollo19);
        assert_eq!(state.saturn_v, SATURN_PLAN, "ракета осталась на заводе");
    }

    /// Третья смена даёт США перевес: восемь ракет против пяти Н-1.
    #[test]
    fn third_shift_outnumbers_n1() {
        let text = Catalog::embedded_only();
        let mut state = GameState::new();
        for choice in [Choice::First, Choice::First, Choice::First, Choice::First] {
            state.apply(choice, &text);
        }
        assert_eq!(state.location, Location::LaunchPad);
        assert_eq!(state.saturn_v, SATURN_LIMIT);
        assert!(state.saturn_v > SOVIET_MODULES);
    }
}
