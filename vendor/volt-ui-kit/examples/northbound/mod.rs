//! Northbound: a trip planner for two weeks along Norway's Arctic coast.
//! All places, prices and journal entries are fictional sample data.
pub mod art;
pub mod icons;
mod kit;
mod screens;
#[cfg(test)]
mod tests;

pub use screens::view;
use volt_ui_kit::ui::{Image, Palette, ThemeMode, UiTheme};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Route,
    Days,
    Journal,
    Settings,
}
impl Screen {
    pub const ALL: [Self; 4] = [Self::Route, Self::Days, Self::Journal, Self::Settings];
    pub fn title(self) -> &'static str {
        match self {
            Self::Route => "Route",
            Self::Days => "Days",
            Self::Journal => "Journal",
            Self::Settings => "Settings",
        }
    }
    pub fn slug(self) -> &'static str {
        match self {
            Self::Route => "route",
            Self::Days => "days",
            Self::Journal => "journal",
            Self::Settings => "settings",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.slug() == value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Peaks,
    Coast,
    Night,
}

pub struct Place {
    pub name: &'static str,
    pub region: &'static str,
    pub blurb: &'static str,
    pub day: u8,
    pub km: u16,
    pub temp: i8,
    pub sky: &'static str,
    pub kinds: &'static [Kind],
    /// Normalized position on the stylized route map.
    pub map: [f32; 2],
}
pub const PLACES: [Place; 6] = [
    Place {
        name: "Reine",
        region: "Lofoten",
        blurb: "Granite spires drop straight into the sea. Stay in a red rorbu cabin on stilts and wake to fishing boats heading out.",
        day: 2,
        km: 0,
        temp: 4,
        sky: "Clear at dusk",
        kinds: &[Kind::Peaks, Kind::Coast],
        map: [0.12, 0.78],
    },
    Place {
        name: "Hamnøy",
        region: "Lofoten",
        blurb: "A cluster of cabins on a skerry, with the best morning light in the archipelago. Bring crampons for the ridge walk.",
        day: 4,
        km: 12,
        temp: -1,
        sky: "Light snow",
        kinds: &[Kind::Peaks, Kind::Coast],
        map: [0.2, 0.7],
    },
    Place {
        name: "Vesterålen",
        region: "Andøya",
        blurb: "Low islands and sea mist. Whale safaris leave Andenes at dawn; the white-sand beaches are empty by noon.",
        day: 6,
        km: 210,
        temp: 6,
        sky: "Sea mist",
        kinds: &[Kind::Coast],
        map: [0.36, 0.5],
    },
    Place {
        name: "Senja",
        region: "Troms",
        blurb: "Norway's second-largest island has more aurora than visitors. Park at Ersfjord and look north after ten.",
        day: 8,
        km: 340,
        temp: -4,
        sky: "Aurora likely",
        kinds: &[Kind::Night, Kind::Peaks],
        map: [0.52, 0.36],
    },
    Place {
        name: "Tromsø",
        region: "Troms",
        blurb: "The Arctic capital: a cable car to Fløya, a cathedral of ice-white triangles, and harbour lights at blue hour.",
        day: 10,
        km: 450,
        temp: -2,
        sky: "Blue hour",
        kinds: &[Kind::Night, Kind::Coast],
        map: [0.66, 0.3],
    },
    Place {
        name: "North Cape",
        region: "Finnmark",
        blurb: "A 307-metre cliff at the edge of Europe. In June the sun circles the horizon and never sets.",
        day: 14,
        km: 890,
        temp: 3,
        sky: "Midnight sun",
        kinds: &[Kind::Coast],
        map: [0.9, 0.12],
    },
];
/// Art scenes are generated in a different order from the route.
pub const SCENE_FOR_PLACE: [usize; 6] = [0, 4, 5, 1, 2, 3];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    All,
    Peaks,
    Coast,
    Night,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leg {
    Lofoten,
    Vesteralen,
    Troms,
    Finnmark,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transport {
    Ferry,
    Car,
    Bus,
    Bike,
}
impl Transport {
    pub const ALL: [(Self, &'static str); 4] = [
        (Self::Ferry, "Coastal ferry"),
        (Self::Car, "Rental car"),
        (Self::Bus, "Bus and ferry"),
        (Self::Bike, "Bicycle"),
    ];
    pub fn per_day(self) -> u32 {
        match self {
            Self::Ferry => 1450,
            Self::Car => 980,
            Self::Bus => 520,
            Self::Bike => 90,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mood {
    Calm,
    Awed,
    Windswept,
    Cozy,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Units {
    Metric,
    Imperial,
}

pub struct Entry {
    pub title: String,
    pub body: String,
    pub mood: Mood,
    pub photo: usize,
}

pub const ACCENTS: [(&str, u32); 4] = [
    ("Aurora", 0x86E3C3),
    ("Ember", 0xF3A66F),
    ("Glacier", 0x93BDF4),
    ("Heather", 0xC9A6EA),
];
pub const PACKING: [&str; 7] = [
    "Wool base layers",
    "Microspikes",
    "Headlamp",
    "Ferry tickets",
    "Tripod for aurora",
    "Power bank",
    "Thermos",
];

pub struct Toast {
    pub id: u32,
    pub message: String,
}

pub struct App {
    pub screen: Screen,
    pub mode: ThemeMode,
    pub accent: usize,
    pub photos: Vec<Image>,
    pub thumbs: Vec<Image>,
    pub place: usize,
    pub filter: Filter,
    pub query: String,
    pub saved: [bool; 6],
    pub leg: Leg,
    pub done_days: [bool; 14],
    pub packing: [bool; 7],
    pub budget: f32,
    pub transport: Transport,
    pub transport_open: bool,
    pub draft_title: String,
    pub draft_body: String,
    pub mood: Mood,
    pub draft_photo: usize,
    pub entries: Vec<Entry>,
    pub reserve_open: bool,
    pub guest: String,
    pub nights: u8,
    pub nights_open: bool,
    pub breakfast: bool,
    pub late_arrival: bool,
    pub booked: Vec<usize>,
    pub offline_maps: bool,
    pub aurora_alerts: bool,
    pub ferry_reminders: bool,
    pub kp: f32,
    pub units: Units,
    pub units_open: bool,
    pub reset_open: bool,
    pub toast: Option<Toast>,
    next_toast: u32,
}

impl App {
    pub fn new() -> Self {
        let scenes = art::scenes();
        let photos = scenes.iter().map(|s| s.render(960, 560)).collect();
        let thumbs = scenes
            .iter()
            .map(|s| {
                let mut s = s.clone();
                s.scrim = 0.;
                s.render(420, 300)
            })
            .collect();
        Self::sample(photos, thumbs)
    }
    /// Sample itinerary state around already-rendered photographs.
    fn sample(photos: Vec<Image>, thumbs: Vec<Image>) -> Self {
        Self {
            screen: Screen::Route,
            mode: ThemeMode::Dark,
            accent: 0,
            photos,
            thumbs,
            place: 3,
            filter: Filter::All,
            query: String::new(),
            saved: [false, true, false, false, false, false],
            leg: Leg::Lofoten,
            done_days: [true, true, true, false, false, false, false, false, false, false, false, false, false, false],
            packing: [true, true, false, true, false, false, true],
            budget: 2400.,
            transport: Transport::Ferry,
            transport_open: false,
            draft_title: String::new(),
            draft_body: String::new(),
            mood: Mood::Awed,
            draft_photo: 3,
            entries: vec![
                Entry {
                    title: "Ersfjord, ten past eleven".into(),
                    body: "The sky went green so slowly I thought it was cloud. Then it moved. Nobody in the car park said a word for twenty minutes.".into(),
                    mood: Mood::Awed,
                    photo: 3,
                },
                Entry {
                    title: "Cabin 14, Hamnøy".into(),
                    body: "Snow all afternoon. Fish soup, a borrowed paperback and the stove ticking. We skipped the ridge and did not regret it.".into(),
                    mood: Mood::Cozy,
                    photo: 1,
                },
            ],
            reserve_open: false,
            guest: String::new(),
            nights: 2,
            nights_open: false,
            breakfast: true,
            late_arrival: false,
            booked: vec![],
            offline_maps: true,
            aurora_alerts: true,
            ferry_reminders: false,
            kp: 4.,
            units: Units::Metric,
            units_open: false,
            reset_open: false,
            toast: None,
            next_toast: 0,
        }
    }
    pub fn notify(&mut self, message: impl Into<String>) {
        self.next_toast += 1;
        self.toast = Some(Toast {
            id: self.next_toast,
            message: message.into(),
        });
    }
    pub fn photo(&self, place: usize) -> &Image {
        &self.photos[SCENE_FOR_PLACE[place]]
    }
    pub fn thumb(&self, place: usize) -> &Image {
        &self.thumbs[SCENE_FOR_PLACE[place]]
    }
    pub fn visible_places(&self) -> Vec<usize> {
        let query = self.query.trim().to_lowercase();
        (0..PLACES.len())
            .filter(|&i| {
                let p = &PLACES[i];
                let kind = match self.filter {
                    Filter::All => true,
                    Filter::Peaks => p.kinds.contains(&Kind::Peaks),
                    Filter::Coast => p.kinds.contains(&Kind::Coast),
                    Filter::Night => p.kinds.contains(&Kind::Night),
                };
                kind && (query.is_empty()
                    || p.name.to_lowercase().contains(&query)
                    || p.region.to_lowercase().contains(&query))
            })
            .collect()
    }
    pub fn stay_price(&self) -> u32 {
        let base = 1890 + PLACES[self.place].km as u32 / 3;
        let breakfast = if self.breakfast { 240 } else { 0 };
        (base + breakfast) * self.nights as u32
    }
    pub fn trip_total(&self) -> u32 {
        (self.budget as u32 + self.transport.per_day()) * 14
    }
    pub fn distance(&self, km: u16) -> String {
        match self.units {
            Units::Metric => format!("{km} km"),
            Units::Imperial => format!("{} mi", (km as f32 * 0.621).round() as u32),
        }
    }
    pub fn temperature(&self, celsius: i8) -> String {
        match self.units {
            Units::Metric => format!("{celsius}°C"),
            Units::Imperial => format!("{}°F", (celsius as f32 * 1.8 + 32.).round() as i32),
        }
    }
    pub fn reset(&mut self) {
        let photos = std::mem::take(&mut self.photos);
        let thumbs = std::mem::take(&mut self.thumbs);
        let (mode, accent, units) = (self.mode, self.accent, self.units);
        // Rebuild the sample data without regenerating the photographs.
        *self = Self::sample(photos, thumbs);
        (self.mode, self.accent, self.units) = (mode, accent, units);
        self.screen = Screen::Settings;
        self.notify("Trip reset to the sample itinerary");
    }
}

fn rgba(v: u32) -> [f32; 4] {
    [
        ((v >> 16) & 255) as f32 / 255.,
        ((v >> 8) & 255) as f32 / 255.,
        (v & 255) as f32 / 255.,
        1.,
    ]
}

/// "Polar night" and "Midnight sun" palettes, with a selectable accent.
pub fn theme(app: &App, system: ThemeMode) -> UiTheme {
    let light = match app.mode {
        ThemeMode::System => system == ThemeMode::Light,
        mode => mode == ThemeMode::Light,
    };
    let mut palette = if light {
        Palette::light()
    } else {
        Palette::dark()
    };
    palette.name = if light { "Midnight sun" } else { "Polar night" }.into();
    let accent = ACCENTS[app.accent].1;
    if light {
        palette.base = [rgba(0xEDF1EE), rgba(0xFFFFFF), rgba(0xDCE5E1)];
        palette.content = rgba(0x13242E);
        palette.muted = rgba(0x5A6F78);
        palette.border = rgba(0xC6D3CE);
        palette.neutral = rgba(0x13242E);
        palette.on_neutral = rgba(0xEDF1EE);
        palette.secondary = rgba(0xC8692F);
        palette.on_secondary = rgba(0xFFFFFF);
        palette.success = rgba(0x1E7B66);
        palette.on_success = rgba(0xFFFFFF);
    } else {
        palette.base = [rgba(0x0F1D27), rgba(0x162A37), rgba(0x213A4A)];
        palette.content = rgba(0xECF2F1);
        palette.muted = rgba(0x8EA6AE);
        palette.border = rgba(0x2C4757);
        palette.neutral = rgba(0xECF2F1);
        palette.on_neutral = rgba(0x0F1D27);
        palette.secondary = rgba(0xF3A66F);
        palette.on_secondary = rgba(0x2A1606);
        palette.success = rgba(0x86E3C3);
        palette.on_success = rgba(0x0B2A20);
    }
    // Light mode deepens the accent so it holds contrast on pale fog.
    let mut color = rgba(accent);
    if light {
        for channel in &mut color[..3] {
            *channel *= 0.62;
        }
    }
    UiTheme::from_palette(palette.with_accent(color))
}
