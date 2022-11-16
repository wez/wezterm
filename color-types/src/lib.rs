use csscolorparser::Color;
#[cfg(feature = "use_serde")]
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use wezterm_dynamic::{FromDynamic, FromDynamicOptions, ToDynamic, Value};

lazy_static::lazy_static! {
    static ref SRGB_TO_F32_TABLE: [f32;256] = generate_srgb8_to_linear_f32_table();
    static ref F32_TO_U8_TABLE: [u32;104] = generate_linear_f32_to_srgb8_table();
    static ref RGB_TO_SRGB_TABLE: [u8;256] = generate_rgb_to_srgb8_table();
    static ref RGB_TO_F32_TABLE: [f32;256] = generate_rgb_to_linear_f32_table();
}

fn generate_rgb_to_srgb8_table() -> [u8; 256] {
    let mut table = [0; 256];
    for (val, entry) in table.iter_mut().enumerate() {
        let linear = (val as f32) / 255.0;
        *entry = linear_f32_to_srgb8_using_table(linear);
    }
    table
}

fn generate_rgb_to_linear_f32_table() -> [f32; 256] {
    let mut table = [0.; 256];
    for (val, entry) in table.iter_mut().enumerate() {
        *entry = (val as f32) / 255.0;
    }
    table
}

fn generate_srgb8_to_linear_f32_table() -> [f32; 256] {
    let mut table = [0.; 256];
    for (val, entry) in table.iter_mut().enumerate() {
        let c = (val as f32) / 255.0;
        *entry = if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        };
    }
    table
}

#[allow(clippy::unreadable_literal)]
fn generate_linear_f32_to_srgb8_table() -> [u32; 104] {
    // My intent was to generate this array on the fly using the code that is commented
    // out below.  It is based on this gist:
    // https://gist.github.com/rygorous/2203834
    // but for whatever reason, the rust translation yields different numbers.
    // I haven't had an opportunity to dig in to why that is, and I just wanted
    // to get things rolling, so we're in a slightly gross state for now.
    [
        0x0073000d, 0x007a000d, 0x0080000d, 0x0087000d, 0x008d000d, 0x0094000d, 0x009a000d,
        0x00a1000d, 0x00a7001a, 0x00b4001a, 0x00c1001a, 0x00ce001a, 0x00da001a, 0x00e7001a,
        0x00f4001a, 0x0101001a, 0x010e0033, 0x01280033, 0x01410033, 0x015b0033, 0x01750033,
        0x018f0033, 0x01a80033, 0x01c20033, 0x01dc0067, 0x020f0067, 0x02430067, 0x02760067,
        0x02aa0067, 0x02dd0067, 0x03110067, 0x03440067, 0x037800ce, 0x03df00ce, 0x044600ce,
        0x04ad00ce, 0x051400ce, 0x057b00c5, 0x05dd00bc, 0x063b00b5, 0x06970158, 0x07420142,
        0x07e30130, 0x087b0120, 0x090b0112, 0x09940106, 0x0a1700fc, 0x0a9500f2, 0x0b0f01cb,
        0x0bf401ae, 0x0ccb0195, 0x0d950180, 0x0e56016e, 0x0f0d015e, 0x0fbc0150, 0x10630143,
        0x11070264, 0x1238023e, 0x1357021d, 0x14660201, 0x156601e9, 0x165a01d3, 0x174401c0,
        0x182401af, 0x18fe0331, 0x1a9602fe, 0x1c1502d2, 0x1d7e02ad, 0x1ed4028d, 0x201a0270,
        0x21520256, 0x227d0240, 0x239f0443, 0x25c003fe, 0x27bf03c4, 0x29a10392, 0x2b6a0367,
        0x2d1d0341, 0x2ebe031f, 0x304d0300, 0x31d105b0, 0x34a80555, 0x37520507, 0x39d504c5,
        0x3c37048b, 0x3e7c0458, 0x40a8042a, 0x42bd0401, 0x44c20798, 0x488e071e, 0x4c1c06b6,
        0x4f76065d, 0x52a50610, 0x55ac05cc, 0x5892058f, 0x5b590559, 0x5e0c0a23, 0x631c0980,
        0x67db08f6, 0x6c55087f, 0x70940818, 0x74a007bd, 0x787d076c, 0x7c330723,
    ]
    /*
    let numexp = 13;
    let mantissa_msb = 3;
    let nbuckets = numexp << mantissa_msb;
    let bucketsize = 1 << (23 - mantissa_msb);
    let mantshift = 12;

    let mut table = [0;104];

    let sum_aa = bucketsize as f64;
    let mut sum_ab = 0.0f64;
    let mut sum_bb = 0.0f64;

    for i in 0..bucketsize {
        let j = (i >> mantshift) as f64;

        sum_ab += j;
        sum_bb += j * j;
    }

    let inv_det = 1.0 / (sum_aa * sum_bb - sum_ab * sum_ab);
    eprintln!("sum_ab={:e} sum_bb={:e} inv_det={:e}", sum_ab, sum_bb, inv_det);

    for bucket in 0..nbuckets {
        let start = ((127 - numexp) << 23) + bucket*bucketsize;

        let mut sum_a = 0.0;
        let mut sum_b = 0.0;

        for i in 0..bucketsize {
            let j = i >> mantshift;

            let val = linear_f32_to_srgbf32(f32::from_bits(start + i)) as f64 + 0.5;
            sum_a += val;
            sum_b += j as f64 * val;
        }

        let solved_a = inv_det * (sum_bb*sum_a - sum_ab*sum_b);
        let solved_b = inv_det * (sum_aa*sum_b - sum_ab*sum_a);
        let scaled_a = solved_a * 65536.0 / 512.0;
        let scaled_b = solved_b * 65536.0;

        let int_a = (scaled_a + 0.5) as u32;
        let int_b = (scaled_b + 0.5) as u32;

        table[bucket as usize] = (int_a << 16) + int_b;
    }

    table
    */
}

