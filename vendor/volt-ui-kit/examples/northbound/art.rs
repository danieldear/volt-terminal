//! Procedural landscape "photographs". Generated once at startup so the example
//! ships no binary assets and makes no network or file requests.
use volt_ui_kit::ui::Image;

type Rgb = [f32; 3];

fn hex(v: u32) -> Rgb {
    [
        ((v >> 16) & 255) as f32 / 255.,
        ((v >> 8) & 255) as f32 / 255.,
        (v & 255) as f32 / 255.,
    ]
}
fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0., 1.);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}
fn add(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [a[0] + b[0] * t, a[1] + b[1] * t, a[2] + b[2] * t]
}
fn scale(a: Rgb, t: f32) -> Rgb {
    [a[0] * t, a[1] * t, a[2] * t]
}
fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}
fn hash(x: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x27d4_eb2d) ^ seed.wrapping_mul(0x9e37_79b9);
    h ^= h >> 15;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    h as f32 / u32::MAX as f32
}
fn hash2(x: u32, y: u32, seed: u32) -> f32 {
    hash(
        (x.wrapping_mul(73_856_093) ^ y.wrapping_mul(19_349_663)) as i32,
        seed,
    )
}
fn noise(x: f32, seed: u32) -> f32 {
    let i = x.floor();
    let f = x - i;
    let f = f * f * (3. - 2. * f);
    let a = hash(i as i32, seed);
    let b = hash(i as i32 + 1, seed);
    a + (b - a) * f
}
/// Ridged fractal noise: sharp, glacier-carved peaks rather than rolling hills.
fn ridge(x: f32, seed: u32, sharp: f32) -> f32 {
    ridge_octaves(x, seed, sharp, 5)
}
fn ridge_octaves(x: f32, seed: u32, sharp: f32, octaves: u32) -> f32 {
    let (mut sum, mut amp, mut freq, mut norm) = (0., 0.5, 1., 0.);
    for octave in 0..octaves {
        let n = noise(x * freq, seed + octave * 31);
        let r = 1. - (2. * n - 1.).abs();
        sum += r.powf(sharp) * amp;
        norm += amp;
        amp *= 0.48;
        freq *= 2.1;
    }
    sum / norm
}

#[derive(Clone)]
pub struct Range {
    /// Ridge base (fraction of height) and peak lift.
    pub base: f32,
    pub lift: f32,
    pub freq: f32,
    pub seed: u32,
    pub color: Rgb,
    pub haze: f32,
    pub snow: f32,
    pub sharp: f32,
}
#[derive(Clone)]
pub struct Scene {
    pub sky: [Rgb; 3],
    pub horizon: f32,
    pub sun: Option<([f32; 2], f32, Rgb)>,
    pub aurora: Option<(Rgb, Rgb, f32)>,
    pub stars: f32,
    pub ranges: Vec<Range>,
    pub water: Option<Rgb>,
    pub cabins: Vec<f32>,
    pub lights: bool,
    pub fog: f32,
    /// Darkening toward the bottom edge so overlaid text stays legible.
    pub scrim: f32,
    pub seed: u32,
}

