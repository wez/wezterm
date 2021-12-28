use serde::{Deserializer, Serialize, Serializer};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DefaultUnit {
    Points,
    Pixels,
    Percent,
    Cells,
}

impl DefaultUnit {
    fn to_dimension(self, value: f32) -> Dimension {
        match self {
            Self::Points => Dimension::Points(value),
            Self::Pixels => Dimension::Pixels(value),
            Self::Percent => Dimension::Percent(value / 100.),
            Self::Cells => Dimension::Cells(value),
        }
    }
}

impl<'de> serde::de::Visitor<'de> for DefaultUnit {
    type Value = Dimension;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("f64 or i64")
    }

    fn visit_f32<E>(self, value: f32) -> Result<Dimension, E>
    where
        E: serde::de::Error,
    {
        Ok(self.to_dimension(value))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Dimension, E>
    where
        E: serde::de::Error,
    {
        Ok(self.to_dimension(value as f32))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Dimension, E>
    where
        E: serde::de::Error,
    {
        Ok(self.to_dimension(value as f32))
    }

    fn visit_str<E>(self, s: &str) -> Result<Dimension, E>
    where
        E: serde::de::Error,
    {
        if let Ok(value) = s.parse::<f32>() {
            Ok(self.to_dimension(value))
        } else {
            fn is_unit(s: &str, unit: &'static str) -> Option<f32> {
                let s = s.strip_suffix(unit)?.trim();
                s.parse().ok()
            }

            if let Some(v) = is_unit(s, "px") {
                Ok(DefaultUnit::Pixels.to_dimension(v))
            } else if let Some(v) = is_unit(s, "%") {
                Ok(DefaultUnit::Percent.to_dimension(v))
            } else if let Some(v) = is_unit(s, "pt") {
                Ok(DefaultUnit::Points.to_dimension(v))
            } else if let Some(v) = is_unit(s, "cell") {
                Ok(DefaultUnit::Cells.to_dimension(v))
            } else {
                Err(serde::de::Error::custom(format!(
                    "expected either a number or a string of \
                        the form '123px' where 'px' is a unit and \
                        can be one of 'px', '%', 'pt' or 'cell', \
                        but got {}",
                    s
                )))
            }
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Dimension {
    /// A value expressed in points, where 72 points == 1 inch.
    Points(f32),

    /// A value expressed in raw pixels
    Pixels(f32),

    /// A value expressed in terms of a fraction of the maximum
    /// value in the same direction.  For example, left padding
    /// of 10% depends on the pixel width of that element.
    /// The value is 1.0 == 100%.  It is possible to express
    /// eg: 2.0 for 200%.
    Percent(f32),

    /// A value expressed in terms of a fraction of the cell
    /// size computed from the configured font size.
    /// 1.0 == the cell size.
    Cells(f32),
}

impl Dimension {
    pub fn is_zero(&self) -> bool {
        match self {
            Self::Points(n) | Self::Pixels(n) | Self::Percent(n) | Self::Cells(n) => *n == 0.,
        }
    }
}

impl Default for Dimension {
    fn default() -> Self {
        Self::Pixels(0.)
    }
}

impl Serialize for Dimension {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = match self {
            Self::Points(n) => format!("{}pt", n),
            Self::Pixels(n) => format!("{}px", n),
            Self::Percent(n) => format!("{}%", n * 100.),
            Self::Cells(n) => format!("{}cell", n),
        };
        serializer.serialize_str(&s)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DimensionContext {
    pub dpi: f32,
    /// Width/Height or other upper bound on the dimension,
    /// measured in pixels.
    pub pixel_max: f32,
    /// Width/Height of the font metrics cell size in the appropriate
    /// dimension, measured in pixels.
    pub pixel_cell: f32,
}

impl Dimension {
    pub fn evaluate_as_pixels(&self, context: DimensionContext) -> f32 {
        match self {
            Self::Pixels(n) => n.floor(),
            Self::Points(pt) => (pt * context.dpi / 72.0).floor(),
            Self::Percent(p) => (p * context.pixel_max).floor(),
            Self::Cells(c) => (c * context.pixel_cell).floor(),
        }
    }
}

fn de_dimension<'de, D>(unit: DefaultUnit, deserializer: D) -> Result<Dimension, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_any(unit)
}

pub fn de_pixels<'de, D>(deserializer: D) -> Result<Dimension, D::Error>
where
    D: Deserializer<'de>,
{
    de_dimension(DefaultUnit::Pixels, deserializer)
}

pub fn de_points<'de, D>(deserializer: D) -> Result<Dimension, D::Error>
where
    D: Deserializer<'de>,
{
    de_dimension(DefaultUnit::Points, deserializer)
}

pub fn de_percent<'de, D>(deserializer: D) -> Result<Dimension, D::Error>
where
    D: Deserializer<'de>,
{
    de_dimension(DefaultUnit::Percent, deserializer)
}

pub fn de_cells<'de, D>(deserializer: D) -> Result<Dimension, D::Error>
where
    D: Deserializer<'de>,
{
    de_dimension(DefaultUnit::Cells, deserializer)
}
