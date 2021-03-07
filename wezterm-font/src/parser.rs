use crate::locator::FontDataHandle;
use crate::shaper::GlyphInfo;
use anyhow::anyhow;
use config::FontAttributes;
use std::path::{Path, PathBuf};
use ttf_parser::{fonts_in_collection, Face, Name, PlatformId};

#[derive(Debug)]
pub enum MaybeShaped {
    Resolved(GlyphInfo),
    Unresolved { raw: String, slice_start: usize },
}

/// Represents a parsed font
pub struct ParsedFont {
    names: Names,
}

#[derive(Debug)]
pub struct Names {
    pub full_name: String,
    pub family: Option<String>,
    pub sub_family: Option<String>,
    pub postscript_name: Option<String>,
}

/// Computes a score for a given name record; font files can contain
/// multiple variants of the same logical name encoded differently
/// for various operating systems and languages.
/// This function assigns a weight to each of the combinations;
/// we generally prefer the English rendition of the name in unicode.
///
/// Borrowed from a similar bit of code in the allsorts crate.
fn score(name: &Name) -> Option<usize> {
    match (name.platform_id(), name.encoding_id(), name.language_id()) {
        (PlatformId::Windows, 10, _) => Some(1000),
        (PlatformId::Unicode, 6, 0) => Some(900),
        (PlatformId::Unicode, 4, 0) => Some(800),
        (PlatformId::Windows, 1, 0x409) => Some(750),
        (PlatformId::Windows, 1, lang) if lang != 0x409 => Some(700),
        (PlatformId::Unicode, 3, 0) => Some(600),
        (PlatformId::Unicode, 2, 0) => Some(500),
        (PlatformId::Unicode, 1, 0) => Some(400),
        (PlatformId::Unicode, 0, 0) => Some(300),
        (PlatformId::Windows, 0, _) => Some(200),
        (PlatformId::Macintosh, 0, 0) => Some(150),
        (PlatformId::Macintosh, 0, lang) if lang != 0 => Some(100),
        _ => None,
    }
}

/// Maybe convert a MacRoman byte to a unicode char.
/// Borrowed from the allsorts crate.
fn macroman_to_char(b: u8) -> Option<char> {
    match b {
        0..=127 => Some(b as char),
        128 => Some('Ä'),  // A dieresis
        129 => Some('Å'),  // A ring
        130 => Some('Ç'),  // C cedilla
        131 => Some('É'),  // E acute
        132 => Some('Ñ'),  // N tilde
        133 => Some('Ö'),  // O dieresis
        134 => Some('Ü'),  // U dieresis
        135 => Some('á'),  // a acute
        136 => Some('à'),  // a grave
        137 => Some('â'),  // a circumflex
        138 => Some('ä'),  // a dieresis
        139 => Some('ã'),  // a tilde
        140 => Some('å'),  // a ring
        141 => Some('ç'),  // c cedilla
        142 => Some('é'),  // e acute
        143 => Some('è'),  // e grave
        144 => Some('ê'),  // e circumflex
        145 => Some('ë'),  // e dieresis
        146 => Some('í'),  // i acute
        147 => Some('ì'),  // i grave
        148 => Some('î'),  // i circumflex
        149 => Some('ï'),  // i dieresis
        150 => Some('ñ'),  // n tilde
        151 => Some('ó'),  // o acute
        152 => Some('ò'),  // o grave
        153 => Some('ô'),  // o circumflex
        154 => Some('ö'),  // o dieresis
        155 => Some('õ'),  // o tilde
        156 => Some('ú'),  // u acute
        157 => Some('ù'),  // u grave
        158 => Some('û'),  // u circumflex
        159 => Some('ü'),  // u dieresis
        160 => Some('†'), // dagger
        161 => Some('°'),  // degree
        162 => Some('¢'),  // cent
        163 => Some('£'),  // sterling
        164 => Some('§'),  // section
        165 => Some('•'), // bullet
        166 => Some('¶'),  // paragraph
        167 => Some('ß'),  // German double s
        168 => Some('®'),  // registered
        169 => Some('©'),  // copyright
        170 => Some('™'), // trademark
        171 => Some('´'),  // acute
        172 => Some('¨'),  // diaeresis
        174 => Some('Æ'),  // AE
        175 => Some('Ø'),  // O slash
        177 => Some('±'),  // plusminus
        180 => Some('¥'),  // yen
        181 => Some('µ'),  // micro
        187 => Some('ª'),  // ordfeminine
        188 => Some('º'),  // ordmasculine
        190 => Some('æ'),  // ae
        191 => Some('ø'),  // o slash
        192 => Some('¿'),  // question down
        193 => Some('¡'),  // exclamation down
        194 => Some('¬'),  // not
        196 => Some('ƒ'),  // florin
        199 => Some('«'),  // left guille
        200 => Some('»'),  // right guille
        201 => Some('…'), // ellipsis
        202 => Some(' '),   // non-breaking space
        203 => Some('À'),  // A grave
        204 => Some('Ã'),  // A tilde
        205 => Some('Õ'),  // O tilde
        206 => Some('Œ'),  // OE
        207 => Some('œ'),  // oe
        208 => Some('–'), // endash
        209 => Some('—'), // emdash
        210 => Some('“'), // ldquo
        211 => Some('”'), // rdquo
        212 => Some('‘'), // lsquo
        213 => Some('’'), // rsquo
        214 => Some('÷'),  // divide
        216 => Some('ÿ'),  // y dieresis
        217 => Some('Ÿ'),  // Y dieresis
        218 => Some('⁄'), // fraction
        219 => Some('¤'),  // currency
        220 => Some('‹'), // left single guille
        221 => Some('›'), // right single guille
        222 => Some('ﬁ'), // fi
        223 => Some('ﬂ'), // fl
        224 => Some('‡'), // double dagger
        225 => Some('·'),  // middle dot
        226 => Some('‚'), // single quote base
        227 => Some('„'), // double quote base
        228 => Some('‰'), // perthousand
        229 => Some('Â'),  // A circumflex
        230 => Some('Ê'),  // E circumflex
        231 => Some('Á'),  // A acute
        232 => Some('Ë'),  // E dieresis
        233 => Some('È'),  // E grave
        234 => Some('Í'),  // I acute
        235 => Some('Î'),  // I circumflex
        236 => Some('Ï'),  // I dieresis
        237 => Some('Ì'),  // I grave
        238 => Some('Ó'),  // O acute
        239 => Some('Ô'),  // O circumflex
        241 => Some('Ò'),  // O grave
        242 => Some('Ú'),  // U acute
        243 => Some('Û'),  // U circumflex
        244 => Some('Ù'),  // U grave
        245 => Some('ı'),  // dot-less i
        246 => Some('^'),   // circumflex
        247 => Some('˜'),  // tilde
        248 => Some('¯'),  // macron
        249 => Some('˘'),  // breve
        250 => Some('˙'),  // dot accent
        251 => Some('˚'),  // ring
        252 => Some('¸'),  // cedilla
        253 => Some('˝'),  // Hungarian umlaut (double acute accent)
        254 => Some('˛'),  // ogonek
        255 => Some('ˇ'),  // caron
        _ => None,
    }
}