impl Scene {
    fn sky_at(&self, u: f32, v: f32) -> Rgb {
        let t = v / self.horizon.max(0.01);
        let mut c = if t < 0.55 {
            mix(self.sky[0], self.sky[1], t / 0.55)
        } else {
            mix(self.sky[1], self.sky[2], (t - 0.55) / 0.45)
        };
        if let Some((pos, r, color)) = self.sun {
            let d = ((u - pos[0]) * 1.7).hypot(v - pos[1]);
            c = add(c, color, (1. - smooth(0., r * 6., d)).powf(2.2) * 0.55);
            c = mix(c, hex(0xFFF4DE), 1. - smooth(r * 0.92, r, d));
        }
        if let Some((low, high, strength)) = self.aurora {
            for (band, offset) in [(0_u32, 0.0_f32), (1, 0.11)] {
                let seed = self.seed + band * 7;
                let center = 0.30
                    + offset
                    + 0.07 * (u * 5.1 + seed as f32).sin()
                    + 0.035 * (u * 11.3 + seed as f32 * 0.7).sin()
                    + 0.03 * (noise(u * 6., seed) - 0.5);
                let streak = 0.55
                    + 0.45
                        * (u * 140. + 3. * (u * 9.).sin()).sin().abs().powf(0.6)
                        * noise(u * 40., seed + 3);
                let above = (center - v).max(0.);
                let below = (v - center).max(0.);
                let body = if v < center {
                    (-above / 0.16).exp()
                } else {
                    (-(below / 0.022).powi(2)).exp()
                };
                let color = mix(low, high, smooth(0., 0.22, above));
                c = add(
                    c,
                    color,
                    body * streak * strength * if band == 0 { 1. } else { 0.55 },
                );
            }
        }
        if self.stars > 0. {
            let fade = 1. - smooth(0.2, 0.9, t);
            let px = (u * 1400.) as u32;
            let py = (v * 900.) as u32;
            let h = hash2(px, py, self.seed);
            if h > 1. - 0.0035 * self.stars {
                c = add(c, [1., 1., 1.], fade * hash2(py, px, 9) * 0.9);
            }
        }
        c
    }
    /// Sky, mountains and shore above the waterline.
    fn land_at(&self, u: f32, v: f32) -> Rgb {
        let mut c = self.sky_at(u, v);
        for range in &self.ranges {
            let h = ridge(u * range.freq, range.seed, range.sharp);
            let top = range.base - range.lift * h;
            if v >= top {
                // Shade and snow follow the broad mountain form (two octaves), so
                // detail noise roughens the skyline without striping the faces.
                let x = u * range.freq;
                let form = ridge_octaves(x, range.seed, range.sharp, 2);
                let slope = ridge_octaves(x + 0.03, range.seed, range.sharp, 2)
                    - ridge_octaves(x - 0.03, range.seed, range.sharp, 2);
                let light = smooth(-0.1, 0.1, slope);
                let mut rock = scale(range.color, 0.8 + 0.4 * light);
                let below_peak = (v - top) / range.lift.max(0.01);
                if range.snow > 0. {
                    let line = range.snow * 0.45 * (0.6 + 0.4 * noise(x * 9., range.seed + 5));
                    let cover =
                        (1. - smooth(line * 0.7, line, below_peak)) * smooth(0.2, 0.55, form);
                    rock = mix(rock, scale(hex(0xF2F6FA), 0.68 + 0.3 * light), cover);
                }
                c = mix(rock, self.sky[2], range.haze);
                // Atmospheric falloff toward the waterline, banded by height only.
                let foot = smooth(range.base - range.lift * 0.35, range.base, v);
                c = mix(c, self.sky[2], self.fog * foot * 0.6);
            }
        }
        let shore = self.horizon;
        for (i, &x) in self.cabins.iter().enumerate() {
            let w = 0.016 + 0.005 * hash(i as i32, 4);
            let hgt = 0.026 + 0.008 * hash(i as i32, 5);
            if (u - x).abs() < w && v > shore - hgt && v < shore {
                let roof = v < shore - hgt * 0.62;
                c = if roof {
                    hex(0x3B2522)
                } else if ((u - x) / w * 3.).fract().abs() < 0.14 {
                    hex(0xE8D9C6)
                } else {
                    hex(0xA8322B)
                };
            }
        }
        if self.lights && v > shore - 0.05 && v < shore {
            let px = (u * 600.) as u32;
            let py = (v * 400.) as u32;
            if hash2(px, py, self.seed + 11) > 0.965 {
                c = mix(c, hex(0xFFD08A), 0.9);
            }
        }
        c
    }
    fn color(&self, u: f32, v: f32) -> Rgb {
        let mut c = match self.water {
            Some(water) if v > self.horizon => {
                let depth = (v - self.horizon) / (1. - self.horizon);
                let ripple = 0.004 * (v * 520.).sin() + 0.002 * (u * 90. + v * 300.).sin();
                let mirrored = self.land_at(u + ripple, 2. * self.horizon - v);
                let reflect = mix(scale(mirrored, 0.78), water, 0.25 + depth * 0.6);
                let glint = if (v * 900.).sin() > 0.93 { 0.05 } else { 0. };
                add(reflect, [1., 1., 1.], glint * (1. - depth))
            }
            _ => self.land_at(u, v),
        };
        if self.fog > 0. {
            let band = (-((v - self.horizon) / 0.05).powi(2)).exp();
            c = mix(c, self.sky[2], band * self.fog);
        }
        let vignette = 1. - 0.28 * ((u - 0.5).powi(2) * 2. + (v - 0.45).powi(2) * 1.6);
        c = scale(c, vignette);
        if self.scrim > 0. {
            c = scale(c, 1. - self.scrim * smooth(0.45, 1., v));
        }
        c
    }
    pub fn render(&self, width: u32, height: u32) -> Image {
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                let u = (x as f32 + 0.5) / width as f32;
                let v = (y as f32 + 0.5) / height as f32;
                let c = self.color(u, v);
                let grain = (hash2(x, y, self.seed ^ 0xA5A5) - 0.5) * 0.035;
                for channel in c {
                    // Gamma-friendly quantization plus film grain to avoid banding.
                    pixels.push(((channel + grain).clamp(0., 1.) * 255. + 0.5) as u8);
                }
                pixels.push(255);
            }
        }
        Image::from_rgba(width, height, pixels).expect("bounded generated photograph")
    }
}

fn range(base: f32, lift: f32, freq: f32, seed: u32, color: u32, haze: f32, snow: f32) -> Range {
    Range {
        base,
        lift,
        freq,
        seed,
        color: hex(color),
        haze,
        snow,
        sharp: 2.2,
    }
}