/// Convert from linear rgb in floating point form (0-1.0) to srgb in floating point (0-255.0)
fn linear_f32_to_srgbf32(f: f32) -> f32 {
    if f <= 0.04045 {
        f * 12.92
    } else {
        f.powf(1.0 / 2.4) * 1.055 - 0.055
    }
}

pub fn linear_u8_to_srgb8(f: u8) -> u8 {
    unsafe { *RGB_TO_SRGB_TABLE.get_unchecked(f as usize) }
}

#[allow(clippy::unreadable_literal)]
const ALMOST_ONE: u32 = 0x3f7fffff;
#[allow(clippy::unreadable_literal)]
const MINVAL: u32 = (127 - 13) << 23;

fn linear_f32_to_srgb8_using_table(f: f32) -> u8 {
    let minval = f32::from_bits(MINVAL);
    let almost_one = f32::from_bits(ALMOST_ONE);

    let f = if f < minval {
        minval
    } else if f > almost_one {
        almost_one
    } else {
        f
    };

    let f_bits = f.to_bits();
    let tab = unsafe { *F32_TO_U8_TABLE.get_unchecked(((f_bits - MINVAL) >> 20) as usize) };
    let bias = (tab >> 16) << 9;
    let scale = tab & 0xffff;

    let t = (f_bits >> 12) & 0xff;

    ((bias + scale * t) >> 16) as u8
}

/// Convert from srgb in u8 0-255 to linear floating point rgb 0-1.0
fn srgb8_to_linear_f32(val: u8) -> f32 {
    unsafe { *SRGB_TO_F32_TABLE.get_unchecked(val as usize) }
}

fn rgb_to_linear_f32(val: u8) -> f32 {
    unsafe { *RGB_TO_F32_TABLE.get_unchecked(val as usize) }
}

/// A pixel holding SRGBA32 data in big endian format
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SrgbaPixel(u32);

