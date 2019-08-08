use crate::Operator;
use palette::{Blend, Srgb, Srgba};

/// A color stored as big endian bgra32
#[derive(Copy, Clone, Debug)]
pub struct Color(pub u32);

impl From<Srgb> for Color {
    #[inline]
    fn from(s: Srgb) -> Color {
        let b: Srgb<u8> = s.into_format();
        let b = b.into_components();
        Color::rgb(b.0, b.1, b.2)
    }
}

impl From<Srgba> for Color {
    #[inline]
    fn from(s: Srgba) -> Color {
        let b: Srgba<u8> = s.into_format();
        let b = b.into_components();
        Color::rgba(b.0, b.1, b.2, b.3)
    }
}

impl From<Color> for Srgb {
    #[inline]
    fn from(c: Color) -> Srgb {
        let c = c.as_rgba();
        let s = Srgb::<u8>::new(c.0, c.1, c.2);
        s.into_format()
    }
}

impl From<Color> for Srgba {
    #[inline]
    fn from(c: Color) -> Srgba {
        let c = c.as_rgba();
        let s = Srgba::<u8>::new(c.0, c.1, c.2, c.3);
        s.into_format()
    }
}

impl Color {
    #[inline]
    pub fn rgb(red: u8, green: u8, blue: u8) -> Color {
        Color::rgba(red, green, blue, 0xff)
    }

    #[inline]
    pub fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Color {
        let word = (blue as u32) << 24 | (green as u32) << 16 | (red as u32) << 8 | alpha as u32;
        Color(word.to_be())
    }

    #[inline]
    pub fn as_rgba(&self) -> (u8, u8, u8, u8) {
        let host = u32::from_be(self.0);
        (
            (host >> 8) as u8,
            (host >> 16) as u8,
            (host >> 24) as u8,
            (host & 0xff) as u8,
        )
    }

    /// Compute the composite of two colors according to the supplied operator.
    /// self is the src operand, dest is the dest operand.
    #[inline]
    pub fn composite(&self, dest: Color, operator: &Operator) -> Color {
        match operator {
            &Operator::Over => {
                let src: Srgba = (*self).into();
                let dest: Srgba = dest.into();
                Srgba::from_linear(src.into_linear().over(dest.into_linear())).into()
            }
            &Operator::Source => *self,
            &Operator::Multiply => {
                let src: Srgba = (*self).into();
                let dest: Srgba = dest.into();
                let result: Color =
                    Srgba::from_linear(src.into_linear().multiply(dest.into_linear())).into();
                result.into()
            }
            &Operator::MultiplyThenOver(ref tint) => {
                // First multiply by the tint color.  This colorizes the glyph.
                let src: Srgba = (*self).into();
                let tint: Srgba = (*tint).into();
                let mut tinted = src.into_linear().multiply(tint.into_linear());
                // We take the alpha from the source.  This is important because
                // we're using Multiply to tint the glyph and if we don't reset the
                // alpha we tend to end up with a background square of the tint color.
                tinted.alpha = src.alpha;
                // Then blend the tinted glyph over the destination background
                let dest: Srgba = dest.into();
                Srgba::from_linear(tinted.over(dest.into_linear())).into()
            }
        }
    }
}