/// Return a unicode version of the name
fn decode_name(name: &Name) -> Option<String> {
    if name.platform_id() == PlatformId::Macintosh {
        Some(
            name.name()
                .iter()
                .filter_map(|&b| macroman_to_char(b))
                .collect::<String>(),
        )
    } else {
        name.to_string()
    }
}

impl Names {
    fn from_face(face: &Face) -> anyhow::Result<Names> {
        // The names table isn't very amenable to a direct lookup, and there
        // can be multiple candidate encodings for a given font name.
        // Since we need to lookup multiple names, we copy potential
        // candidates into a vector and then sort it so that the best
        // candidates are towards the front of the vector.
        // This should result in less overall work to extract the names.
        let mut names = face
            .names()
            .filter(|name| {
                let id = name.name_id();
                let interesting_id = id == ttf_parser::name_id::FAMILY
                    || id == ttf_parser::name_id::SUBFAMILY
                    || id == ttf_parser::name_id::FULL_NAME
                    || id == ttf_parser::name_id::POST_SCRIPT_NAME;
                interesting_id && (name.is_unicode() || name.platform_id() == PlatformId::Macintosh)
            })
            .collect::<Vec<_>>();
        // Best scores at the front
        names.sort_by(|a, b| score(a).cmp(&score(b)).reverse());

        // Now looking up a name is a simple matter of finding the
        // first entry with the desired id
        fn get_name(names: &[Name], id: u16) -> Option<String> {
            let name = names.iter().find(|n| n.name_id() == id)?;
            decode_name(name)
        }

        Ok(Names {
            full_name: get_name(&names, ttf_parser::name_id::FULL_NAME)
                .ok_or_else(|| anyhow!("missing full name"))?,
            family: get_name(&names, ttf_parser::name_id::FAMILY),
            sub_family: get_name(&names, ttf_parser::name_id::SUBFAMILY),
            postscript_name: get_name(&names, ttf_parser::name_id::POST_SCRIPT_NAME),
        })
    }
}

impl ParsedFont {
    pub fn from_locator(handle: &FontDataHandle) -> anyhow::Result<Self> {
        match handle {
            FontDataHandle::OnDisk { path, index } => {
                let data = std::fs::read(path)?;
                let face = Face::from_slice(&data, *index)?;
                Ok(Self {
                    names: Names::from_face(&face)?,
                })
            }

            FontDataHandle::Memory { data, index, .. } => {
                let face = Face::from_slice(data, *index)?;
                Ok(Self {
                    names: Names::from_face(&face)?,
                })
            }
        }
    }

    pub fn names(&self) -> &Names {
        &self.names
    }
}