impl SrgbaPixel {
    /// Create a pixel with the provided sRGBA values in u8 format
    pub fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        #[allow(clippy::cast_lossless)]
        let word = (blue as u32) << 24 | (green as u32) << 16 | (red as u32) << 8 | alpha as u32;
        Self(word.to_be())
    }

    /// Returns the unpacked sRGBA components as u8
    #[inline]
    pub fn as_rgba(self) -> (u8, u8, u8, u8) {
        let host = u32::from_be(self.0);
        (
            (host >> 8) as u8,
            (host >> 16) as u8,
            (host >> 24) as u8,
            (host & 0xff) as u8,
        )
    }

    /// Returns RGBA channels in linear f32 format
    pub fn to_linear(self) -> LinearRgba {
        let (r, g, b, a) = self.as_rgba();
        LinearRgba::with_srgba(r, g, b, a)
    }

    /// Create a pixel with the provided big-endian u32 SRGBA data
    pub fn with_srgba_u32(word: u32) -> Self {
        Self(word)
    }

    /// Returns the underlying big-endian u32 SRGBA data
    pub fn as_srgba32(self) -> u32 {
        self.0
    }
}

/// A pixel value encoded as SRGBA RGBA values in f32 format (range: 0.0-1.0)
#[derive(Copy, Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
pub struct SrgbaTuple(pub f32, pub f32, pub f32, pub f32);

impl ToDynamic for SrgbaTuple {
    fn to_dynamic(&self) -> Value {
        self.to_string().to_dynamic()
    }
}

impl FromDynamic for SrgbaTuple {
    fn from_dynamic(
        value: &Value,
        options: FromDynamicOptions,
    ) -> Result<Self, wezterm_dynamic::Error> {
        let s = String::from_dynamic(value, options)?;
        Ok(SrgbaTuple::from_str(&s).map_err(|()| format!("unknown color name: {}", s))?)
    }
}

impl From<(f32, f32, f32, f32)> for SrgbaTuple {
    fn from((r, g, b, a): (f32, f32, f32, f32)) -> SrgbaTuple {
        SrgbaTuple(r, g, b, a)
    }
}

impl From<(u8, u8, u8, u8)> for SrgbaTuple {
    fn from((r, g, b, a): (u8, u8, u8, u8)) -> SrgbaTuple {
        SrgbaTuple(
            r as f32 / 255.,
            g as f32 / 255.,
            b as f32 / 255.,
            a as f32 / 255.,
        )
    }
}

impl From<(u8, u8, u8)> for SrgbaTuple {
    fn from((r, g, b): (u8, u8, u8)) -> SrgbaTuple {
        SrgbaTuple(r as f32 / 255., g as f32 / 255., b as f32 / 255., 1.0)
    }
}

impl From<SrgbaTuple> for (f32, f32, f32, f32) {
    fn from(t: SrgbaTuple) -> (f32, f32, f32, f32) {
        (t.0, t.1, t.2, t.3)
    }
}

impl From<Color> for SrgbaTuple {
    fn from(color: Color) -> Self {
        Self(
            color.r as f32,
            color.g as f32,
            color.b as f32,
            color.a as f32,
        )
    }
}

lazy_static::lazy_static! {
    static ref NAMED_COLORS: HashMap<String, SrgbaTuple> = build_colors();
}

fn build_colors() -> HashMap<String, SrgbaTuple> {
    let mut map = HashMap::new();
    let rgb_txt = include_str!("rgb.txt");

    map.insert("transparent".to_string(), SrgbaTuple(0., 0., 0., 0.));
    map.insert("none".to_string(), SrgbaTuple(0., 0., 0., 0.));
    map.insert("clear".to_string(), SrgbaTuple(0., 0., 0., 0.));

    for line in rgb_txt.lines() {
        let mut fields = line.split_ascii_whitespace();
        let red = fields.next().unwrap();
        let green = fields.next().unwrap();
        let blue = fields.next().unwrap();
        let name = fields.collect::<Vec<&str>>().join(" ");

        let name = name.to_ascii_lowercase();
        map.insert(
            name,
            SrgbaTuple(
                red.parse::<f32>().unwrap() / 255.,
                green.parse::<f32>().unwrap() / 255.,
                blue.parse::<f32>().unwrap() / 255.,
                1.0,
            ),
        );
    }

    map
}