/// One scene per stop on the route; indices match `Place::ALL`.
pub fn scenes() -> Vec<Scene> {
    vec![
        // Reine at dusk: rose sky, granite spires, red rorbu cabins on the water.
        Scene {
            sky: [hex(0x2B3A63), hex(0xC98A9C), hex(0xF6C59A)],
            horizon: 0.66,
            sun: Some(([0.78, 0.6], 0.025, hex(0xFFB26B))),
            aurora: None,
            stars: 0.,
            ranges: vec![
                range(0.64, 0.42, 3.2, 11, 0x5A5E7C, 0.45, 0.0),
                Range {
                    sharp: 3.0,
                    ..range(0.66, 0.52, 2.4, 3, 0x232B3E, 0.0, 0.28)
                },
            ],
            water: Some(hex(0x24324A)),
            cabins: vec![0.18, 0.215, 0.25, 0.62, 0.655],
            lights: false,
            fog: 0.12,
            scrim: 0.55,
            seed: 1,
        },
        // Senja under a green-violet aurora.
        Scene {
            sky: [hex(0x050B1A), hex(0x0D2236), hex(0x18394A)],
            horizon: 0.7,
            sun: None,
            aurora: Some((hex(0x5CF2B0), hex(0xA86CF0), 0.85)),
            stars: 1.,
            ranges: vec![
                range(0.69, 0.3, 2.6, 21, 0x14263A, 0.2, 0.0),
                Range {
                    sharp: 2.6,
                    ..range(0.7, 0.38, 1.8, 8, 0x0A1522, 0.0, 0.35)
                },
            ],
            water: Some(hex(0x06121E)),
            cabins: vec![],
            lights: false,
            fog: 0.0,
            scrim: 0.45,
            seed: 2,
        },
        // Tromsø at blue hour with harbour lights.
        Scene {
            sky: [hex(0x101E45), hex(0x3A5A97), hex(0x9FB6DA)],
            horizon: 0.68,
            sun: None,
            aurora: Some((hex(0x7DF5C8), hex(0x7DB6F5), 0.25)),
            stars: 0.4,
            ranges: vec![
                Range {
                    sharp: 1.4,
                    ..range(0.67, 0.22, 1.6, 31, 0x4F6595, 0.35, 0.6)
                },
                Range {
                    sharp: 1.6,
                    ..range(0.68, 0.14, 2.2, 13, 0x22304F, 0.0, 0.0)
                },
            ],
            water: Some(hex(0x1A2A4D)),
            cabins: vec![],
            lights: true,
            fog: 0.08,
            scrim: 0.5,
            seed: 3,
        },
        // North Cape: the midnight sun grazing the Barents Sea.
        Scene {
            sky: [hex(0x3B4C77), hex(0xE59A6B), hex(0xFFD49A)],
            horizon: 0.62,
            sun: Some(([0.6, 0.56], 0.04, hex(0xFFC27A))),
            aurora: None,
            stars: 0.,
            ranges: vec![Range {
                sharp: 0.8,
                ..range(0.62, 0.3, 0.55, 41, 0x2D2630, 0.0, 0.0)
            }],
            water: Some(hex(0x3B3852)),
            cabins: vec![],
            lights: false,
            fog: 0.18,
            scrim: 0.5,
            seed: 4,
        },
        // Hamnøy in winter: pale pastel light, heavy snow.
        Scene {
            sky: [hex(0x7D9CC8), hex(0xD8C6DA), hex(0xF4E0D6)],
            horizon: 0.67,
            sun: None,
            aurora: None,
            stars: 0.,
            ranges: vec![
                range(0.62, 0.36, 2.8, 51, 0x8E9BB8, 0.5, 0.9),
                Range {
                    sharp: 2.8,
                    ..range(0.67, 0.5, 2.0, 17, 0x4A5269, 0.0, 0.7)
                },
            ],
            water: Some(hex(0x5C6E8E)),
            cabins: vec![0.32, 0.355, 0.39, 0.425],
            lights: false,
            fog: 0.2,
            scrim: 0.45,
            seed: 5,
        },
        // Vesterålen: sea mist rolling between low islands at dawn.
        Scene {
            sky: [hex(0x5A7F8F), hex(0xA9C3C2), hex(0xE3E8DC)],
            horizon: 0.64,
            sun: Some(([0.25, 0.5], 0.03, hex(0xFFE7C2))),
            aurora: None,
            stars: 0.,
            ranges: vec![
                Range {
                    sharp: 1.2,
                    ..range(0.6, 0.18, 1.4, 61, 0x7E9A98, 0.55, 0.0)
                },
                Range {
                    sharp: 1.5,
                    ..range(0.64, 0.2, 2.3, 23, 0x3E5853, 0.1, 0.0)
                },
            ],
            water: Some(hex(0x58777A)),
            cabins: vec![0.71],
            lights: false,
            fog: 0.55,
            scrim: 0.45,
            seed: 6,
        },
    ]
}