pub fn font_info_matches(attr: &FontAttributes, names: &Names) -> bool {
    if let Some(fam) = names.family.as_ref() {
        // TODO: correctly match using family and sub-family;
        // this is a pretty rough approximation
        if attr.family == *fam {
            match names.sub_family.as_ref().map(String::as_str) {
                Some("Italic") if attr.italic && !attr.bold => return true,
                Some("Bold") if attr.bold && !attr.italic => return true,
                Some("Bold Italic") if attr.bold && attr.italic => return true,
                Some("Medium") | Some("Regular") | None if !attr.italic && !attr.bold => {
                    return true
                }
                _ => {}
            }
        }
    }
    if attr.family == names.full_name && !attr.bold && !attr.italic {
        true
    } else {
        false
    }
}

/// Given a blob representing a True Type Collection (.ttc) file,
/// and a desired font, enumerate the collection to resolve the index of
/// the font inside that collection that matches it.
/// Even though this is intended to work with a TTC, this also returns
/// the index of a singular TTF file, if it matches.
pub fn resolve_font_from_ttc_data(
    attr: &FontAttributes,
    data: &[u8],
) -> anyhow::Result<Option<usize>> {
    if let Some(size) = fonts_in_collection(data) {
        for index in 0..size {
            let face = Face::from_slice(data, index)?;
            let names = Names::from_face(&face)?;
            if font_info_matches(attr, &names) {
                return Ok(Some(index as usize));
            }
        }
        Ok(None)
    } else {
        let face = Face::from_slice(data, 0)?;
        let names = Names::from_face(&face)?;
        if font_info_matches(attr, &names) {
            Ok(Some(0))
        } else {
            Ok(None)
        }
    }
}

/// In case the user has a broken configuration, or no configuration,
/// we bundle JetBrains Mono and Noto Color Emoji to act as reasonably
/// sane fallback fonts.
/// This function loads those.
pub(crate) fn load_built_in_fonts(
    font_info: &mut Vec<(Names, PathBuf, FontDataHandle)>,
) -> anyhow::Result<()> {
    macro_rules! font {
        ($font:literal) => {
            (include_bytes!($font) as &'static [u8], $font)
        };
    }
    for (data, name) in &[
        font!("../../assets/fonts/JetBrainsMono-BoldItalic.ttf"),
        font!("../../assets/fonts/JetBrainsMono-Bold.ttf"),
        font!("../../assets/fonts/JetBrainsMono-ExtraBoldItalic.ttf"),
        font!("../../assets/fonts/JetBrainsMono-ExtraBold.ttf"),
        font!("../../assets/fonts/JetBrainsMono-ExtraLightItalic.ttf"),
        font!("../../assets/fonts/JetBrainsMono-ExtraLight.ttf"),
        font!("../../assets/fonts/JetBrainsMono-Italic.ttf"),
        font!("../../assets/fonts/JetBrainsMono-LightItalic.ttf"),
        font!("../../assets/fonts/JetBrainsMono-Light.ttf"),
        font!("../../assets/fonts/JetBrainsMono-MediumItalic.ttf"),
        font!("../../assets/fonts/JetBrainsMono-Medium.ttf"),
        font!("../../assets/fonts/JetBrainsMono-Regular.ttf"),
        font!("../../assets/fonts/JetBrainsMono-ThinItalic.ttf"),
        font!("../../assets/fonts/JetBrainsMono-Thin.ttf"),
        font!("../../assets/fonts/NotoColorEmoji.ttf"),
        font!("../../assets/fonts/PowerlineExtraSymbols.otf"),
        font!("../../assets/fonts/LastResortHE-Regular.ttf"),
    ] {
        let face = Face::from_slice(data, 0)?;
        let names = Names::from_face(&face)?;
        font_info.push((
            names,
            PathBuf::from(name),
            FontDataHandle::Memory {
                data: data.to_vec(),
                index: 0,
                name: name.to_string(),
            },
        ));
    }

    Ok(())
}

pub(crate) fn parse_and_collect_font_info(
    path: &Path,
    font_info: &mut Vec<(Names, PathBuf, FontDataHandle)>,
) -> anyhow::Result<()> {
    let data = std::fs::read(path)?;
    let size = fonts_in_collection(&data).unwrap_or(0);

    fn load_one(
        data: &[u8],
        path: &Path,
        index: u32,
        font_info: &mut Vec<(Names, PathBuf, FontDataHandle)>,
    ) -> anyhow::Result<()> {
        let face = Face::from_slice(data, index)?;
        let names = Names::from_face(&face)?;
        font_info.push((
            names,
            path.to_path_buf(),
            FontDataHandle::OnDisk {
                path: path.to_path_buf(),
                index,
            },
        ));
        Ok(())
    }

    for index in 0..=size {
        if let Err(err) = load_one(&data, path, index, font_info) {
            log::trace!(
                "error while parsing {} index {}: {}",
                path.display(),
                index,
                err
            );
        }
    }

    Ok(())
}