impl SrgbaTuple {
    /// Construct a color from an X11/SVG/CSS3 color name.
    /// Returns None if the supplied name is not recognized.
    /// The list of names can be found here:
    /// <https://en.wikipedia.org/wiki/X11_color_names>
    pub fn from_named(name: &str) -> Option<Self> {
        NAMED_COLORS.get(&name.to_ascii_lowercase()).cloned()
    }

    /// Returns self multiplied by the supplied alpha value.
    /// We don't need to linearize for this, as alpha is defined
    /// as being linear even in srgba!
    pub fn mul_alpha(self, alpha: f32) -> Self {
        Self(self.0, self.1, self.2, self.3 * alpha)
    }

    pub fn to_linear(self) -> LinearRgba {
        // See https://docs.rs/palette/0.5.0/src/palette/encoding/srgb.rs.html#43
        fn to_linear(v: f32) -> f32 {
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        }
        // Note that alpha is always linear
        LinearRgba(
            to_linear(self.0),
            to_linear(self.1),
            to_linear(self.2),
            self.3,
        )
    }

    pub fn to_srgb_u8(self) -> (u8, u8, u8, u8) {
        (
            (self.0 * 255.) as u8,
            (self.1 * 255.) as u8,
            (self.2 * 255.) as u8,
            (self.3 * 255.) as u8,
        )
    }

    pub fn to_string(self) -> String {
        if self.3 == 1.0 {
            self.to_rgb_string()
        } else {
            self.to_rgba_string()
        }
    }

    /// Returns a string of the form `#RRGGBB`
    pub fn to_rgb_string(self) -> String {
        format!(
            "#{:02x}{:02x}{:02x}",
            (self.0 * 255.) as u8,
            (self.1 * 255.) as u8,
            (self.2 * 255.) as u8
        )
    }

    pub fn to_rgba_string(self) -> String {
        format!(
            "rgba({}% {}% {}% {}%)",
            (self.0 * 100.),
            (self.1 * 100.),
            (self.2 * 100.),
            (self.3 * 100.)
        )
    }

    /// Returns a string of the form `rgb:RRRR/GGGG/BBBB`
    pub fn to_x11_16bit_rgb_string(self) -> String {
        format!(
            "rgb:{:04x}/{:04x}/{:04x}",
            (self.0 * 65535.) as u16,
            (self.1 * 65535.) as u16,
            (self.2 * 65535.) as u16
        )
    }

    pub fn to_laba(self) -> (f64, f64, f64, f64) {
        Color::new(self.0.into(), self.1.into(), self.2.into(), self.3.into()).to_lab()
    }

    pub fn to_hsla(self) -> (f64, f64, f64, f64) {
        Color::new(self.0.into(), self.1.into(), self.2.into(), self.3.into()).to_hsla()
    }

    pub fn from_hsla(h: f64, s: f64, l: f64, a: f64) -> Self {
        let Color { r, g, b, a } = Color::from_hsla(h, s, l, a);
        Self(r as f32, g as f32, b as f32, a as f32)
    }

    /// Scale the color towards the maximum saturation by factor, a value ranging from 0.0 to 1.0.
    pub fn saturate(&self, factor: f64) -> Self {
        let (h, s, l, a) = self.to_hsla();
        let s = apply_scale(s, factor);
        Self::from_hsla(h, s, l, a)
    }

    /// Increase the saturation by amount, a value ranging from 0.0 to 1.0.
    pub fn saturate_fixed(&self, amount: f64) -> Self {
        let (h, s, l, a) = self.to_hsla();
        let s = apply_fixed(s, amount);
        Self::from_hsla(h, s, l, a)
    }

    /// Scale the color towards the maximum lightness by factor, a value ranging from 0.0 to 1.0
    pub fn lighten(&self, factor: f64) -> Self {
        let (h, s, l, a) = self.to_hsla();
        let l = apply_scale(l, factor);
        Self::from_hsla(h, s, l, a)
    }

    /// Lighten the color by amount, a value ranging from 0.0 to 1.0
    pub fn lighten_fixed(&self, amount: f64) -> Self {
        let (h, s, l, a) = self.to_hsla();
        let l = apply_fixed(l, amount);
        Self::from_hsla(h, s, l, a)
    }

    /// Rotate the hue angle by the specified number of degrees
    pub fn adjust_hue_fixed(&self, amount: f64) -> Self {
        let (h, s, l, a) = self.to_hsla();
        let h = normalize_angle(h + amount);
        Self::from_hsla(h, s, l, a)
    }

    pub fn complement(&self) -> Self {
        self.adjust_hue_fixed(180.)
    }

    pub fn complement_ryb(&self) -> Self {
        self.adjust_hue_fixed_ryb(180.)
    }

    pub fn triad(&self) -> (Self, Self) {
        (self.adjust_hue_fixed(120.), self.adjust_hue_fixed(-120.))
    }

    pub fn square(&self) -> (Self, Self, Self) {
        (
            self.adjust_hue_fixed(90.),
            self.adjust_hue_fixed(270.),
            self.adjust_hue_fixed(180.),
        )
    }

    /// Rotate the hue angle by the specified number of degrees, using
    /// the RYB color wheel
    pub fn adjust_hue_fixed_ryb(&self, amount: f64) -> Self {
        let (h, s, l, a) = self.to_hsla();
        let h = rgb_hue_to_ryb_hue(h);
        let h = normalize_angle(h + amount);
        let h = ryb_huge_to_rgb_hue(h);
        Self::from_hsla(h, s, l, a)
    }

    fn lab_value(&self) -> deltae::LabValue {
        let (l, a, b, _alpha) = self.to_laba();
        deltae::LabValue {
            l: l as f32,
            a: a as f32,
            b: b as f32,
        }
    }

    pub fn delta_e(&self, other: &Self) -> f32 {
        let a = self.lab_value();
        let b = other.lab_value();
        *deltae::DeltaE::new(a, b, deltae::DEMethod::DE2000).value()
    }

    pub fn contrast_ratio(&self, other: &Self) -> f64 {
        let (_, _, l_a, _) = self.to_hsla();
        let (_, _, l_b, _) = other.to_hsla();
        let a = l_a + 0.05;
        let b = l_b + 0.05;
        if a > b {
            a / b
        } else {
            b / a
        }
    }
}

/// Convert an RGB color space hue angle to an RYB colorspace hue angle
/// <https://github.com/TNMEM/Material-Design-Color-Picker/blob/1afe330c67d9db4deef7031d601324b538b43b09/rybcolor.js#L33>
fn rgb_hue_to_ryb_hue(hue: f64) -> f64 {
    if hue < 35. {
        map_range(hue, 0., 35., 0., 60.)
    } else if hue < 60. {
        map_range(hue, 35., 60., 60., 122.)
    } else if hue < 120. {
        map_range(hue, 60., 120., 122., 165.)
    } else if hue < 180. {
        map_range(hue, 120., 180., 165., 218.)
    } else if hue < 240. {
        map_range(hue, 180., 240., 218., 275.)
    } else if hue < 300. {
        map_range(hue, 240., 300., 275., 330.)
    } else {
        map_range(hue, 300., 360., 330., 360.)
    }
}

/// Convert an RYB color space hue angle to an RGB colorspace hue angle
fn ryb_huge_to_rgb_hue(hue: f64) -> f64 {
    if hue < 60. {
        map_range(hue, 0., 60., 0., 35.)
    } else if hue < 122. {
        map_range(hue, 60., 122., 35., 60.)
    } else if hue < 165. {
        map_range(hue, 122., 165., 60., 120.)
    } else if hue < 218. {
        map_range(hue, 165., 218., 120., 180.)
    } else if hue < 275. {
        map_range(hue, 218., 275., 180., 240.)
    } else if hue < 330. {
        map_range(hue, 275., 330., 240., 300.)
    } else {
        map_range(hue, 330., 360., 300., 360.)
    }
}

fn map_range(x: f64, x1: f64, x2: f64, y1: f64, y2: f64) -> f64 {
    let a_slope = (y2 - y1) / (x2 - x1);
    let a_slope_intercept = y1 - (a_slope * x1);
    x * a_slope + a_slope_intercept
}

fn normalize_angle(t: f64) -> f64 {
    let mut t = t % 360.0;
    if t < 0.0 {
        t += 360.0;
    }
    t
}

fn apply_scale(current: f64, factor: f64) -> f64 {
    let difference = if factor >= 0. { 1.0 - current } else { current };
    let delta = difference.max(0.) * factor;
    (current + delta).max(0.)
}

fn apply_fixed(current: f64, amount: f64) -> f64 {
    (current + amount).max(0.)
}

impl Hash for SrgbaTuple {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_ne_bytes().hash(state);
        self.1.to_ne_bytes().hash(state);
        self.2.to_ne_bytes().hash(state);
        self.3.to_ne_bytes().hash(state);
    }
}

impl Eq for SrgbaTuple {}

fn x_parse_color_component(value: &str) -> Result<f32, ()> {
    let mut component = 0u16;
    let mut num_digits = 0;

    for c in value.chars() {
        num_digits += 1;
        component = component << 4;

        let nybble = match c.to_digit(16) {
            Some(v) => v as u16,
            None => return Err(()),
        };
        component |= nybble;
    }

    // From XParseColor, the `rgb:` prefixed syntax scales the
    // value into 16 bits from the number of bits specified
    Ok((match num_digits {
        1 => (component | component << 4) as f32,
        2 => component as f32,
        3 => (component >> 4) as f32,
        4 => (component >> 8) as f32,
        _ => return Err(()),
    }) / 255.0)
}

impl FromStr for SrgbaTuple {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Workaround <https://github.com/mazznoer/csscolorparser-rs/pull/7/files>
        if !s.is_ascii() {
            return Err(());
        }
        if s.len() > 0 && s.as_bytes()[0] == b'#' {
            // Probably `#RGB`

            let digits = (s.len() - 1) / 3;
            if 1 + (digits * 3) != s.len() {
                return Err(());
            }

            if digits == 0 || digits > 4 {
                // Max of 16 bits supported
                return Err(());
            }

            let mut chars = s.chars().skip(1);

            macro_rules! digit {
                () => {{
                    let mut component = 0u16;

                    for _ in 0..digits {
                        component = component << 4;

                        let nybble = match chars.next().unwrap().to_digit(16) {
                            Some(v) => v as u16,
                            None => return Err(()),
                        };
                        component |= nybble;
                    }

                    // From XParseColor, the `#` syntax takes the most significant
                    // bits and uses those for the color value.  That function produces
                    // 16-bit color components but we want 8-bit components so we shift
                    // or truncate the bits here depending on the number of digits
                    (match digits {
                        1 => (component << 4) as f32,
                        2 => component as f32,
                        3 => (component >> 4) as f32,
                        4 => (component >> 8) as f32,
                        _ => return Err(()),
                    }) / 255.0
                }};
            }
            Ok(Self(digit!(), digit!(), digit!(), 1.0))
        } else if let Some(value) = s.strip_prefix("rgb:") {
            let fields: Vec<&str> = value.split('/').collect();
            if fields.len() != 3 {
                return Err(());
            }

            let red = x_parse_color_component(fields[0])?;
            let green = x_parse_color_component(fields[1])?;
            let blue = x_parse_color_component(fields[2])?;
            Ok(Self(red, green, blue, 1.0))
        } else if let Some(value) = s.strip_prefix("rgba:") {
            let fields: Vec<&str> = value.split('/').collect();
            if fields.len() == 4 {
                let red = x_parse_color_component(fields[0])?;
                let green = x_parse_color_component(fields[1])?;
                let blue = x_parse_color_component(fields[2])?;
                let alpha = x_parse_color_component(fields[3])?;
                return Ok(Self(red, green, blue, alpha));
            }

            let fields: Vec<_> = s[5..].split_ascii_whitespace().collect();
            if fields.len() == 4 {
                fn field(s: &str) -> Result<f32, ()> {
                    if s.ends_with('%') {
                        let v: f32 = s[0..s.len() - 1].parse().map_err(|_| ())?;
                        Ok(v / 100.)
                    } else {
                        let v: f32 = s.parse().map_err(|_| ())?;
                        if v > 255.0 || v < 0. {
                            Err(())
                        } else {
                            Ok(v / 255.)
                        }
                    }
                }
                let r: f32 = field(fields[0])?;
                let g: f32 = field(fields[1])?;
                let b: f32 = field(fields[2])?;
                let a: f32 = field(fields[3])?;

                Ok(Self(r, g, b, a))
            } else {
                Err(())
            }
        } else if s.starts_with("hsl:") {
            let fields: Vec<_> = s[4..].split_ascii_whitespace().collect();
            if fields.len() == 3 {
                // Expected to be degrees in range 0-360, but we allow for negative and wrapping
                let h: i32 = fields[0].parse().map_err(|_| ())?;
                // Expected to be percentage in range 0-100
                let s: i32 = fields[1].parse().map_err(|_| ())?;
                // Expected to be percentage in range 0-100
                let l: i32 = fields[2].parse().map_err(|_| ())?;

                fn hsl_to_rgb(hue: i32, sat: i32, light: i32) -> (f32, f32, f32) {
                    let hue = hue % 360;
                    let hue = if hue < 0 { hue + 360 } else { hue } as f32;
                    let sat = sat as f32 / 100.;
                    let light = light as f32 / 100.;
                    let a = sat * light.min(1. - light);
                    let f = |n: f32| -> f32 {
                        let k = (n + hue / 30.) % 12.;
                        light - a * (k - 3.).min(9. - k).min(1.).max(-1.)
                    };
                    (f(0.), f(8.), f(4.))
                }

                let (r, g, b) = hsl_to_rgb(h, s, l);
                Ok(Self(r, g, b, 1.0))
            } else {
                Err(())
            }
        } else if let Ok(c) = csscolorparser::parse(s) {
            Ok(Self(c.r as f32, c.g as f32, c.b as f32, c.a as f32))
        } else {
            Self::from_named(s).ok_or(())
        }
    }
}

/// A pixel value encoded as linear RGBA values in f32 format (range: 0.0-1.0)
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct LinearRgba(pub f32, pub f32, pub f32, pub f32);

impl Eq for LinearRgba {}

impl Hash for LinearRgba {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self.0.to_ne_bytes().hash(state);
        self.1.to_ne_bytes().hash(state);
        self.2.to_ne_bytes().hash(state);
        self.3.to_ne_bytes().hash(state);
    }
}

impl From<(f32, f32, f32, f32)> for LinearRgba {
    fn from((r, g, b, a): (f32, f32, f32, f32)) -> Self {
        Self(r, g, b, a)
    }
}

impl From<[f32; 4]> for LinearRgba {
    fn from([r, g, b, a]: [f32; 4]) -> Self {
        Self(r, g, b, a)
    }
}

impl Into<[f32; 4]> for LinearRgba {
    fn into(self) -> [f32; 4] {
        [self.0, self.1, self.2, self.3]
    }
}

impl LinearRgba {
    /// Convert SRGBA u8 components to LinearRgba.
    /// Note that alpha in SRGBA colorspace is already linear,
    /// so this only applies gamma correction to RGB.
    pub fn with_srgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self(
            srgb8_to_linear_f32(red),
            srgb8_to_linear_f32(green),
            srgb8_to_linear_f32(blue),
            rgb_to_linear_f32(alpha),
        )
    }

    /// Convert linear RGBA u8 components to LinearRgba (f32)
    pub fn with_rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self(
            rgb_to_linear_f32(red),
            rgb_to_linear_f32(green),
            rgb_to_linear_f32(blue),
            rgb_to_linear_f32(alpha),
        )
    }

    /// Create using the provided f32 components in the range 0.0-1.0
    pub const fn with_components(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self(red, green, blue, alpha)
    }

    pub const TRANSPARENT: Self = Self::with_components(0., 0., 0., 0.);

    /// Returns true if this color is fully transparent
    pub fn is_fully_transparent(self) -> bool {
        self.3 == 0.0
    }

    /// Returns self, except when self is transparent, in which case returns other
    pub fn when_fully_transparent(self, other: Self) -> Self {
        if self.is_fully_transparent() {
            other
        } else {
            self
        }
    }

    /// Returns self multiplied by the supplied alpha value
    pub fn mul_alpha(self, alpha: f32) -> Self {
        Self(self.0, self.1, self.2, self.3 * alpha)
    }

    /// Convert to an SRGB u32 pixel
    pub fn srgba_pixel(self) -> SrgbaPixel {
        SrgbaPixel::rgba(
            linear_f32_to_srgb8_using_table(self.0),
            linear_f32_to_srgb8_using_table(self.1),
            linear_f32_to_srgb8_using_table(self.2),
            (self.3 * 255.) as u8,
        )
    }

    /// Returns the individual RGBA channels as f32 components 0.0-1.0
    pub fn tuple(self) -> (f32, f32, f32, f32) {
        (self.0, self.1, self.2, self.3)
    }

    pub fn to_srgb(self) -> SrgbaTuple {
        // Note that alpha is always linear
        SrgbaTuple(
            linear_f32_to_srgbf32(self.0),
            linear_f32_to_srgbf32(self.1),
            linear_f32_to_srgbf32(self.2),
            self.3,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn named_rgb() {
        let dark_green = SrgbaTuple::from_named("DarkGreen").unwrap();
        assert_eq!(dark_green.to_rgb_string(), "#006400");
    }

    #[test]
    fn from_hsl() {
        let foo = SrgbaTuple::from_str("hsl:235 100  50").unwrap();
        assert_eq!(foo.to_rgb_string(), "#0015ff");
    }

    #[test]
    fn from_rgba() {
        assert_eq!(
            SrgbaTuple::from_str("clear").unwrap().to_rgba_string(),
            "rgba(0% 0% 0% 0%)"
        );
        assert_eq!(
            SrgbaTuple::from_str("rgba:100% 0 0 50%")
                .unwrap()
                .to_rgba_string(),
            "rgba(100% 0% 0% 50%)"
        );
    }

    #[test]
    fn from_css() {
        assert_eq!(
            SrgbaTuple::from_str("rgb(255,0,0)")
                .unwrap()
                .to_rgb_string(),
            "#ff0000"
        );

        let rgba = SrgbaTuple::from_str("rgba(255,0,0,1)").unwrap();
        let round_trip = SrgbaTuple::from_str(&rgba.to_rgba_string()).unwrap();
        assert_eq!(rgba, round_trip);
        assert_eq!(rgba.to_rgba_string(), "rgba(100% 0% 0% 100%)");
    }

    #[test]
    fn from_rgb() {
        assert!(SrgbaTuple::from_str("").is_err());
        assert!(SrgbaTuple::from_str("#xyxyxy").is_err());

        let foo = SrgbaTuple::from_str("#f00f00f00").unwrap();
        assert_eq!(foo.to_rgb_string(), "#f0f0f0");

        let black = SrgbaTuple::from_str("#000").unwrap();
        assert_eq!(black.to_rgb_string(), "#000000");

        let black = SrgbaTuple::from_str("#FFF").unwrap();
        assert_eq!(black.to_rgb_string(), "#f0f0f0");

        let black = SrgbaTuple::from_str("#000000").unwrap();
        assert_eq!(black.to_rgb_string(), "#000000");

        let grey = SrgbaTuple::from_str("rgb:D6/D6/D6").unwrap();
        assert_eq!(grey.to_rgb_string(), "#d6d6d6");

        let grey = SrgbaTuple::from_str("rgb:f0f0/f0f0/f0f0").unwrap();
        assert_eq!(grey.to_rgb_string(), "#f0f0f0");
    }
}
