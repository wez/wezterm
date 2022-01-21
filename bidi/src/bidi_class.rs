//! Generated from bidi/data/DerivedBidiClass.txt by bidi/generate/src/main.rs

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
#[repr(u8)]
pub enum BidiClass {
    ArabicLetter,
    ArabicNumber,
    BoundaryNeutral,
    CommonSeparator,
    EuropeanNumber,
    EuropeanSeparator,
    EuropeanTerminator,
    FirstStrongIsolate,
    LeftToRight,
    LeftToRightEmbedding,
    LeftToRightIsolate,
    LeftToRightOverride,
    NonspacingMark,
    OtherNeutral,
    ParagraphSeparator,
    PopDirectionalFormat,
    PopDirectionalIsolate,
    RightToLeft,
    RightToLeftEmbedding,
    RightToLeftIsolate,
    RightToLeftOverride,
    SegmentSeparator,
    WhiteSpace,
}

pub const BIDI_CLASS: &'static [(char, char, BidiClass)] = &[
    ('\u{0}', '\u{8}', BidiClass::BoundaryNeutral), // Cc   [9] <control-0000>..<control-0008>
    ('\u{9}', '\u{9}', BidiClass::SegmentSeparator), // Cc       <control-0009>
    ('\u{a}', '\u{a}', BidiClass::ParagraphSeparator), // Cc       <control-000A>
    ('\u{b}', '\u{b}', BidiClass::SegmentSeparator), // Cc       <control-000B>
    ('\u{c}', '\u{c}', BidiClass::WhiteSpace),      // Cc       <control-000C>
    ('\u{d}', '\u{d}', BidiClass::ParagraphSeparator), // Cc       <control-000D>
    ('\u{e}', '\u{1b}', BidiClass::BoundaryNeutral), // Cc  [14] <control-000E>..<control-001B>
    ('\u{1c}', '\u{1e}', BidiClass::ParagraphSeparator), // Cc   [3] <control-001C>..<control-001E>
    ('\u{1f}', '\u{1f}', BidiClass::SegmentSeparator), // Cc       <control-001F>
    ('\u{20}', '\u{20}', BidiClass::WhiteSpace),    // Zs       SPACE
    ('\u{21}', '\u{22}', BidiClass::OtherNeutral),  // Po   [2] EXCLAMATION MARK..QUOTATION MARK
    ('\u{23}', '\u{23}', BidiClass::EuropeanTerminator), // Po       NUMBER SIGN
    ('\u{24}', '\u{24}', BidiClass::EuropeanTerminator), // Sc       DOLLAR SIGN
    ('\u{25}', '\u{25}', BidiClass::EuropeanTerminator), // Po       PERCENT SIGN
    ('\u{26}', '\u{27}', BidiClass::OtherNeutral),  // Po   [2] AMPERSAND..APOSTROPHE
    ('\u{28}', '\u{28}', BidiClass::OtherNeutral),  // Ps       LEFT PARENTHESIS
    ('\u{29}', '\u{29}', BidiClass::OtherNeutral),  // Pe       RIGHT PARENTHESIS
    ('\u{2a}', '\u{2a}', BidiClass::OtherNeutral),  // Po       ASTERISK
    ('\u{2b}', '\u{2b}', BidiClass::EuropeanSeparator), // Sm       PLUS SIGN
    ('\u{2c}', '\u{2c}', BidiClass::CommonSeparator), // Po       COMMA
    ('\u{2d}', '\u{2d}', BidiClass::EuropeanSeparator), // Pd       HYPHEN-MINUS
    ('\u{2e}', '\u{2f}', BidiClass::CommonSeparator), // Po   [2] FULL STOP..SOLIDUS
    ('\u{30}', '\u{39}', BidiClass::EuropeanNumber), // Nd  [10] DIGIT ZERO..DIGIT NINE
    ('\u{3a}', '\u{3a}', BidiClass::CommonSeparator), // Po       COLON
    ('\u{3b}', '\u{3b}', BidiClass::OtherNeutral),  // Po       SEMICOLON
    ('\u{3c}', '\u{3e}', BidiClass::OtherNeutral),  // Sm   [3] LESS-THAN SIGN..GREATER-THAN SIGN
    ('\u{3f}', '\u{40}', BidiClass::OtherNeutral),  // Po   [2] QUESTION MARK..COMMERCIAL AT
    ('\u{41}', '\u{5a}', BidiClass::LeftToRight), // L&  [26] LATIN CAPITAL LETTER A..LATIN CAPITAL LETTER Z
    ('\u{5b}', '\u{5b}', BidiClass::OtherNeutral), // Ps       LEFT SQUARE BRACKET
    ('\u{5c}', '\u{5c}', BidiClass::OtherNeutral), // Po       REVERSE SOLIDUS
    ('\u{5d}', '\u{5d}', BidiClass::OtherNeutral), // Pe       RIGHT SQUARE BRACKET
    ('\u{5e}', '\u{5e}', BidiClass::OtherNeutral), // Sk       CIRCUMFLEX ACCENT
    ('\u{5f}', '\u{5f}', BidiClass::OtherNeutral), // Pc       LOW LINE
    ('\u{60}', '\u{60}', BidiClass::OtherNeutral), // Sk       GRAVE ACCENT
    ('\u{61}', '\u{7a}', BidiClass::LeftToRight), // L&  [26] LATIN SMALL LETTER A..LATIN SMALL LETTER Z
    ('\u{7b}', '\u{7b}', BidiClass::OtherNeutral), // Ps       LEFT CURLY BRACKET
    ('\u{7c}', '\u{7c}', BidiClass::OtherNeutral), // Sm       VERTICAL LINE
    ('\u{7d}', '\u{7d}', BidiClass::OtherNeutral), // Pe       RIGHT CURLY BRACKET
    ('\u{7e}', '\u{7e}', BidiClass::OtherNeutral), // Sm       TILDE
    ('\u{7f}', '\u{84}', BidiClass::BoundaryNeutral), // Cc   [6] <control-007F>..<control-0084>
    ('\u{85}', '\u{85}', BidiClass::ParagraphSeparator), // Cc       <control-0085>
    ('\u{86}', '\u{9f}', BidiClass::BoundaryNeutral), // Cc  [26] <control-0086>..<control-009F>
    ('\u{a0}', '\u{a0}', BidiClass::CommonSeparator), // Zs       NO-BREAK SPACE
    ('\u{a1}', '\u{a1}', BidiClass::OtherNeutral), // Po       INVERTED EXCLAMATION MARK
    ('\u{a2}', '\u{a5}', BidiClass::EuropeanTerminator), // Sc   [4] CENT SIGN..YEN SIGN
    ('\u{a6}', '\u{a6}', BidiClass::OtherNeutral), // So       BROKEN BAR
    ('\u{a7}', '\u{a7}', BidiClass::OtherNeutral), // Po       SECTION SIGN
    ('\u{a8}', '\u{a8}', BidiClass::OtherNeutral), // Sk       DIAERESIS
    ('\u{a9}', '\u{a9}', BidiClass::OtherNeutral), // So       COPYRIGHT SIGN
    ('\u{aa}', '\u{aa}', BidiClass::LeftToRight), // Lo       FEMININE ORDINAL INDICATOR
    ('\u{ab}', '\u{ab}', BidiClass::OtherNeutral), // Pi       LEFT-POINTING DOUBLE ANGLE QUOTATION MARK
    ('\u{ac}', '\u{ac}', BidiClass::OtherNeutral), // Sm       NOT SIGN
    ('\u{ad}', '\u{ad}', BidiClass::BoundaryNeutral), // Cf       SOFT HYPHEN
    ('\u{ae}', '\u{ae}', BidiClass::OtherNeutral), // So       REGISTERED SIGN
    ('\u{af}', '\u{af}', BidiClass::OtherNeutral), // Sk       MACRON
    ('\u{b0}', '\u{b0}', BidiClass::EuropeanTerminator), // So       DEGREE SIGN
    ('\u{b1}', '\u{b1}', BidiClass::EuropeanTerminator), // Sm       PLUS-MINUS SIGN
    ('\u{b2}', '\u{b3}', BidiClass::EuropeanNumber), // No   [2] SUPERSCRIPT TWO..SUPERSCRIPT THREE
    ('\u{b4}', '\u{b4}', BidiClass::OtherNeutral), // Sk       ACUTE ACCENT
    ('\u{b5}', '\u{b5}', BidiClass::LeftToRight),  // L&       MICRO SIGN
    ('\u{b6}', '\u{b7}', BidiClass::OtherNeutral), // Po   [2] PILCROW SIGN..MIDDLE DOT
    ('\u{b8}', '\u{b8}', BidiClass::OtherNeutral), // Sk       CEDILLA
    ('\u{b9}', '\u{b9}', BidiClass::EuropeanNumber), // No       SUPERSCRIPT ONE
    ('\u{ba}', '\u{ba}', BidiClass::LeftToRight),  // Lo       MASCULINE ORDINAL INDICATOR
    ('\u{bb}', '\u{bb}', BidiClass::OtherNeutral), // Pf       RIGHT-POINTING DOUBLE ANGLE QUOTATION MARK
    ('\u{bc}', '\u{be}', BidiClass::OtherNeutral), // No   [3] VULGAR FRACTION ONE QUARTER..VULGAR FRACTION THREE QUARTERS
    ('\u{bf}', '\u{bf}', BidiClass::OtherNeutral), // Po       INVERTED QUESTION MARK
    ('\u{c0}', '\u{d6}', BidiClass::LeftToRight), // L&  [23] LATIN CAPITAL LETTER A WITH GRAVE..LATIN CAPITAL LETTER O WITH DIAERESIS
    ('\u{d7}', '\u{d7}', BidiClass::OtherNeutral), // Sm       MULTIPLICATION SIGN
    ('\u{d8}', '\u{f6}', BidiClass::LeftToRight), // L&  [31] LATIN CAPITAL LETTER O WITH STROKE..LATIN SMALL LETTER O WITH DIAERESIS
    ('\u{f7}', '\u{f7}', BidiClass::OtherNeutral), // Sm       DIVISION SIGN
    ('\u{f8}', '\u{1ba}', BidiClass::LeftToRight), // L& [195] LATIN SMALL LETTER O WITH STROKE..LATIN SMALL LETTER EZH WITH TAIL
    ('\u{1bb}', '\u{1bb}', BidiClass::LeftToRight), // Lo       LATIN LETTER TWO WITH STROKE
    ('\u{1bc}', '\u{1bf}', BidiClass::LeftToRight), // L&   [4] LATIN CAPITAL LETTER TONE FIVE..LATIN LETTER WYNN
    ('\u{1c0}', '\u{1c3}', BidiClass::LeftToRight), // Lo   [4] LATIN LETTER DENTAL CLICK..LATIN LETTER RETROFLEX CLICK
    ('\u{1c4}', '\u{293}', BidiClass::LeftToRight), // L& [208] LATIN CAPITAL LETTER DZ WITH CARON..LATIN SMALL LETTER EZH WITH CURL
    ('\u{294}', '\u{294}', BidiClass::LeftToRight), // Lo       LATIN LETTER GLOTTAL STOP
    ('\u{295}', '\u{2af}', BidiClass::LeftToRight), // L&  [27] LATIN LETTER PHARYNGEAL VOICED FRICATIVE..LATIN SMALL LETTER TURNED H WITH FISHHOOK AND TAIL
    ('\u{2b0}', '\u{2b8}', BidiClass::LeftToRight), // Lm   [9] MODIFIER LETTER SMALL H..MODIFIER LETTER SMALL Y
    ('\u{2b9}', '\u{2ba}', BidiClass::OtherNeutral), // Lm   [2] MODIFIER LETTER PRIME..MODIFIER LETTER DOUBLE PRIME
    ('\u{2bb}', '\u{2c1}', BidiClass::LeftToRight), // Lm   [7] MODIFIER LETTER TURNED COMMA..MODIFIER LETTER REVERSED GLOTTAL STOP
    ('\u{2c2}', '\u{2c5}', BidiClass::OtherNeutral), // Sk   [4] MODIFIER LETTER LEFT ARROWHEAD..MODIFIER LETTER DOWN ARROWHEAD
    ('\u{2c6}', '\u{2cf}', BidiClass::OtherNeutral), // Lm  [10] MODIFIER LETTER CIRCUMFLEX ACCENT..MODIFIER LETTER LOW ACUTE ACCENT
    ('\u{2d0}', '\u{2d1}', BidiClass::LeftToRight), // Lm   [2] MODIFIER LETTER TRIANGULAR COLON..MODIFIER LETTER HALF TRIANGULAR COLON
    ('\u{2d2}', '\u{2df}', BidiClass::OtherNeutral), // Sk  [14] MODIFIER LETTER CENTRED RIGHT HALF RING..MODIFIER LETTER CROSS ACCENT
    ('\u{2e0}', '\u{2e4}', BidiClass::LeftToRight), // Lm   [5] MODIFIER LETTER SMALL GAMMA..MODIFIER LETTER SMALL REVERSED GLOTTAL STOP
    ('\u{2e5}', '\u{2eb}', BidiClass::OtherNeutral), // Sk   [7] MODIFIER LETTER EXTRA-HIGH TONE BAR..MODIFIER LETTER YANG DEPARTING TONE MARK
    ('\u{2ec}', '\u{2ec}', BidiClass::OtherNeutral), // Lm       MODIFIER LETTER VOICING
    ('\u{2ed}', '\u{2ed}', BidiClass::OtherNeutral), // Sk       MODIFIER LETTER UNASPIRATED
    ('\u{2ee}', '\u{2ee}', BidiClass::LeftToRight),  // Lm       MODIFIER LETTER DOUBLE APOSTROPHE
    ('\u{2ef}', '\u{2ff}', BidiClass::OtherNeutral), // Sk  [17] MODIFIER LETTER LOW DOWN ARROWHEAD..MODIFIER LETTER LOW LEFT ARROW
    ('\u{300}', '\u{36f}', BidiClass::NonspacingMark), // Mn [112] COMBINING GRAVE ACCENT..COMBINING LATIN SMALL LETTER X
    ('\u{370}', '\u{373}', BidiClass::LeftToRight), // L&   [4] GREEK CAPITAL LETTER HETA..GREEK SMALL LETTER ARCHAIC SAMPI
    ('\u{374}', '\u{374}', BidiClass::OtherNeutral), // Lm       GREEK NUMERAL SIGN
    ('\u{375}', '\u{375}', BidiClass::OtherNeutral), // Sk       GREEK LOWER NUMERAL SIGN
    ('\u{376}', '\u{377}', BidiClass::LeftToRight), // L&   [2] GREEK CAPITAL LETTER PAMPHYLIAN DIGAMMA..GREEK SMALL LETTER PAMPHYLIAN DIGAMMA
    ('\u{37a}', '\u{37a}', BidiClass::LeftToRight), // Lm       GREEK YPOGEGRAMMENI
    ('\u{37b}', '\u{37d}', BidiClass::LeftToRight), // L&   [3] GREEK SMALL REVERSED LUNATE SIGMA SYMBOL..GREEK SMALL REVERSED DOTTED LUNATE SIGMA SYMBOL
    ('\u{37e}', '\u{37e}', BidiClass::OtherNeutral), // Po       GREEK QUESTION MARK
    ('\u{37f}', '\u{37f}', BidiClass::LeftToRight), // L&       GREEK CAPITAL LETTER YOT
    ('\u{384}', '\u{385}', BidiClass::OtherNeutral), // Sk   [2] GREEK TONOS..GREEK DIALYTIKA TONOS
    ('\u{386}', '\u{386}', BidiClass::LeftToRight), // L&       GREEK CAPITAL LETTER ALPHA WITH TONOS
    ('\u{387}', '\u{387}', BidiClass::OtherNeutral), // Po       GREEK ANO TELEIA
    ('\u{388}', '\u{38a}', BidiClass::LeftToRight), // L&   [3] GREEK CAPITAL LETTER EPSILON WITH TONOS..GREEK CAPITAL LETTER IOTA WITH TONOS
    ('\u{38c}', '\u{38c}', BidiClass::LeftToRight), // L&       GREEK CAPITAL LETTER OMICRON WITH TONOS
    ('\u{38e}', '\u{3a1}', BidiClass::LeftToRight), // L&  [20] GREEK CAPITAL LETTER UPSILON WITH TONOS..GREEK CAPITAL LETTER RHO
    ('\u{3a3}', '\u{3f5}', BidiClass::LeftToRight), // L&  [83] GREEK CAPITAL LETTER SIGMA..GREEK LUNATE EPSILON SYMBOL
    ('\u{3f6}', '\u{3f6}', BidiClass::OtherNeutral), // Sm       GREEK REVERSED LUNATE EPSILON SYMBOL
    ('\u{3f7}', '\u{481}', BidiClass::LeftToRight), // L& [139] GREEK CAPITAL LETTER SHO..CYRILLIC SMALL LETTER KOPPA
    ('\u{482}', '\u{482}', BidiClass::LeftToRight), // So       CYRILLIC THOUSANDS SIGN
    ('\u{483}', '\u{487}', BidiClass::NonspacingMark), // Mn   [5] COMBINING CYRILLIC TITLO..COMBINING CYRILLIC POKRYTIE
    ('\u{488}', '\u{489}', BidiClass::NonspacingMark), // Me   [2] COMBINING CYRILLIC HUNDRED THOUSANDS SIGN..COMBINING CYRILLIC MILLIONS SIGN
    ('\u{48a}', '\u{52f}', BidiClass::LeftToRight), // L& [166] CYRILLIC CAPITAL LETTER SHORT I WITH TAIL..CYRILLIC SMALL LETTER EL WITH DESCENDER
    ('\u{531}', '\u{556}', BidiClass::LeftToRight), // L&  [38] ARMENIAN CAPITAL LETTER AYB..ARMENIAN CAPITAL LETTER FEH
    ('\u{559}', '\u{559}', BidiClass::LeftToRight), // Lm       ARMENIAN MODIFIER LETTER LEFT HALF RING
    ('\u{55a}', '\u{55f}', BidiClass::LeftToRight), // Po   [6] ARMENIAN APOSTROPHE..ARMENIAN ABBREVIATION MARK
    ('\u{560}', '\u{588}', BidiClass::LeftToRight), // L&  [41] ARMENIAN SMALL LETTER TURNED AYB..ARMENIAN SMALL LETTER YI WITH STROKE
    ('\u{589}', '\u{589}', BidiClass::LeftToRight), // Po       ARMENIAN FULL STOP
    ('\u{58a}', '\u{58a}', BidiClass::OtherNeutral), // Pd       ARMENIAN HYPHEN
    ('\u{58d}', '\u{58e}', BidiClass::OtherNeutral), // So   [2] RIGHT-FACING ARMENIAN ETERNITY SIGN..LEFT-FACING ARMENIAN ETERNITY SIGN
    ('\u{58f}', '\u{58f}', BidiClass::EuropeanTerminator), // Sc       ARMENIAN DRAM SIGN
    ('\u{590}', '\u{590}', BidiClass::RightToLeft),  // Cn       <reserved-0590>
    ('\u{591}', '\u{5bd}', BidiClass::NonspacingMark), // Mn  [45] HEBREW ACCENT ETNAHTA..HEBREW POINT METEG
    ('\u{5be}', '\u{5be}', BidiClass::RightToLeft),    // Pd       HEBREW PUNCTUATION MAQAF
    ('\u{5bf}', '\u{5bf}', BidiClass::NonspacingMark), // Mn       HEBREW POINT RAFE
    ('\u{5c0}', '\u{5c0}', BidiClass::RightToLeft),    // Po       HEBREW PUNCTUATION PASEQ
    ('\u{5c1}', '\u{5c2}', BidiClass::NonspacingMark), // Mn   [2] HEBREW POINT SHIN DOT..HEBREW POINT SIN DOT
    ('\u{5c3}', '\u{5c3}', BidiClass::RightToLeft),    // Po       HEBREW PUNCTUATION SOF PASUQ
    ('\u{5c4}', '\u{5c5}', BidiClass::NonspacingMark), // Mn   [2] HEBREW MARK UPPER DOT..HEBREW MARK LOWER DOT
    ('\u{5c6}', '\u{5c6}', BidiClass::RightToLeft),    // Po       HEBREW PUNCTUATION NUN HAFUKHA
    ('\u{5c7}', '\u{5c7}', BidiClass::NonspacingMark), // Mn       HEBREW POINT QAMATS QATAN
    ('\u{5c8}', '\u{5cf}', BidiClass::RightToLeft),    // Cn   [8] <reserved-05C8>..<reserved-05CF>
    ('\u{5d0}', '\u{5ea}', BidiClass::RightToLeft), // Lo  [27] HEBREW LETTER ALEF..HEBREW LETTER TAV
    ('\u{5eb}', '\u{5ee}', BidiClass::RightToLeft), // Cn   [4] <reserved-05EB>..<reserved-05EE>
    ('\u{5ef}', '\u{5f2}', BidiClass::RightToLeft), // Lo   [4] HEBREW YOD TRIANGLE..HEBREW LIGATURE YIDDISH DOUBLE YOD
    ('\u{5f3}', '\u{5f4}', BidiClass::RightToLeft), // Po   [2] HEBREW PUNCTUATION GERESH..HEBREW PUNCTUATION GERSHAYIM
    ('\u{5f5}', '\u{5ff}', BidiClass::RightToLeft), // Cn  [11] <reserved-05F5>..<reserved-05FF>
    ('\u{600}', '\u{605}', BidiClass::ArabicNumber), // Cf   [6] ARABIC NUMBER SIGN..ARABIC NUMBER MARK ABOVE
    ('\u{606}', '\u{607}', BidiClass::OtherNeutral), // Sm   [2] ARABIC-INDIC CUBE ROOT..ARABIC-INDIC FOURTH ROOT
    ('\u{608}', '\u{608}', BidiClass::ArabicLetter), // Sm       ARABIC RAY
    ('\u{609}', '\u{60a}', BidiClass::EuropeanTerminator), // Po   [2] ARABIC-INDIC PER MILLE SIGN..ARABIC-INDIC PER TEN THOUSAND SIGN
    ('\u{60b}', '\u{60b}', BidiClass::ArabicLetter),       // Sc       AFGHANI SIGN
    ('\u{60c}', '\u{60c}', BidiClass::CommonSeparator),    // Po       ARABIC COMMA
    ('\u{60d}', '\u{60d}', BidiClass::ArabicLetter),       // Po       ARABIC DATE SEPARATOR
    ('\u{60e}', '\u{60f}', BidiClass::OtherNeutral), // So   [2] ARABIC POETIC VERSE SIGN..ARABIC SIGN MISRA
    ('\u{610}', '\u{61a}', BidiClass::NonspacingMark), // Mn  [11] ARABIC SIGN SALLALLAHOU ALAYHE WASSALLAM..ARABIC SMALL KASRA
    ('\u{61b}', '\u{61b}', BidiClass::ArabicLetter),   // Po       ARABIC SEMICOLON
    ('\u{61c}', '\u{61c}', BidiClass::ArabicLetter),   // Cf       ARABIC LETTER MARK
    ('\u{61d}', '\u{61f}', BidiClass::ArabicLetter), // Po   [3] ARABIC END OF TEXT MARK..ARABIC QUESTION MARK
    ('\u{620}', '\u{63f}', BidiClass::ArabicLetter), // Lo  [32] ARABIC LETTER KASHMIRI YEH..ARABIC LETTER FARSI YEH WITH THREE DOTS ABOVE
    ('\u{640}', '\u{640}', BidiClass::ArabicLetter), // Lm       ARABIC TATWEEL
    ('\u{641}', '\u{64a}', BidiClass::ArabicLetter), // Lo  [10] ARABIC LETTER FEH..ARABIC LETTER YEH
    ('\u{64b}', '\u{65f}', BidiClass::NonspacingMark), // Mn  [21] ARABIC FATHATAN..ARABIC WAVY HAMZA BELOW
    ('\u{660}', '\u{669}', BidiClass::ArabicNumber), // Nd  [10] ARABIC-INDIC DIGIT ZERO..ARABIC-INDIC DIGIT NINE
    ('\u{66a}', '\u{66a}', BidiClass::EuropeanTerminator), // Po       ARABIC PERCENT SIGN
    ('\u{66b}', '\u{66c}', BidiClass::ArabicNumber), // Po   [2] ARABIC DECIMAL SEPARATOR..ARABIC THOUSANDS SEPARATOR
    ('\u{66d}', '\u{66d}', BidiClass::ArabicLetter), // Po       ARABIC FIVE POINTED STAR
    ('\u{66e}', '\u{66f}', BidiClass::ArabicLetter), // Lo   [2] ARABIC LETTER DOTLESS BEH..ARABIC LETTER DOTLESS QAF
    ('\u{670}', '\u{670}', BidiClass::NonspacingMark), // Mn       ARABIC LETTER SUPERSCRIPT ALEF
    ('\u{671}', '\u{6d3}', BidiClass::ArabicLetter), // Lo  [99] ARABIC LETTER ALEF WASLA..ARABIC LETTER YEH BARREE WITH HAMZA ABOVE
    ('\u{6d4}', '\u{6d4}', BidiClass::ArabicLetter), // Po       ARABIC FULL STOP
    ('\u{6d5}', '\u{6d5}', BidiClass::ArabicLetter), // Lo       ARABIC LETTER AE
    ('\u{6d6}', '\u{6dc}', BidiClass::NonspacingMark), // Mn   [7] ARABIC SMALL HIGH LIGATURE SAD WITH LAM WITH ALEF MAKSURA..ARABIC SMALL HIGH SEEN
    ('\u{6dd}', '\u{6dd}', BidiClass::ArabicNumber),   // Cf       ARABIC END OF AYAH
    ('\u{6de}', '\u{6de}', BidiClass::OtherNeutral),   // So       ARABIC START OF RUB EL HIZB
    ('\u{6df}', '\u{6e4}', BidiClass::NonspacingMark), // Mn   [6] ARABIC SMALL HIGH ROUNDED ZERO..ARABIC SMALL HIGH MADDA
    ('\u{6e5}', '\u{6e6}', BidiClass::ArabicLetter), // Lm   [2] ARABIC SMALL WAW..ARABIC SMALL YEH
    ('\u{6e7}', '\u{6e8}', BidiClass::NonspacingMark), // Mn   [2] ARABIC SMALL HIGH YEH..ARABIC SMALL HIGH NOON
    ('\u{6e9}', '\u{6e9}', BidiClass::OtherNeutral),   // So       ARABIC PLACE OF SAJDAH
    ('\u{6ea}', '\u{6ed}', BidiClass::NonspacingMark), // Mn   [4] ARABIC EMPTY CENTRE LOW STOP..ARABIC SMALL LOW MEEM
    ('\u{6ee}', '\u{6ef}', BidiClass::ArabicLetter), // Lo   [2] ARABIC LETTER DAL WITH INVERTED V..ARABIC LETTER REH WITH INVERTED V
    ('\u{6f0}', '\u{6f9}', BidiClass::EuropeanNumber), // Nd  [10] EXTENDED ARABIC-INDIC DIGIT ZERO..EXTENDED ARABIC-INDIC DIGIT NINE
    ('\u{6fa}', '\u{6fc}', BidiClass::ArabicLetter), // Lo   [3] ARABIC LETTER SHEEN WITH DOT BELOW..ARABIC LETTER GHAIN WITH DOT BELOW
    ('\u{6fd}', '\u{6fe}', BidiClass::ArabicLetter), // So   [2] ARABIC SIGN SINDHI AMPERSAND..ARABIC SIGN SINDHI POSTPOSITION MEN
    ('\u{6ff}', '\u{6ff}', BidiClass::ArabicLetter), // Lo       ARABIC LETTER HEH WITH INVERTED V
    ('\u{700}', '\u{70d}', BidiClass::ArabicLetter), // Po  [14] SYRIAC END OF PARAGRAPH..SYRIAC HARKLEAN ASTERISCUS
    ('\u{70e}', '\u{70e}', BidiClass::ArabicLetter), // Cn       <reserved-070E>
    ('\u{70f}', '\u{70f}', BidiClass::ArabicLetter), // Cf       SYRIAC ABBREVIATION MARK
    ('\u{710}', '\u{710}', BidiClass::ArabicLetter), // Lo       SYRIAC LETTER ALAPH
    ('\u{711}', '\u{711}', BidiClass::NonspacingMark), // Mn       SYRIAC LETTER SUPERSCRIPT ALAPH
    ('\u{712}', '\u{72f}', BidiClass::ArabicLetter), // Lo  [30] SYRIAC LETTER BETH..SYRIAC LETTER PERSIAN DHALATH
    ('\u{730}', '\u{74a}', BidiClass::NonspacingMark), // Mn  [27] SYRIAC PTHAHA ABOVE..SYRIAC BARREKH
    ('\u{74b}', '\u{74c}', BidiClass::ArabicLetter),   // Cn   [2] <reserved-074B>..<reserved-074C>
    ('\u{74d}', '\u{7a5}', BidiClass::ArabicLetter), // Lo  [89] SYRIAC LETTER SOGDIAN ZHAIN..THAANA LETTER WAAVU
    ('\u{7a6}', '\u{7b0}', BidiClass::NonspacingMark), // Mn  [11] THAANA ABAFILI..THAANA SUKUN
    ('\u{7b1}', '\u{7b1}', BidiClass::ArabicLetter), // Lo       THAANA LETTER NAA
    ('\u{7b2}', '\u{7bf}', BidiClass::ArabicLetter), // Cn  [14] <reserved-07B2>..<reserved-07BF>
    ('\u{7c0}', '\u{7c9}', BidiClass::RightToLeft),  // Nd  [10] NKO DIGIT ZERO..NKO DIGIT NINE
    ('\u{7ca}', '\u{7ea}', BidiClass::RightToLeft),  // Lo  [33] NKO LETTER A..NKO LETTER JONA RA
    ('\u{7eb}', '\u{7f3}', BidiClass::NonspacingMark), // Mn   [9] NKO COMBINING SHORT HIGH TONE..NKO COMBINING DOUBLE DOT ABOVE
    ('\u{7f4}', '\u{7f5}', BidiClass::RightToLeft), // Lm   [2] NKO HIGH TONE APOSTROPHE..NKO LOW TONE APOSTROPHE
    ('\u{7f6}', '\u{7f6}', BidiClass::OtherNeutral), // So       NKO SYMBOL OO DENNEN
    ('\u{7f7}', '\u{7f9}', BidiClass::OtherNeutral), // Po   [3] NKO SYMBOL GBAKURUNEN..NKO EXCLAMATION MARK
    ('\u{7fa}', '\u{7fa}', BidiClass::RightToLeft),  // Lm       NKO LAJANYALAN
    ('\u{7fb}', '\u{7fc}', BidiClass::RightToLeft),  // Cn   [2] <reserved-07FB>..<reserved-07FC>
    ('\u{7fd}', '\u{7fd}', BidiClass::NonspacingMark), // Mn       NKO DANTAYALAN
    ('\u{7fe}', '\u{7ff}', BidiClass::RightToLeft),  // Sc   [2] NKO DOROME SIGN..NKO TAMAN SIGN
    ('\u{800}', '\u{815}', BidiClass::RightToLeft), // Lo  [22] SAMARITAN LETTER ALAF..SAMARITAN LETTER TAAF
    ('\u{816}', '\u{819}', BidiClass::NonspacingMark), // Mn   [4] SAMARITAN MARK IN..SAMARITAN MARK DAGESH
    ('\u{81a}', '\u{81a}', BidiClass::RightToLeft), // Lm       SAMARITAN MODIFIER LETTER EPENTHETIC YUT
    ('\u{81b}', '\u{823}', BidiClass::NonspacingMark), // Mn   [9] SAMARITAN MARK EPENTHETIC YUT..SAMARITAN VOWEL SIGN A
    ('\u{824}', '\u{824}', BidiClass::RightToLeft),    // Lm       SAMARITAN MODIFIER LETTER SHORT A
    ('\u{825}', '\u{827}', BidiClass::NonspacingMark), // Mn   [3] SAMARITAN VOWEL SIGN SHORT A..SAMARITAN VOWEL SIGN U
    ('\u{828}', '\u{828}', BidiClass::RightToLeft),    // Lm       SAMARITAN MODIFIER LETTER I
    ('\u{829}', '\u{82d}', BidiClass::NonspacingMark), // Mn   [5] SAMARITAN VOWEL SIGN LONG I..SAMARITAN MARK NEQUDAA
    ('\u{82e}', '\u{82f}', BidiClass::RightToLeft),    // Cn   [2] <reserved-082E>..<reserved-082F>
    ('\u{830}', '\u{83e}', BidiClass::RightToLeft), // Po  [15] SAMARITAN PUNCTUATION NEQUDAA..SAMARITAN PUNCTUATION ANNAAU
    ('\u{83f}', '\u{83f}', BidiClass::RightToLeft), // Cn       <reserved-083F>
    ('\u{840}', '\u{858}', BidiClass::RightToLeft), // Lo  [25] MANDAIC LETTER HALQA..MANDAIC LETTER AIN
    ('\u{859}', '\u{85b}', BidiClass::NonspacingMark), // Mn   [3] MANDAIC AFFRICATION MARK..MANDAIC GEMINATION MARK
    ('\u{85c}', '\u{85d}', BidiClass::RightToLeft),    // Cn   [2] <reserved-085C>..<reserved-085D>
    ('\u{85e}', '\u{85e}', BidiClass::RightToLeft),    // Po       MANDAIC PUNCTUATION
    ('\u{85f}', '\u{85f}', BidiClass::RightToLeft),    // Cn       <reserved-085F>
    ('\u{860}', '\u{86a}', BidiClass::ArabicLetter), // Lo  [11] SYRIAC LETTER MALAYALAM NGA..SYRIAC LETTER MALAYALAM SSA
    ('\u{86b}', '\u{86f}', BidiClass::ArabicLetter), // Cn   [5] <reserved-086B>..<reserved-086F>
    ('\u{870}', '\u{887}', BidiClass::ArabicLetter), // Lo  [24] ARABIC LETTER ALEF WITH ATTACHED FATHA..ARABIC BASELINE ROUND DOT
    ('\u{888}', '\u{888}', BidiClass::ArabicLetter), // Sk       ARABIC RAISED ROUND DOT
    ('\u{889}', '\u{88e}', BidiClass::ArabicLetter), // Lo   [6] ARABIC LETTER NOON WITH INVERTED SMALL V..ARABIC VERTICAL TAIL
    ('\u{88f}', '\u{88f}', BidiClass::ArabicLetter), // Cn       <reserved-088F>
    ('\u{890}', '\u{891}', BidiClass::ArabicNumber), // Cf   [2] ARABIC POUND MARK ABOVE..ARABIC PIASTRE MARK ABOVE
    ('\u{892}', '\u{897}', BidiClass::ArabicLetter), // Cn   [6] <reserved-0892>..<reserved-0897>
    ('\u{898}', '\u{89f}', BidiClass::NonspacingMark), // Mn   [8] ARABIC SMALL HIGH WORD AL-JUZ..ARABIC HALF MADDA OVER MADDA
    ('\u{8a0}', '\u{8c8}', BidiClass::ArabicLetter), // Lo  [41] ARABIC LETTER BEH WITH SMALL V BELOW..ARABIC LETTER GRAF
    ('\u{8c9}', '\u{8c9}', BidiClass::ArabicLetter), // Lm       ARABIC SMALL FARSI YEH
    ('\u{8ca}', '\u{8e1}', BidiClass::NonspacingMark), // Mn  [24] ARABIC SMALL HIGH FARSI YEH..ARABIC SMALL HIGH SIGN SAFHA
    ('\u{8e2}', '\u{8e2}', BidiClass::ArabicNumber),   // Cf       ARABIC DISPUTED END OF AYAH
    ('\u{8e3}', '\u{902}', BidiClass::NonspacingMark), // Mn  [32] ARABIC TURNED DAMMA BELOW..DEVANAGARI SIGN ANUSVARA
    ('\u{903}', '\u{903}', BidiClass::LeftToRight),    // Mc       DEVANAGARI SIGN VISARGA
    ('\u{904}', '\u{939}', BidiClass::LeftToRight), // Lo  [54] DEVANAGARI LETTER SHORT A..DEVANAGARI LETTER HA
    ('\u{93a}', '\u{93a}', BidiClass::NonspacingMark), // Mn       DEVANAGARI VOWEL SIGN OE
    ('\u{93b}', '\u{93b}', BidiClass::LeftToRight), // Mc       DEVANAGARI VOWEL SIGN OOE
    ('\u{93c}', '\u{93c}', BidiClass::NonspacingMark), // Mn       DEVANAGARI SIGN NUKTA
    ('\u{93d}', '\u{93d}', BidiClass::LeftToRight), // Lo       DEVANAGARI SIGN AVAGRAHA
    ('\u{93e}', '\u{940}', BidiClass::LeftToRight), // Mc   [3] DEVANAGARI VOWEL SIGN AA..DEVANAGARI VOWEL SIGN II
    ('\u{941}', '\u{948}', BidiClass::NonspacingMark), // Mn   [8] DEVANAGARI VOWEL SIGN U..DEVANAGARI VOWEL SIGN AI
    ('\u{949}', '\u{94c}', BidiClass::LeftToRight), // Mc   [4] DEVANAGARI VOWEL SIGN CANDRA O..DEVANAGARI VOWEL SIGN AU
    ('\u{94d}', '\u{94d}', BidiClass::NonspacingMark), // Mn       DEVANAGARI SIGN VIRAMA
    ('\u{94e}', '\u{94f}', BidiClass::LeftToRight), // Mc   [2] DEVANAGARI VOWEL SIGN PRISHTHAMATRA E..DEVANAGARI VOWEL SIGN AW
    ('\u{950}', '\u{950}', BidiClass::LeftToRight), // Lo       DEVANAGARI OM
    ('\u{951}', '\u{957}', BidiClass::NonspacingMark), // Mn   [7] DEVANAGARI STRESS SIGN UDATTA..DEVANAGARI VOWEL SIGN UUE
    ('\u{958}', '\u{961}', BidiClass::LeftToRight), // Lo  [10] DEVANAGARI LETTER QA..DEVANAGARI LETTER VOCALIC LL
    ('\u{962}', '\u{963}', BidiClass::NonspacingMark), // Mn   [2] DEVANAGARI VOWEL SIGN VOCALIC L..DEVANAGARI VOWEL SIGN VOCALIC LL
    ('\u{964}', '\u{965}', BidiClass::LeftToRight), // Po   [2] DEVANAGARI DANDA..DEVANAGARI DOUBLE DANDA
    ('\u{966}', '\u{96f}', BidiClass::LeftToRight), // Nd  [10] DEVANAGARI DIGIT ZERO..DEVANAGARI DIGIT NINE
    ('\u{970}', '\u{970}', BidiClass::LeftToRight), // Po       DEVANAGARI ABBREVIATION SIGN
    ('\u{971}', '\u{971}', BidiClass::LeftToRight), // Lm       DEVANAGARI SIGN HIGH SPACING DOT
    ('\u{972}', '\u{980}', BidiClass::LeftToRight), // Lo  [15] DEVANAGARI LETTER CANDRA A..BENGALI ANJI
    ('\u{981}', '\u{981}', BidiClass::NonspacingMark), // Mn       BENGALI SIGN CANDRABINDU
    ('\u{982}', '\u{983}', BidiClass::LeftToRight), // Mc   [2] BENGALI SIGN ANUSVARA..BENGALI SIGN VISARGA
    ('\u{985}', '\u{98c}', BidiClass::LeftToRight), // Lo   [8] BENGALI LETTER A..BENGALI LETTER VOCALIC L
    ('\u{98f}', '\u{990}', BidiClass::LeftToRight), // Lo   [2] BENGALI LETTER E..BENGALI LETTER AI
    ('\u{993}', '\u{9a8}', BidiClass::LeftToRight), // Lo  [22] BENGALI LETTER O..BENGALI LETTER NA
    ('\u{9aa}', '\u{9b0}', BidiClass::LeftToRight), // Lo   [7] BENGALI LETTER PA..BENGALI LETTER RA
    ('\u{9b2}', '\u{9b2}', BidiClass::LeftToRight), // Lo       BENGALI LETTER LA
    ('\u{9b6}', '\u{9b9}', BidiClass::LeftToRight), // Lo   [4] BENGALI LETTER SHA..BENGALI LETTER HA
    ('\u{9bc}', '\u{9bc}', BidiClass::NonspacingMark), // Mn       BENGALI SIGN NUKTA
    ('\u{9bd}', '\u{9bd}', BidiClass::LeftToRight), // Lo       BENGALI SIGN AVAGRAHA
    ('\u{9be}', '\u{9c0}', BidiClass::LeftToRight), // Mc   [3] BENGALI VOWEL SIGN AA..BENGALI VOWEL SIGN II
    ('\u{9c1}', '\u{9c4}', BidiClass::NonspacingMark), // Mn   [4] BENGALI VOWEL SIGN U..BENGALI VOWEL SIGN VOCALIC RR
    ('\u{9c7}', '\u{9c8}', BidiClass::LeftToRight), // Mc   [2] BENGALI VOWEL SIGN E..BENGALI VOWEL SIGN AI
    ('\u{9cb}', '\u{9cc}', BidiClass::LeftToRight), // Mc   [2] BENGALI VOWEL SIGN O..BENGALI VOWEL SIGN AU
    ('\u{9cd}', '\u{9cd}', BidiClass::NonspacingMark), // Mn       BENGALI SIGN VIRAMA
    ('\u{9ce}', '\u{9ce}', BidiClass::LeftToRight), // Lo       BENGALI LETTER KHANDA TA
    ('\u{9d7}', '\u{9d7}', BidiClass::LeftToRight), // Mc       BENGALI AU LENGTH MARK
    ('\u{9dc}', '\u{9dd}', BidiClass::LeftToRight), // Lo   [2] BENGALI LETTER RRA..BENGALI LETTER RHA
    ('\u{9df}', '\u{9e1}', BidiClass::LeftToRight), // Lo   [3] BENGALI LETTER YYA..BENGALI LETTER VOCALIC LL
    ('\u{9e2}', '\u{9e3}', BidiClass::NonspacingMark), // Mn   [2] BENGALI VOWEL SIGN VOCALIC L..BENGALI VOWEL SIGN VOCALIC LL
    ('\u{9e6}', '\u{9ef}', BidiClass::LeftToRight), // Nd  [10] BENGALI DIGIT ZERO..BENGALI DIGIT NINE
    ('\u{9f0}', '\u{9f1}', BidiClass::LeftToRight), // Lo   [2] BENGALI LETTER RA WITH MIDDLE DIAGONAL..BENGALI LETTER RA WITH LOWER DIAGONAL
    ('\u{9f2}', '\u{9f3}', BidiClass::EuropeanTerminator), // Sc   [2] BENGALI RUPEE MARK..BENGALI RUPEE SIGN
    ('\u{9f4}', '\u{9f9}', BidiClass::LeftToRight), // No   [6] BENGALI CURRENCY NUMERATOR ONE..BENGALI CURRENCY DENOMINATOR SIXTEEN
    ('\u{9fa}', '\u{9fa}', BidiClass::LeftToRight), // So       BENGALI ISSHAR
    ('\u{9fb}', '\u{9fb}', BidiClass::EuropeanTerminator), // Sc       BENGALI GANDA MARK
    ('\u{9fc}', '\u{9fc}', BidiClass::LeftToRight), // Lo       BENGALI LETTER VEDIC ANUSVARA
    ('\u{9fd}', '\u{9fd}', BidiClass::LeftToRight), // Po       BENGALI ABBREVIATION SIGN
    ('\u{9fe}', '\u{9fe}', BidiClass::NonspacingMark), // Mn       BENGALI SANDHI MARK
    ('\u{a01}', '\u{a02}', BidiClass::NonspacingMark), // Mn   [2] GURMUKHI SIGN ADAK BINDI..GURMUKHI SIGN BINDI
    ('\u{a03}', '\u{a03}', BidiClass::LeftToRight),    // Mc       GURMUKHI SIGN VISARGA
    ('\u{a05}', '\u{a0a}', BidiClass::LeftToRight), // Lo   [6] GURMUKHI LETTER A..GURMUKHI LETTER UU
    ('\u{a0f}', '\u{a10}', BidiClass::LeftToRight), // Lo   [2] GURMUKHI LETTER EE..GURMUKHI LETTER AI
    ('\u{a13}', '\u{a28}', BidiClass::LeftToRight), // Lo  [22] GURMUKHI LETTER OO..GURMUKHI LETTER NA
    ('\u{a2a}', '\u{a30}', BidiClass::LeftToRight), // Lo   [7] GURMUKHI LETTER PA..GURMUKHI LETTER RA
    ('\u{a32}', '\u{a33}', BidiClass::LeftToRight), // Lo   [2] GURMUKHI LETTER LA..GURMUKHI LETTER LLA
    ('\u{a35}', '\u{a36}', BidiClass::LeftToRight), // Lo   [2] GURMUKHI LETTER VA..GURMUKHI LETTER SHA
    ('\u{a38}', '\u{a39}', BidiClass::LeftToRight), // Lo   [2] GURMUKHI LETTER SA..GURMUKHI LETTER HA
    ('\u{a3c}', '\u{a3c}', BidiClass::NonspacingMark), // Mn       GURMUKHI SIGN NUKTA
    ('\u{a3e}', '\u{a40}', BidiClass::LeftToRight), // Mc   [3] GURMUKHI VOWEL SIGN AA..GURMUKHI VOWEL SIGN II
    ('\u{a41}', '\u{a42}', BidiClass::NonspacingMark), // Mn   [2] GURMUKHI VOWEL SIGN U..GURMUKHI VOWEL SIGN UU
    ('\u{a47}', '\u{a48}', BidiClass::NonspacingMark), // Mn   [2] GURMUKHI VOWEL SIGN EE..GURMUKHI VOWEL SIGN AI
    ('\u{a4b}', '\u{a4d}', BidiClass::NonspacingMark), // Mn   [3] GURMUKHI VOWEL SIGN OO..GURMUKHI SIGN VIRAMA
    ('\u{a51}', '\u{a51}', BidiClass::NonspacingMark), // Mn       GURMUKHI SIGN UDAAT
    ('\u{a59}', '\u{a5c}', BidiClass::LeftToRight), // Lo   [4] GURMUKHI LETTER KHHA..GURMUKHI LETTER RRA
    ('\u{a5e}', '\u{a5e}', BidiClass::LeftToRight), // Lo       GURMUKHI LETTER FA
    ('\u{a66}', '\u{a6f}', BidiClass::LeftToRight), // Nd  [10] GURMUKHI DIGIT ZERO..GURMUKHI DIGIT NINE
    ('\u{a70}', '\u{a71}', BidiClass::NonspacingMark), // Mn   [2] GURMUKHI TIPPI..GURMUKHI ADDAK
    ('\u{a72}', '\u{a74}', BidiClass::LeftToRight), // Lo   [3] GURMUKHI IRI..GURMUKHI EK ONKAR
    ('\u{a75}', '\u{a75}', BidiClass::NonspacingMark), // Mn       GURMUKHI SIGN YAKASH
    ('\u{a76}', '\u{a76}', BidiClass::LeftToRight), // Po       GURMUKHI ABBREVIATION SIGN
    ('\u{a81}', '\u{a82}', BidiClass::NonspacingMark), // Mn   [2] GUJARATI SIGN CANDRABINDU..GUJARATI SIGN ANUSVARA
    ('\u{a83}', '\u{a83}', BidiClass::LeftToRight),    // Mc       GUJARATI SIGN VISARGA
    ('\u{a85}', '\u{a8d}', BidiClass::LeftToRight), // Lo   [9] GUJARATI LETTER A..GUJARATI VOWEL CANDRA E
    ('\u{a8f}', '\u{a91}', BidiClass::LeftToRight), // Lo   [3] GUJARATI LETTER E..GUJARATI VOWEL CANDRA O
    ('\u{a93}', '\u{aa8}', BidiClass::LeftToRight), // Lo  [22] GUJARATI LETTER O..GUJARATI LETTER NA
    ('\u{aaa}', '\u{ab0}', BidiClass::LeftToRight), // Lo   [7] GUJARATI LETTER PA..GUJARATI LETTER RA
    ('\u{ab2}', '\u{ab3}', BidiClass::LeftToRight), // Lo   [2] GUJARATI LETTER LA..GUJARATI LETTER LLA
    ('\u{ab5}', '\u{ab9}', BidiClass::LeftToRight), // Lo   [5] GUJARATI LETTER VA..GUJARATI LETTER HA
    ('\u{abc}', '\u{abc}', BidiClass::NonspacingMark), // Mn       GUJARATI SIGN NUKTA
    ('\u{abd}', '\u{abd}', BidiClass::LeftToRight), // Lo       GUJARATI SIGN AVAGRAHA
    ('\u{abe}', '\u{ac0}', BidiClass::LeftToRight), // Mc   [3] GUJARATI VOWEL SIGN AA..GUJARATI VOWEL SIGN II
    ('\u{ac1}', '\u{ac5}', BidiClass::NonspacingMark), // Mn   [5] GUJARATI VOWEL SIGN U..GUJARATI VOWEL SIGN CANDRA E
    ('\u{ac7}', '\u{ac8}', BidiClass::NonspacingMark), // Mn   [2] GUJARATI VOWEL SIGN E..GUJARATI VOWEL SIGN AI
    ('\u{ac9}', '\u{ac9}', BidiClass::LeftToRight),    // Mc       GUJARATI VOWEL SIGN CANDRA O
    ('\u{acb}', '\u{acc}', BidiClass::LeftToRight), // Mc   [2] GUJARATI VOWEL SIGN O..GUJARATI VOWEL SIGN AU
    ('\u{acd}', '\u{acd}', BidiClass::NonspacingMark), // Mn       GUJARATI SIGN VIRAMA
    ('\u{ad0}', '\u{ad0}', BidiClass::LeftToRight), // Lo       GUJARATI OM
    ('\u{ae0}', '\u{ae1}', BidiClass::LeftToRight), // Lo   [2] GUJARATI LETTER VOCALIC RR..GUJARATI LETTER VOCALIC LL
    ('\u{ae2}', '\u{ae3}', BidiClass::NonspacingMark), // Mn   [2] GUJARATI VOWEL SIGN VOCALIC L..GUJARATI VOWEL SIGN VOCALIC LL
    ('\u{ae6}', '\u{aef}', BidiClass::LeftToRight), // Nd  [10] GUJARATI DIGIT ZERO..GUJARATI DIGIT NINE
    ('\u{af0}', '\u{af0}', BidiClass::LeftToRight), // Po       GUJARATI ABBREVIATION SIGN
    ('\u{af1}', '\u{af1}', BidiClass::EuropeanTerminator), // Sc       GUJARATI RUPEE SIGN
    ('\u{af9}', '\u{af9}', BidiClass::LeftToRight), // Lo       GUJARATI LETTER ZHA
    ('\u{afa}', '\u{aff}', BidiClass::NonspacingMark), // Mn   [6] GUJARATI SIGN SUKUN..GUJARATI SIGN TWO-CIRCLE NUKTA ABOVE
    ('\u{b01}', '\u{b01}', BidiClass::NonspacingMark), // Mn       ORIYA SIGN CANDRABINDU
    ('\u{b02}', '\u{b03}', BidiClass::LeftToRight), // Mc   [2] ORIYA SIGN ANUSVARA..ORIYA SIGN VISARGA
    ('\u{b05}', '\u{b0c}', BidiClass::LeftToRight), // Lo   [8] ORIYA LETTER A..ORIYA LETTER VOCALIC L
    ('\u{b0f}', '\u{b10}', BidiClass::LeftToRight), // Lo   [2] ORIYA LETTER E..ORIYA LETTER AI
    ('\u{b13}', '\u{b28}', BidiClass::LeftToRight), // Lo  [22] ORIYA LETTER O..ORIYA LETTER NA
    ('\u{b2a}', '\u{b30}', BidiClass::LeftToRight), // Lo   [7] ORIYA LETTER PA..ORIYA LETTER RA
    ('\u{b32}', '\u{b33}', BidiClass::LeftToRight), // Lo   [2] ORIYA LETTER LA..ORIYA LETTER LLA
    ('\u{b35}', '\u{b39}', BidiClass::LeftToRight), // Lo   [5] ORIYA LETTER VA..ORIYA LETTER HA
    ('\u{b3c}', '\u{b3c}', BidiClass::NonspacingMark), // Mn       ORIYA SIGN NUKTA
    ('\u{b3d}', '\u{b3d}', BidiClass::LeftToRight), // Lo       ORIYA SIGN AVAGRAHA
    ('\u{b3e}', '\u{b3e}', BidiClass::LeftToRight), // Mc       ORIYA VOWEL SIGN AA
    ('\u{b3f}', '\u{b3f}', BidiClass::NonspacingMark), // Mn       ORIYA VOWEL SIGN I
    ('\u{b40}', '\u{b40}', BidiClass::LeftToRight), // Mc       ORIYA VOWEL SIGN II
    ('\u{b41}', '\u{b44}', BidiClass::NonspacingMark), // Mn   [4] ORIYA VOWEL SIGN U..ORIYA VOWEL SIGN VOCALIC RR
    ('\u{b47}', '\u{b48}', BidiClass::LeftToRight), // Mc   [2] ORIYA VOWEL SIGN E..ORIYA VOWEL SIGN AI
    ('\u{b4b}', '\u{b4c}', BidiClass::LeftToRight), // Mc   [2] ORIYA VOWEL SIGN O..ORIYA VOWEL SIGN AU
    ('\u{b4d}', '\u{b4d}', BidiClass::NonspacingMark), // Mn       ORIYA SIGN VIRAMA
    ('\u{b55}', '\u{b56}', BidiClass::NonspacingMark), // Mn   [2] ORIYA SIGN OVERLINE..ORIYA AI LENGTH MARK
    ('\u{b57}', '\u{b57}', BidiClass::LeftToRight),    // Mc       ORIYA AU LENGTH MARK
    ('\u{b5c}', '\u{b5d}', BidiClass::LeftToRight), // Lo   [2] ORIYA LETTER RRA..ORIYA LETTER RHA
    ('\u{b5f}', '\u{b61}', BidiClass::LeftToRight), // Lo   [3] ORIYA LETTER YYA..ORIYA LETTER VOCALIC LL
    ('\u{b62}', '\u{b63}', BidiClass::NonspacingMark), // Mn   [2] ORIYA VOWEL SIGN VOCALIC L..ORIYA VOWEL SIGN VOCALIC LL
    ('\u{b66}', '\u{b6f}', BidiClass::LeftToRight), // Nd  [10] ORIYA DIGIT ZERO..ORIYA DIGIT NINE
    ('\u{b70}', '\u{b70}', BidiClass::LeftToRight), // So       ORIYA ISSHAR
    ('\u{b71}', '\u{b71}', BidiClass::LeftToRight), // Lo       ORIYA LETTER WA
    ('\u{b72}', '\u{b77}', BidiClass::LeftToRight), // No   [6] ORIYA FRACTION ONE QUARTER..ORIYA FRACTION THREE SIXTEENTHS
    ('\u{b82}', '\u{b82}', BidiClass::NonspacingMark), // Mn       TAMIL SIGN ANUSVARA
    ('\u{b83}', '\u{b83}', BidiClass::LeftToRight), // Lo       TAMIL SIGN VISARGA
    ('\u{b85}', '\u{b8a}', BidiClass::LeftToRight), // Lo   [6] TAMIL LETTER A..TAMIL LETTER UU
    ('\u{b8e}', '\u{b90}', BidiClass::LeftToRight), // Lo   [3] TAMIL LETTER E..TAMIL LETTER AI
    ('\u{b92}', '\u{b95}', BidiClass::LeftToRight), // Lo   [4] TAMIL LETTER O..TAMIL LETTER KA
    ('\u{b99}', '\u{b9a}', BidiClass::LeftToRight), // Lo   [2] TAMIL LETTER NGA..TAMIL LETTER CA
    ('\u{b9c}', '\u{b9c}', BidiClass::LeftToRight), // Lo       TAMIL LETTER JA
    ('\u{b9e}', '\u{b9f}', BidiClass::LeftToRight), // Lo   [2] TAMIL LETTER NYA..TAMIL LETTER TTA
    ('\u{ba3}', '\u{ba4}', BidiClass::LeftToRight), // Lo   [2] TAMIL LETTER NNA..TAMIL LETTER TA
    ('\u{ba8}', '\u{baa}', BidiClass::LeftToRight), // Lo   [3] TAMIL LETTER NA..TAMIL LETTER PA
    ('\u{bae}', '\u{bb9}', BidiClass::LeftToRight), // Lo  [12] TAMIL LETTER MA..TAMIL LETTER HA
    ('\u{bbe}', '\u{bbf}', BidiClass::LeftToRight), // Mc   [2] TAMIL VOWEL SIGN AA..TAMIL VOWEL SIGN I
    ('\u{bc0}', '\u{bc0}', BidiClass::NonspacingMark), // Mn       TAMIL VOWEL SIGN II
    ('\u{bc1}', '\u{bc2}', BidiClass::LeftToRight), // Mc   [2] TAMIL VOWEL SIGN U..TAMIL VOWEL SIGN UU
    ('\u{bc6}', '\u{bc8}', BidiClass::LeftToRight), // Mc   [3] TAMIL VOWEL SIGN E..TAMIL VOWEL SIGN AI
    ('\u{bca}', '\u{bcc}', BidiClass::LeftToRight), // Mc   [3] TAMIL VOWEL SIGN O..TAMIL VOWEL SIGN AU
    ('\u{bcd}', '\u{bcd}', BidiClass::NonspacingMark), // Mn       TAMIL SIGN VIRAMA
    ('\u{bd0}', '\u{bd0}', BidiClass::LeftToRight), // Lo       TAMIL OM
    ('\u{bd7}', '\u{bd7}', BidiClass::LeftToRight), // Mc       TAMIL AU LENGTH MARK
    ('\u{be6}', '\u{bef}', BidiClass::LeftToRight), // Nd  [10] TAMIL DIGIT ZERO..TAMIL DIGIT NINE
    ('\u{bf0}', '\u{bf2}', BidiClass::LeftToRight), // No   [3] TAMIL NUMBER TEN..TAMIL NUMBER ONE THOUSAND
    ('\u{bf3}', '\u{bf8}', BidiClass::OtherNeutral), // So   [6] TAMIL DAY SIGN..TAMIL AS ABOVE SIGN
    ('\u{bf9}', '\u{bf9}', BidiClass::EuropeanTerminator), // Sc       TAMIL RUPEE SIGN
    ('\u{bfa}', '\u{bfa}', BidiClass::OtherNeutral), // So       TAMIL NUMBER SIGN
    ('\u{c00}', '\u{c00}', BidiClass::NonspacingMark), // Mn       TELUGU SIGN COMBINING CANDRABINDU ABOVE
    ('\u{c01}', '\u{c03}', BidiClass::LeftToRight), // Mc   [3] TELUGU SIGN CANDRABINDU..TELUGU SIGN VISARGA
    ('\u{c04}', '\u{c04}', BidiClass::NonspacingMark), // Mn       TELUGU SIGN COMBINING ANUSVARA ABOVE
    ('\u{c05}', '\u{c0c}', BidiClass::LeftToRight), // Lo   [8] TELUGU LETTER A..TELUGU LETTER VOCALIC L
    ('\u{c0e}', '\u{c10}', BidiClass::LeftToRight), // Lo   [3] TELUGU LETTER E..TELUGU LETTER AI
    ('\u{c12}', '\u{c28}', BidiClass::LeftToRight), // Lo  [23] TELUGU LETTER O..TELUGU LETTER NA
    ('\u{c2a}', '\u{c39}', BidiClass::LeftToRight), // Lo  [16] TELUGU LETTER PA..TELUGU LETTER HA
    ('\u{c3c}', '\u{c3c}', BidiClass::NonspacingMark), // Mn       TELUGU SIGN NUKTA
    ('\u{c3d}', '\u{c3d}', BidiClass::LeftToRight), // Lo       TELUGU SIGN AVAGRAHA
    ('\u{c3e}', '\u{c40}', BidiClass::NonspacingMark), // Mn   [3] TELUGU VOWEL SIGN AA..TELUGU VOWEL SIGN II
    ('\u{c41}', '\u{c44}', BidiClass::LeftToRight), // Mc   [4] TELUGU VOWEL SIGN U..TELUGU VOWEL SIGN VOCALIC RR
    ('\u{c46}', '\u{c48}', BidiClass::NonspacingMark), // Mn   [3] TELUGU VOWEL SIGN E..TELUGU VOWEL SIGN AI
    ('\u{c4a}', '\u{c4d}', BidiClass::NonspacingMark), // Mn   [4] TELUGU VOWEL SIGN O..TELUGU SIGN VIRAMA
    ('\u{c55}', '\u{c56}', BidiClass::NonspacingMark), // Mn   [2] TELUGU LENGTH MARK..TELUGU AI LENGTH MARK
    ('\u{c58}', '\u{c5a}', BidiClass::LeftToRight), // Lo   [3] TELUGU LETTER TSA..TELUGU LETTER RRRA
    ('\u{c5d}', '\u{c5d}', BidiClass::LeftToRight), // Lo       TELUGU LETTER NAKAARA POLLU
    ('\u{c60}', '\u{c61}', BidiClass::LeftToRight), // Lo   [2] TELUGU LETTER VOCALIC RR..TELUGU LETTER VOCALIC LL
    ('\u{c62}', '\u{c63}', BidiClass::NonspacingMark), // Mn   [2] TELUGU VOWEL SIGN VOCALIC L..TELUGU VOWEL SIGN VOCALIC LL
    ('\u{c66}', '\u{c6f}', BidiClass::LeftToRight), // Nd  [10] TELUGU DIGIT ZERO..TELUGU DIGIT NINE
    ('\u{c77}', '\u{c77}', BidiClass::LeftToRight), // Po       TELUGU SIGN SIDDHAM
    ('\u{c78}', '\u{c7e}', BidiClass::OtherNeutral), // No   [7] TELUGU FRACTION DIGIT ZERO FOR ODD POWERS OF FOUR..TELUGU FRACTION DIGIT THREE FOR EVEN POWERS OF FOUR
    ('\u{c7f}', '\u{c7f}', BidiClass::LeftToRight),  // So       TELUGU SIGN TUUMU
    ('\u{c80}', '\u{c80}', BidiClass::LeftToRight),  // Lo       KANNADA SIGN SPACING CANDRABINDU
    ('\u{c81}', '\u{c81}', BidiClass::NonspacingMark), // Mn       KANNADA SIGN CANDRABINDU
    ('\u{c82}', '\u{c83}', BidiClass::LeftToRight), // Mc   [2] KANNADA SIGN ANUSVARA..KANNADA SIGN VISARGA
    ('\u{c84}', '\u{c84}', BidiClass::LeftToRight), // Po       KANNADA SIGN SIDDHAM
    ('\u{c85}', '\u{c8c}', BidiClass::LeftToRight), // Lo   [8] KANNADA LETTER A..KANNADA LETTER VOCALIC L
    ('\u{c8e}', '\u{c90}', BidiClass::LeftToRight), // Lo   [3] KANNADA LETTER E..KANNADA LETTER AI
    ('\u{c92}', '\u{ca8}', BidiClass::LeftToRight), // Lo  [23] KANNADA LETTER O..KANNADA LETTER NA
    ('\u{caa}', '\u{cb3}', BidiClass::LeftToRight), // Lo  [10] KANNADA LETTER PA..KANNADA LETTER LLA
    ('\u{cb5}', '\u{cb9}', BidiClass::LeftToRight), // Lo   [5] KANNADA LETTER VA..KANNADA LETTER HA
    ('\u{cbc}', '\u{cbc}', BidiClass::NonspacingMark), // Mn       KANNADA SIGN NUKTA
    ('\u{cbd}', '\u{cbd}', BidiClass::LeftToRight), // Lo       KANNADA SIGN AVAGRAHA
    ('\u{cbe}', '\u{cbe}', BidiClass::LeftToRight), // Mc       KANNADA VOWEL SIGN AA
    ('\u{cbf}', '\u{cbf}', BidiClass::LeftToRight), // Mn       KANNADA VOWEL SIGN I
    ('\u{cc0}', '\u{cc4}', BidiClass::LeftToRight), // Mc   [5] KANNADA VOWEL SIGN II..KANNADA VOWEL SIGN VOCALIC RR
    ('\u{cc6}', '\u{cc6}', BidiClass::LeftToRight), // Mn       KANNADA VOWEL SIGN E
    ('\u{cc7}', '\u{cc8}', BidiClass::LeftToRight), // Mc   [2] KANNADA VOWEL SIGN EE..KANNADA VOWEL SIGN AI
    ('\u{cca}', '\u{ccb}', BidiClass::LeftToRight), // Mc   [2] KANNADA VOWEL SIGN O..KANNADA VOWEL SIGN OO
    ('\u{ccc}', '\u{ccd}', BidiClass::NonspacingMark), // Mn   [2] KANNADA VOWEL SIGN AU..KANNADA SIGN VIRAMA
    ('\u{cd5}', '\u{cd6}', BidiClass::LeftToRight), // Mc   [2] KANNADA LENGTH MARK..KANNADA AI LENGTH MARK
    ('\u{cdd}', '\u{cde}', BidiClass::LeftToRight), // Lo   [2] KANNADA LETTER NAKAARA POLLU..KANNADA LETTER FA
    ('\u{ce0}', '\u{ce1}', BidiClass::LeftToRight), // Lo   [2] KANNADA LETTER VOCALIC RR..KANNADA LETTER VOCALIC LL
    ('\u{ce2}', '\u{ce3}', BidiClass::NonspacingMark), // Mn   [2] KANNADA VOWEL SIGN VOCALIC L..KANNADA VOWEL SIGN VOCALIC LL
    ('\u{ce6}', '\u{cef}', BidiClass::LeftToRight), // Nd  [10] KANNADA DIGIT ZERO..KANNADA DIGIT NINE
    ('\u{cf1}', '\u{cf2}', BidiClass::LeftToRight), // Lo   [2] KANNADA SIGN JIHVAMULIYA..KANNADA SIGN UPADHMANIYA
    ('\u{d00}', '\u{d01}', BidiClass::NonspacingMark), // Mn   [2] MALAYALAM SIGN COMBINING ANUSVARA ABOVE..MALAYALAM SIGN CANDRABINDU
    ('\u{d02}', '\u{d03}', BidiClass::LeftToRight), // Mc   [2] MALAYALAM SIGN ANUSVARA..MALAYALAM SIGN VISARGA
    ('\u{d04}', '\u{d0c}', BidiClass::LeftToRight), // Lo   [9] MALAYALAM LETTER VEDIC ANUSVARA..MALAYALAM LETTER VOCALIC L
    ('\u{d0e}', '\u{d10}', BidiClass::LeftToRight), // Lo   [3] MALAYALAM LETTER E..MALAYALAM LETTER AI
    ('\u{d12}', '\u{d3a}', BidiClass::LeftToRight), // Lo  [41] MALAYALAM LETTER O..MALAYALAM LETTER TTTA
    ('\u{d3b}', '\u{d3c}', BidiClass::NonspacingMark), // Mn   [2] MALAYALAM SIGN VERTICAL BAR VIRAMA..MALAYALAM SIGN CIRCULAR VIRAMA
    ('\u{d3d}', '\u{d3d}', BidiClass::LeftToRight),    // Lo       MALAYALAM SIGN AVAGRAHA
    ('\u{d3e}', '\u{d40}', BidiClass::LeftToRight), // Mc   [3] MALAYALAM VOWEL SIGN AA..MALAYALAM VOWEL SIGN II
    ('\u{d41}', '\u{d44}', BidiClass::NonspacingMark), // Mn   [4] MALAYALAM VOWEL SIGN U..MALAYALAM VOWEL SIGN VOCALIC RR
    ('\u{d46}', '\u{d48}', BidiClass::LeftToRight), // Mc   [3] MALAYALAM VOWEL SIGN E..MALAYALAM VOWEL SIGN AI
    ('\u{d4a}', '\u{d4c}', BidiClass::LeftToRight), // Mc   [3] MALAYALAM VOWEL SIGN O..MALAYALAM VOWEL SIGN AU
    ('\u{d4d}', '\u{d4d}', BidiClass::NonspacingMark), // Mn       MALAYALAM SIGN VIRAMA
    ('\u{d4e}', '\u{d4e}', BidiClass::LeftToRight), // Lo       MALAYALAM LETTER DOT REPH
    ('\u{d4f}', '\u{d4f}', BidiClass::LeftToRight), // So       MALAYALAM SIGN PARA
    ('\u{d54}', '\u{d56}', BidiClass::LeftToRight), // Lo   [3] MALAYALAM LETTER CHILLU M..MALAYALAM LETTER CHILLU LLL
    ('\u{d57}', '\u{d57}', BidiClass::LeftToRight), // Mc       MALAYALAM AU LENGTH MARK
    ('\u{d58}', '\u{d5e}', BidiClass::LeftToRight), // No   [7] MALAYALAM FRACTION ONE ONE-HUNDRED-AND-SIXTIETH..MALAYALAM FRACTION ONE FIFTH
    ('\u{d5f}', '\u{d61}', BidiClass::LeftToRight), // Lo   [3] MALAYALAM LETTER ARCHAIC II..MALAYALAM LETTER VOCALIC LL
    ('\u{d62}', '\u{d63}', BidiClass::NonspacingMark), // Mn   [2] MALAYALAM VOWEL SIGN VOCALIC L..MALAYALAM VOWEL SIGN VOCALIC LL
    ('\u{d66}', '\u{d6f}', BidiClass::LeftToRight), // Nd  [10] MALAYALAM DIGIT ZERO..MALAYALAM DIGIT NINE
    ('\u{d70}', '\u{d78}', BidiClass::LeftToRight), // No   [9] MALAYALAM NUMBER TEN..MALAYALAM FRACTION THREE SIXTEENTHS
    ('\u{d79}', '\u{d79}', BidiClass::LeftToRight), // So       MALAYALAM DATE MARK
    ('\u{d7a}', '\u{d7f}', BidiClass::LeftToRight), // Lo   [6] MALAYALAM LETTER CHILLU NN..MALAYALAM LETTER CHILLU K
    ('\u{d81}', '\u{d81}', BidiClass::NonspacingMark), // Mn       SINHALA SIGN CANDRABINDU
    ('\u{d82}', '\u{d83}', BidiClass::LeftToRight), // Mc   [2] SINHALA SIGN ANUSVARAYA..SINHALA SIGN VISARGAYA
    ('\u{d85}', '\u{d96}', BidiClass::LeftToRight), // Lo  [18] SINHALA LETTER AYANNA..SINHALA LETTER AUYANNA
    ('\u{d9a}', '\u{db1}', BidiClass::LeftToRight), // Lo  [24] SINHALA LETTER ALPAPRAANA KAYANNA..SINHALA LETTER DANTAJA NAYANNA
    ('\u{db3}', '\u{dbb}', BidiClass::LeftToRight), // Lo   [9] SINHALA LETTER SANYAKA DAYANNA..SINHALA LETTER RAYANNA
    ('\u{dbd}', '\u{dbd}', BidiClass::LeftToRight), // Lo       SINHALA LETTER DANTAJA LAYANNA
    ('\u{dc0}', '\u{dc6}', BidiClass::LeftToRight), // Lo   [7] SINHALA LETTER VAYANNA..SINHALA LETTER FAYANNA
    ('\u{dca}', '\u{dca}', BidiClass::NonspacingMark), // Mn       SINHALA SIGN AL-LAKUNA
    ('\u{dcf}', '\u{dd1}', BidiClass::LeftToRight), // Mc   [3] SINHALA VOWEL SIGN AELA-PILLA..SINHALA VOWEL SIGN DIGA AEDA-PILLA
    ('\u{dd2}', '\u{dd4}', BidiClass::NonspacingMark), // Mn   [3] SINHALA VOWEL SIGN KETTI IS-PILLA..SINHALA VOWEL SIGN KETTI PAA-PILLA
    ('\u{dd6}', '\u{dd6}', BidiClass::NonspacingMark), // Mn       SINHALA VOWEL SIGN DIGA PAA-PILLA
    ('\u{dd8}', '\u{ddf}', BidiClass::LeftToRight), // Mc   [8] SINHALA VOWEL SIGN GAETTA-PILLA..SINHALA VOWEL SIGN GAYANUKITTA
    ('\u{de6}', '\u{def}', BidiClass::LeftToRight), // Nd  [10] SINHALA LITH DIGIT ZERO..SINHALA LITH DIGIT NINE
    ('\u{df2}', '\u{df3}', BidiClass::LeftToRight), // Mc   [2] SINHALA VOWEL SIGN DIGA GAETTA-PILLA..SINHALA VOWEL SIGN DIGA GAYANUKITTA
    ('\u{df4}', '\u{df4}', BidiClass::LeftToRight), // Po       SINHALA PUNCTUATION KUNDDALIYA
    ('\u{e01}', '\u{e30}', BidiClass::LeftToRight), // Lo  [48] THAI CHARACTER KO KAI..THAI CHARACTER SARA A
    ('\u{e31}', '\u{e31}', BidiClass::NonspacingMark), // Mn       THAI CHARACTER MAI HAN-AKAT
    ('\u{e32}', '\u{e33}', BidiClass::LeftToRight), // Lo   [2] THAI CHARACTER SARA AA..THAI CHARACTER SARA AM
    ('\u{e34}', '\u{e3a}', BidiClass::NonspacingMark), // Mn   [7] THAI CHARACTER SARA I..THAI CHARACTER PHINTHU
    ('\u{e3f}', '\u{e3f}', BidiClass::EuropeanTerminator), // Sc       THAI CURRENCY SYMBOL BAHT
    ('\u{e40}', '\u{e45}', BidiClass::LeftToRight), // Lo   [6] THAI CHARACTER SARA E..THAI CHARACTER LAKKHANGYAO
    ('\u{e46}', '\u{e46}', BidiClass::LeftToRight), // Lm       THAI CHARACTER MAIYAMOK
    ('\u{e47}', '\u{e4e}', BidiClass::NonspacingMark), // Mn   [8] THAI CHARACTER MAITAIKHU..THAI CHARACTER YAMAKKAN
    ('\u{e4f}', '\u{e4f}', BidiClass::LeftToRight),    // Po       THAI CHARACTER FONGMAN
    ('\u{e50}', '\u{e59}', BidiClass::LeftToRight),    // Nd  [10] THAI DIGIT ZERO..THAI DIGIT NINE
    ('\u{e5a}', '\u{e5b}', BidiClass::LeftToRight), // Po   [2] THAI CHARACTER ANGKHANKHU..THAI CHARACTER KHOMUT
    ('\u{e81}', '\u{e82}', BidiClass::LeftToRight), // Lo   [2] LAO LETTER KO..LAO LETTER KHO SUNG
    ('\u{e84}', '\u{e84}', BidiClass::LeftToRight), // Lo       LAO LETTER KHO TAM
    ('\u{e86}', '\u{e8a}', BidiClass::LeftToRight), // Lo   [5] LAO LETTER PALI GHA..LAO LETTER SO TAM
    ('\u{e8c}', '\u{ea3}', BidiClass::LeftToRight), // Lo  [24] LAO LETTER PALI JHA..LAO LETTER LO LING
    ('\u{ea5}', '\u{ea5}', BidiClass::LeftToRight), // Lo       LAO LETTER LO LOOT
    ('\u{ea7}', '\u{eb0}', BidiClass::LeftToRight), // Lo  [10] LAO LETTER WO..LAO VOWEL SIGN A
    ('\u{eb1}', '\u{eb1}', BidiClass::NonspacingMark), // Mn       LAO VOWEL SIGN MAI KAN
    ('\u{eb2}', '\u{eb3}', BidiClass::LeftToRight), // Lo   [2] LAO VOWEL SIGN AA..LAO VOWEL SIGN AM
    ('\u{eb4}', '\u{ebc}', BidiClass::NonspacingMark), // Mn   [9] LAO VOWEL SIGN I..LAO SEMIVOWEL SIGN LO
    ('\u{ebd}', '\u{ebd}', BidiClass::LeftToRight),    // Lo       LAO SEMIVOWEL SIGN NYO
    ('\u{ec0}', '\u{ec4}', BidiClass::LeftToRight), // Lo   [5] LAO VOWEL SIGN E..LAO VOWEL SIGN AI
    ('\u{ec6}', '\u{ec6}', BidiClass::LeftToRight), // Lm       LAO KO LA
    ('\u{ec8}', '\u{ecd}', BidiClass::NonspacingMark), // Mn   [6] LAO TONE MAI EK..LAO NIGGAHITA
    ('\u{ed0}', '\u{ed9}', BidiClass::LeftToRight), // Nd  [10] LAO DIGIT ZERO..LAO DIGIT NINE
    ('\u{edc}', '\u{edf}', BidiClass::LeftToRight), // Lo   [4] LAO HO NO..LAO LETTER KHMU NYO
    ('\u{f00}', '\u{f00}', BidiClass::LeftToRight), // Lo       TIBETAN SYLLABLE OM
    ('\u{f01}', '\u{f03}', BidiClass::LeftToRight), // So   [3] TIBETAN MARK GTER YIG MGO TRUNCATED A..TIBETAN MARK GTER YIG MGO -UM GTER TSHEG MA
    ('\u{f04}', '\u{f12}', BidiClass::LeftToRight), // Po  [15] TIBETAN MARK INITIAL YIG MGO MDUN MA..TIBETAN MARK RGYA GRAM SHAD
    ('\u{f13}', '\u{f13}', BidiClass::LeftToRight), // So       TIBETAN MARK CARET -DZUD RTAGS ME LONG CAN
    ('\u{f14}', '\u{f14}', BidiClass::LeftToRight), // Po       TIBETAN MARK GTER TSHEG
    ('\u{f15}', '\u{f17}', BidiClass::LeftToRight), // So   [3] TIBETAN LOGOTYPE SIGN CHAD RTAGS..TIBETAN ASTROLOGICAL SIGN SGRA GCAN -CHAR RTAGS
    ('\u{f18}', '\u{f19}', BidiClass::NonspacingMark), // Mn   [2] TIBETAN ASTROLOGICAL SIGN -KHYUD PA..TIBETAN ASTROLOGICAL SIGN SDONG TSHUGS
    ('\u{f1a}', '\u{f1f}', BidiClass::LeftToRight), // So   [6] TIBETAN SIGN RDEL DKAR GCIG..TIBETAN SIGN RDEL DKAR RDEL NAG
    ('\u{f20}', '\u{f29}', BidiClass::LeftToRight), // Nd  [10] TIBETAN DIGIT ZERO..TIBETAN DIGIT NINE
    ('\u{f2a}', '\u{f33}', BidiClass::LeftToRight), // No  [10] TIBETAN DIGIT HALF ONE..TIBETAN DIGIT HALF ZERO
    ('\u{f34}', '\u{f34}', BidiClass::LeftToRight), // So       TIBETAN MARK BSDUS RTAGS
    ('\u{f35}', '\u{f35}', BidiClass::NonspacingMark), // Mn       TIBETAN MARK NGAS BZUNG NYI ZLA
    ('\u{f36}', '\u{f36}', BidiClass::LeftToRight), // So       TIBETAN MARK CARET -DZUD RTAGS BZHI MIG CAN
    ('\u{f37}', '\u{f37}', BidiClass::NonspacingMark), // Mn       TIBETAN MARK NGAS BZUNG SGOR RTAGS
    ('\u{f38}', '\u{f38}', BidiClass::LeftToRight),    // So       TIBETAN MARK CHE MGO
    ('\u{f39}', '\u{f39}', BidiClass::NonspacingMark), // Mn       TIBETAN MARK TSA -PHRU
    ('\u{f3a}', '\u{f3a}', BidiClass::OtherNeutral),   // Ps       TIBETAN MARK GUG RTAGS GYON
    ('\u{f3b}', '\u{f3b}', BidiClass::OtherNeutral),   // Pe       TIBETAN MARK GUG RTAGS GYAS
    ('\u{f3c}', '\u{f3c}', BidiClass::OtherNeutral),   // Ps       TIBETAN MARK ANG KHANG GYON
    ('\u{f3d}', '\u{f3d}', BidiClass::OtherNeutral),   // Pe       TIBETAN MARK ANG KHANG GYAS
    ('\u{f3e}', '\u{f3f}', BidiClass::LeftToRight), // Mc   [2] TIBETAN SIGN YAR TSHES..TIBETAN SIGN MAR TSHES
    ('\u{f40}', '\u{f47}', BidiClass::LeftToRight), // Lo   [8] TIBETAN LETTER KA..TIBETAN LETTER JA
    ('\u{f49}', '\u{f6c}', BidiClass::LeftToRight), // Lo  [36] TIBETAN LETTER NYA..TIBETAN LETTER RRA
    ('\u{f71}', '\u{f7e}', BidiClass::NonspacingMark), // Mn  [14] TIBETAN VOWEL SIGN AA..TIBETAN SIGN RJES SU NGA RO
    ('\u{f7f}', '\u{f7f}', BidiClass::LeftToRight),    // Mc       TIBETAN SIGN RNAM BCAD
    ('\u{f80}', '\u{f84}', BidiClass::NonspacingMark), // Mn   [5] TIBETAN VOWEL SIGN REVERSED I..TIBETAN MARK HALANTA
    ('\u{f85}', '\u{f85}', BidiClass::LeftToRight),    // Po       TIBETAN MARK PALUTA
    ('\u{f86}', '\u{f87}', BidiClass::NonspacingMark), // Mn   [2] TIBETAN SIGN LCI RTAGS..TIBETAN SIGN YANG RTAGS
    ('\u{f88}', '\u{f8c}', BidiClass::LeftToRight), // Lo   [5] TIBETAN SIGN LCE TSA CAN..TIBETAN SIGN INVERTED MCHU CAN
    ('\u{f8d}', '\u{f97}', BidiClass::NonspacingMark), // Mn  [11] TIBETAN SUBJOINED SIGN LCE TSA CAN..TIBETAN SUBJOINED LETTER JA
    ('\u{f99}', '\u{fbc}', BidiClass::NonspacingMark), // Mn  [36] TIBETAN SUBJOINED LETTER NYA..TIBETAN SUBJOINED LETTER FIXED-FORM RA
    ('\u{fbe}', '\u{fc5}', BidiClass::LeftToRight), // So   [8] TIBETAN KU RU KHA..TIBETAN SYMBOL RDO RJE
    ('\u{fc6}', '\u{fc6}', BidiClass::NonspacingMark), // Mn       TIBETAN SYMBOL PADMA GDAN
    ('\u{fc7}', '\u{fcc}', BidiClass::LeftToRight), // So   [6] TIBETAN SYMBOL RDO RJE RGYA GRAM..TIBETAN SYMBOL NOR BU BZHI -KHYIL
    ('\u{fce}', '\u{fcf}', BidiClass::LeftToRight), // So   [2] TIBETAN SIGN RDEL NAG RDEL DKAR..TIBETAN SIGN RDEL NAG GSUM
    ('\u{fd0}', '\u{fd4}', BidiClass::LeftToRight), // Po   [5] TIBETAN MARK BSKA- SHOG GI MGO RGYAN..TIBETAN MARK CLOSING BRDA RNYING YIG MGO SGAB MA
    ('\u{fd5}', '\u{fd8}', BidiClass::LeftToRight), // So   [4] RIGHT-FACING SVASTI SIGN..LEFT-FACING SVASTI SIGN WITH DOTS
    ('\u{fd9}', '\u{fda}', BidiClass::LeftToRight), // Po   [2] TIBETAN MARK LEADING MCHAN RTAGS..TIBETAN MARK TRAILING MCHAN RTAGS
    ('\u{1000}', '\u{102a}', BidiClass::LeftToRight), // Lo  [43] MYANMAR LETTER KA..MYANMAR LETTER AU
    ('\u{102b}', '\u{102c}', BidiClass::LeftToRight), // Mc   [2] MYANMAR VOWEL SIGN TALL AA..MYANMAR VOWEL SIGN AA
    ('\u{102d}', '\u{1030}', BidiClass::NonspacingMark), // Mn   [4] MYANMAR VOWEL SIGN I..MYANMAR VOWEL SIGN UU
    ('\u{1031}', '\u{1031}', BidiClass::LeftToRight),    // Mc       MYANMAR VOWEL SIGN E
    ('\u{1032}', '\u{1037}', BidiClass::NonspacingMark), // Mn   [6] MYANMAR VOWEL SIGN AI..MYANMAR SIGN DOT BELOW
    ('\u{1038}', '\u{1038}', BidiClass::LeftToRight),    // Mc       MYANMAR SIGN VISARGA
    ('\u{1039}', '\u{103a}', BidiClass::NonspacingMark), // Mn   [2] MYANMAR SIGN VIRAMA..MYANMAR SIGN ASAT
    ('\u{103b}', '\u{103c}', BidiClass::LeftToRight), // Mc   [2] MYANMAR CONSONANT SIGN MEDIAL YA..MYANMAR CONSONANT SIGN MEDIAL RA
    ('\u{103d}', '\u{103e}', BidiClass::NonspacingMark), // Mn   [2] MYANMAR CONSONANT SIGN MEDIAL WA..MYANMAR CONSONANT SIGN MEDIAL HA
    ('\u{103f}', '\u{103f}', BidiClass::LeftToRight),    // Lo       MYANMAR LETTER GREAT SA
    ('\u{1040}', '\u{1049}', BidiClass::LeftToRight), // Nd  [10] MYANMAR DIGIT ZERO..MYANMAR DIGIT NINE
    ('\u{104a}', '\u{104f}', BidiClass::LeftToRight), // Po   [6] MYANMAR SIGN LITTLE SECTION..MYANMAR SYMBOL GENITIVE
    ('\u{1050}', '\u{1055}', BidiClass::LeftToRight), // Lo   [6] MYANMAR LETTER SHA..MYANMAR LETTER VOCALIC LL
    ('\u{1056}', '\u{1057}', BidiClass::LeftToRight), // Mc   [2] MYANMAR VOWEL SIGN VOCALIC R..MYANMAR VOWEL SIGN VOCALIC RR
    ('\u{1058}', '\u{1059}', BidiClass::NonspacingMark), // Mn   [2] MYANMAR VOWEL SIGN VOCALIC L..MYANMAR VOWEL SIGN VOCALIC LL
    ('\u{105a}', '\u{105d}', BidiClass::LeftToRight), // Lo   [4] MYANMAR LETTER MON NGA..MYANMAR LETTER MON BBE
    ('\u{105e}', '\u{1060}', BidiClass::NonspacingMark), // Mn   [3] MYANMAR CONSONANT SIGN MON MEDIAL NA..MYANMAR CONSONANT SIGN MON MEDIAL LA
    ('\u{1061}', '\u{1061}', BidiClass::LeftToRight),    // Lo       MYANMAR LETTER SGAW KAREN SHA
    ('\u{1062}', '\u{1064}', BidiClass::LeftToRight), // Mc   [3] MYANMAR VOWEL SIGN SGAW KAREN EU..MYANMAR TONE MARK SGAW KAREN KE PHO
    ('\u{1065}', '\u{1066}', BidiClass::LeftToRight), // Lo   [2] MYANMAR LETTER WESTERN PWO KAREN THA..MYANMAR LETTER WESTERN PWO KAREN PWA
    ('\u{1067}', '\u{106d}', BidiClass::LeftToRight), // Mc   [7] MYANMAR VOWEL SIGN WESTERN PWO KAREN EU..MYANMAR SIGN WESTERN PWO KAREN TONE-5
    ('\u{106e}', '\u{1070}', BidiClass::LeftToRight), // Lo   [3] MYANMAR LETTER EASTERN PWO KAREN NNA..MYANMAR LETTER EASTERN PWO KAREN GHWA
    ('\u{1071}', '\u{1074}', BidiClass::NonspacingMark), // Mn   [4] MYANMAR VOWEL SIGN GEBA KAREN I..MYANMAR VOWEL SIGN KAYAH EE
    ('\u{1075}', '\u{1081}', BidiClass::LeftToRight), // Lo  [13] MYANMAR LETTER SHAN KA..MYANMAR LETTER SHAN HA
    ('\u{1082}', '\u{1082}', BidiClass::NonspacingMark), // Mn       MYANMAR CONSONANT SIGN SHAN MEDIAL WA
    ('\u{1083}', '\u{1084}', BidiClass::LeftToRight), // Mc   [2] MYANMAR VOWEL SIGN SHAN AA..MYANMAR VOWEL SIGN SHAN E
    ('\u{1085}', '\u{1086}', BidiClass::NonspacingMark), // Mn   [2] MYANMAR VOWEL SIGN SHAN E ABOVE..MYANMAR VOWEL SIGN SHAN FINAL Y
    ('\u{1087}', '\u{108c}', BidiClass::LeftToRight), // Mc   [6] MYANMAR SIGN SHAN TONE-2..MYANMAR SIGN SHAN COUNCIL TONE-3
    ('\u{108d}', '\u{108d}', BidiClass::NonspacingMark), // Mn       MYANMAR SIGN SHAN COUNCIL EMPHATIC TONE
    ('\u{108e}', '\u{108e}', BidiClass::LeftToRight),    // Lo       MYANMAR LETTER RUMAI PALAUNG FA
    ('\u{108f}', '\u{108f}', BidiClass::LeftToRight), // Mc       MYANMAR SIGN RUMAI PALAUNG TONE-5
    ('\u{1090}', '\u{1099}', BidiClass::LeftToRight), // Nd  [10] MYANMAR SHAN DIGIT ZERO..MYANMAR SHAN DIGIT NINE
    ('\u{109a}', '\u{109c}', BidiClass::LeftToRight), // Mc   [3] MYANMAR SIGN KHAMTI TONE-1..MYANMAR VOWEL SIGN AITON A
    ('\u{109d}', '\u{109d}', BidiClass::NonspacingMark), // Mn       MYANMAR VOWEL SIGN AITON AI
    ('\u{109e}', '\u{109f}', BidiClass::LeftToRight), // So   [2] MYANMAR SYMBOL SHAN ONE..MYANMAR SYMBOL SHAN EXCLAMATION
    ('\u{10a0}', '\u{10c5}', BidiClass::LeftToRight), // L&  [38] GEORGIAN CAPITAL LETTER AN..GEORGIAN CAPITAL LETTER HOE
    ('\u{10c7}', '\u{10c7}', BidiClass::LeftToRight), // L&       GEORGIAN CAPITAL LETTER YN
    ('\u{10cd}', '\u{10cd}', BidiClass::LeftToRight), // L&       GEORGIAN CAPITAL LETTER AEN
    ('\u{10d0}', '\u{10fa}', BidiClass::LeftToRight), // L&  [43] GEORGIAN LETTER AN..GEORGIAN LETTER AIN
    ('\u{10fb}', '\u{10fb}', BidiClass::LeftToRight), // Po       GEORGIAN PARAGRAPH SEPARATOR
    ('\u{10fc}', '\u{10fc}', BidiClass::LeftToRight), // Lm       MODIFIER LETTER GEORGIAN NAR
    ('\u{10fd}', '\u{10ff}', BidiClass::LeftToRight), // L&   [3] GEORGIAN LETTER AEN..GEORGIAN LETTER LABIAL SIGN
    ('\u{1100}', '\u{1248}', BidiClass::LeftToRight), // Lo [329] HANGUL CHOSEONG KIYEOK..ETHIOPIC SYLLABLE QWA
    ('\u{124a}', '\u{124d}', BidiClass::LeftToRight), // Lo   [4] ETHIOPIC SYLLABLE QWI..ETHIOPIC SYLLABLE QWE
    ('\u{1250}', '\u{1256}', BidiClass::LeftToRight), // Lo   [7] ETHIOPIC SYLLABLE QHA..ETHIOPIC SYLLABLE QHO
    ('\u{1258}', '\u{1258}', BidiClass::LeftToRight), // Lo       ETHIOPIC SYLLABLE QHWA
    ('\u{125a}', '\u{125d}', BidiClass::LeftToRight), // Lo   [4] ETHIOPIC SYLLABLE QHWI..ETHIOPIC SYLLABLE QHWE
    ('\u{1260}', '\u{1288}', BidiClass::LeftToRight), // Lo  [41] ETHIOPIC SYLLABLE BA..ETHIOPIC SYLLABLE XWA
    ('\u{128a}', '\u{128d}', BidiClass::LeftToRight), // Lo   [4] ETHIOPIC SYLLABLE XWI..ETHIOPIC SYLLABLE XWE
    ('\u{1290}', '\u{12b0}', BidiClass::LeftToRight), // Lo  [33] ETHIOPIC SYLLABLE NA..ETHIOPIC SYLLABLE KWA
    ('\u{12b2}', '\u{12b5}', BidiClass::LeftToRight), // Lo   [4] ETHIOPIC SYLLABLE KWI..ETHIOPIC SYLLABLE KWE
    ('\u{12b8}', '\u{12be}', BidiClass::LeftToRight), // Lo   [7] ETHIOPIC SYLLABLE KXA..ETHIOPIC SYLLABLE KXO
    ('\u{12c0}', '\u{12c0}', BidiClass::LeftToRight), // Lo       ETHIOPIC SYLLABLE KXWA
    ('\u{12c2}', '\u{12c5}', BidiClass::LeftToRight), // Lo   [4] ETHIOPIC SYLLABLE KXWI..ETHIOPIC SYLLABLE KXWE
    ('\u{12c8}', '\u{12d6}', BidiClass::LeftToRight), // Lo  [15] ETHIOPIC SYLLABLE WA..ETHIOPIC SYLLABLE PHARYNGEAL O
    ('\u{12d8}', '\u{1310}', BidiClass::LeftToRight), // Lo  [57] ETHIOPIC SYLLABLE ZA..ETHIOPIC SYLLABLE GWA
    ('\u{1312}', '\u{1315}', BidiClass::LeftToRight), // Lo   [4] ETHIOPIC SYLLABLE GWI..ETHIOPIC SYLLABLE GWE
    ('\u{1318}', '\u{135a}', BidiClass::LeftToRight), // Lo  [67] ETHIOPIC SYLLABLE GGA..ETHIOPIC SYLLABLE FYA
    ('\u{135d}', '\u{135f}', BidiClass::NonspacingMark), // Mn   [3] ETHIOPIC COMBINING GEMINATION AND VOWEL LENGTH MARK..ETHIOPIC COMBINING GEMINATION MARK
    ('\u{1360}', '\u{1368}', BidiClass::LeftToRight), // Po   [9] ETHIOPIC SECTION MARK..ETHIOPIC PARAGRAPH SEPARATOR
    ('\u{1369}', '\u{137c}', BidiClass::LeftToRight), // No  [20] ETHIOPIC DIGIT ONE..ETHIOPIC NUMBER TEN THOUSAND
    ('\u{1380}', '\u{138f}', BidiClass::LeftToRight), // Lo  [16] ETHIOPIC SYLLABLE SEBATBEIT MWA..ETHIOPIC SYLLABLE PWE
    ('\u{1390}', '\u{1399}', BidiClass::OtherNeutral), // So  [10] ETHIOPIC TONAL MARK YIZET..ETHIOPIC TONAL MARK KURT
    ('\u{13a0}', '\u{13f5}', BidiClass::LeftToRight), // L&  [86] CHEROKEE LETTER A..CHEROKEE LETTER MV
    ('\u{13f8}', '\u{13fd}', BidiClass::LeftToRight), // L&   [6] CHEROKEE SMALL LETTER YE..CHEROKEE SMALL LETTER MV
    ('\u{1400}', '\u{1400}', BidiClass::OtherNeutral), // Pd       CANADIAN SYLLABICS HYPHEN
    ('\u{1401}', '\u{166c}', BidiClass::LeftToRight), // Lo [620] CANADIAN SYLLABICS E..CANADIAN SYLLABICS CARRIER TTSA
    ('\u{166d}', '\u{166d}', BidiClass::LeftToRight), // So       CANADIAN SYLLABICS CHI SIGN
    ('\u{166e}', '\u{166e}', BidiClass::LeftToRight), // Po       CANADIAN SYLLABICS FULL STOP
    ('\u{166f}', '\u{167f}', BidiClass::LeftToRight), // Lo  [17] CANADIAN SYLLABICS QAI..CANADIAN SYLLABICS BLACKFOOT W
    ('\u{1680}', '\u{1680}', BidiClass::WhiteSpace),  // Zs       OGHAM SPACE MARK
    ('\u{1681}', '\u{169a}', BidiClass::LeftToRight), // Lo  [26] OGHAM LETTER BEITH..OGHAM LETTER PEITH
    ('\u{169b}', '\u{169b}', BidiClass::OtherNeutral), // Ps       OGHAM FEATHER MARK
    ('\u{169c}', '\u{169c}', BidiClass::OtherNeutral), // Pe       OGHAM REVERSED FEATHER MARK
    ('\u{16a0}', '\u{16ea}', BidiClass::LeftToRight), // Lo  [75] RUNIC LETTER FEHU FEOH FE F..RUNIC LETTER X
    ('\u{16eb}', '\u{16ed}', BidiClass::LeftToRight), // Po   [3] RUNIC SINGLE PUNCTUATION..RUNIC CROSS PUNCTUATION
    ('\u{16ee}', '\u{16f0}', BidiClass::LeftToRight), // Nl   [3] RUNIC ARLAUG SYMBOL..RUNIC BELGTHOR SYMBOL
    ('\u{16f1}', '\u{16f8}', BidiClass::LeftToRight), // Lo   [8] RUNIC LETTER K..RUNIC LETTER FRANKS CASKET AESC
    ('\u{1700}', '\u{1711}', BidiClass::LeftToRight), // Lo  [18] TAGALOG LETTER A..TAGALOG LETTER HA
    ('\u{1712}', '\u{1714}', BidiClass::NonspacingMark), // Mn   [3] TAGALOG VOWEL SIGN I..TAGALOG SIGN VIRAMA
    ('\u{1715}', '\u{1715}', BidiClass::LeftToRight),    // Mc       TAGALOG SIGN PAMUDPOD
    ('\u{171f}', '\u{1731}', BidiClass::LeftToRight), // Lo  [19] TAGALOG LETTER ARCHAIC RA..HANUNOO LETTER HA
    ('\u{1732}', '\u{1733}', BidiClass::NonspacingMark), // Mn   [2] HANUNOO VOWEL SIGN I..HANUNOO VOWEL SIGN U
    ('\u{1734}', '\u{1734}', BidiClass::LeftToRight),    // Mc       HANUNOO SIGN PAMUDPOD
    ('\u{1735}', '\u{1736}', BidiClass::LeftToRight), // Po   [2] PHILIPPINE SINGLE PUNCTUATION..PHILIPPINE DOUBLE PUNCTUATION
    ('\u{1740}', '\u{1751}', BidiClass::LeftToRight), // Lo  [18] BUHID LETTER A..BUHID LETTER HA
    ('\u{1752}', '\u{1753}', BidiClass::NonspacingMark), // Mn   [2] BUHID VOWEL SIGN I..BUHID VOWEL SIGN U
    ('\u{1760}', '\u{176c}', BidiClass::LeftToRight), // Lo  [13] TAGBANWA LETTER A..TAGBANWA LETTER YA
    ('\u{176e}', '\u{1770}', BidiClass::LeftToRight), // Lo   [3] TAGBANWA LETTER LA..TAGBANWA LETTER SA
    ('\u{1772}', '\u{1773}', BidiClass::NonspacingMark), // Mn   [2] TAGBANWA VOWEL SIGN I..TAGBANWA VOWEL SIGN U
    ('\u{1780}', '\u{17b3}', BidiClass::LeftToRight), // Lo  [52] KHMER LETTER KA..KHMER INDEPENDENT VOWEL QAU
    ('\u{17b4}', '\u{17b5}', BidiClass::NonspacingMark), // Mn   [2] KHMER VOWEL INHERENT AQ..KHMER VOWEL INHERENT AA
    ('\u{17b6}', '\u{17b6}', BidiClass::LeftToRight),    // Mc       KHMER VOWEL SIGN AA
    ('\u{17b7}', '\u{17bd}', BidiClass::NonspacingMark), // Mn   [7] KHMER VOWEL SIGN I..KHMER VOWEL SIGN UA
    ('\u{17be}', '\u{17c5}', BidiClass::LeftToRight), // Mc   [8] KHMER VOWEL SIGN OE..KHMER VOWEL SIGN AU
    ('\u{17c6}', '\u{17c6}', BidiClass::NonspacingMark), // Mn       KHMER SIGN NIKAHIT
    ('\u{17c7}', '\u{17c8}', BidiClass::LeftToRight), // Mc   [2] KHMER SIGN REAHMUK..KHMER SIGN YUUKALEAPINTU
    ('\u{17c9}', '\u{17d3}', BidiClass::NonspacingMark), // Mn  [11] KHMER SIGN MUUSIKATOAN..KHMER SIGN BATHAMASAT
    ('\u{17d4}', '\u{17d6}', BidiClass::LeftToRight), // Po   [3] KHMER SIGN KHAN..KHMER SIGN CAMNUC PII KUUH
    ('\u{17d7}', '\u{17d7}', BidiClass::LeftToRight), // Lm       KHMER SIGN LEK TOO
    ('\u{17d8}', '\u{17da}', BidiClass::LeftToRight), // Po   [3] KHMER SIGN BEYYAL..KHMER SIGN KOOMUUT
    ('\u{17db}', '\u{17db}', BidiClass::EuropeanTerminator), // Sc       KHMER CURRENCY SYMBOL RIEL
    ('\u{17dc}', '\u{17dc}', BidiClass::LeftToRight), // Lo       KHMER SIGN AVAKRAHASANYA
    ('\u{17dd}', '\u{17dd}', BidiClass::NonspacingMark), // Mn       KHMER SIGN ATTHACAN
    ('\u{17e0}', '\u{17e9}', BidiClass::LeftToRight), // Nd  [10] KHMER DIGIT ZERO..KHMER DIGIT NINE
    ('\u{17f0}', '\u{17f9}', BidiClass::OtherNeutral), // No  [10] KHMER SYMBOL LEK ATTAK SON..KHMER SYMBOL LEK ATTAK PRAM-BUON
    ('\u{1800}', '\u{1805}', BidiClass::OtherNeutral), // Po   [6] MONGOLIAN BIRGA..MONGOLIAN FOUR DOTS
    ('\u{1806}', '\u{1806}', BidiClass::OtherNeutral), // Pd       MONGOLIAN TODO SOFT HYPHEN
    ('\u{1807}', '\u{180a}', BidiClass::OtherNeutral), // Po   [4] MONGOLIAN SIBE SYLLABLE BOUNDARY MARKER..MONGOLIAN NIRUGU
    ('\u{180b}', '\u{180d}', BidiClass::NonspacingMark), // Mn   [3] MONGOLIAN FREE VARIATION SELECTOR ONE..MONGOLIAN FREE VARIATION SELECTOR THREE
    ('\u{180e}', '\u{180e}', BidiClass::BoundaryNeutral), // Cf       MONGOLIAN VOWEL SEPARATOR
    ('\u{180f}', '\u{180f}', BidiClass::NonspacingMark), // Mn       MONGOLIAN FREE VARIATION SELECTOR FOUR
    ('\u{1810}', '\u{1819}', BidiClass::LeftToRight), // Nd  [10] MONGOLIAN DIGIT ZERO..MONGOLIAN DIGIT NINE
    ('\u{1820}', '\u{1842}', BidiClass::LeftToRight), // Lo  [35] MONGOLIAN LETTER A..MONGOLIAN LETTER CHI
    ('\u{1843}', '\u{1843}', BidiClass::LeftToRight), // Lm       MONGOLIAN LETTER TODO LONG VOWEL SIGN
    ('\u{1844}', '\u{1878}', BidiClass::LeftToRight), // Lo  [53] MONGOLIAN LETTER TODO E..MONGOLIAN LETTER CHA WITH TWO DOTS
    ('\u{1880}', '\u{1884}', BidiClass::LeftToRight), // Lo   [5] MONGOLIAN LETTER ALI GALI ANUSVARA ONE..MONGOLIAN LETTER ALI GALI INVERTED UBADAMA
    ('\u{1885}', '\u{1886}', BidiClass::NonspacingMark), // Mn   [2] MONGOLIAN LETTER ALI GALI BALUDA..MONGOLIAN LETTER ALI GALI THREE BALUDA
    ('\u{1887}', '\u{18a8}', BidiClass::LeftToRight), // Lo  [34] MONGOLIAN LETTER ALI GALI A..MONGOLIAN LETTER MANCHU ALI GALI BHA
    ('\u{18a9}', '\u{18a9}', BidiClass::NonspacingMark), // Mn       MONGOLIAN LETTER ALI GALI DAGALGA
    ('\u{18aa}', '\u{18aa}', BidiClass::LeftToRight), // Lo       MONGOLIAN LETTER MANCHU ALI GALI LHA
    ('\u{18b0}', '\u{18f5}', BidiClass::LeftToRight), // Lo  [70] CANADIAN SYLLABICS OY..CANADIAN SYLLABICS CARRIER DENTAL S
    ('\u{1900}', '\u{191e}', BidiClass::LeftToRight), // Lo  [31] LIMBU VOWEL-CARRIER LETTER..LIMBU LETTER TRA
    ('\u{1920}', '\u{1922}', BidiClass::NonspacingMark), // Mn   [3] LIMBU VOWEL SIGN A..LIMBU VOWEL SIGN U
    ('\u{1923}', '\u{1926}', BidiClass::LeftToRight), // Mc   [4] LIMBU VOWEL SIGN EE..LIMBU VOWEL SIGN AU
    ('\u{1927}', '\u{1928}', BidiClass::NonspacingMark), // Mn   [2] LIMBU VOWEL SIGN E..LIMBU VOWEL SIGN O
    ('\u{1929}', '\u{192b}', BidiClass::LeftToRight), // Mc   [3] LIMBU SUBJOINED LETTER YA..LIMBU SUBJOINED LETTER WA
    ('\u{1930}', '\u{1931}', BidiClass::LeftToRight), // Mc   [2] LIMBU SMALL LETTER KA..LIMBU SMALL LETTER NGA
    ('\u{1932}', '\u{1932}', BidiClass::NonspacingMark), // Mn       LIMBU SMALL LETTER ANUSVARA
    ('\u{1933}', '\u{1938}', BidiClass::LeftToRight), // Mc   [6] LIMBU SMALL LETTER TA..LIMBU SMALL LETTER LA
    ('\u{1939}', '\u{193b}', BidiClass::NonspacingMark), // Mn   [3] LIMBU SIGN MUKPHRENG..LIMBU SIGN SA-I
    ('\u{1940}', '\u{1940}', BidiClass::OtherNeutral),   // So       LIMBU SIGN LOO
    ('\u{1944}', '\u{1945}', BidiClass::OtherNeutral), // Po   [2] LIMBU EXCLAMATION MARK..LIMBU QUESTION MARK
    ('\u{1946}', '\u{194f}', BidiClass::LeftToRight), // Nd  [10] LIMBU DIGIT ZERO..LIMBU DIGIT NINE
    ('\u{1950}', '\u{196d}', BidiClass::LeftToRight), // Lo  [30] TAI LE LETTER KA..TAI LE LETTER AI
    ('\u{1970}', '\u{1974}', BidiClass::LeftToRight), // Lo   [5] TAI LE LETTER TONE-2..TAI LE LETTER TONE-6
    ('\u{1980}', '\u{19ab}', BidiClass::LeftToRight), // Lo  [44] NEW TAI LUE LETTER HIGH QA..NEW TAI LUE LETTER LOW SUA
    ('\u{19b0}', '\u{19c9}', BidiClass::LeftToRight), // Lo  [26] NEW TAI LUE VOWEL SIGN VOWEL SHORTENER..NEW TAI LUE TONE MARK-2
    ('\u{19d0}', '\u{19d9}', BidiClass::LeftToRight), // Nd  [10] NEW TAI LUE DIGIT ZERO..NEW TAI LUE DIGIT NINE
    ('\u{19da}', '\u{19da}', BidiClass::LeftToRight), // No       NEW TAI LUE THAM DIGIT ONE
    ('\u{19de}', '\u{19ff}', BidiClass::OtherNeutral), // So  [34] NEW TAI LUE SIGN LAE..KHMER SYMBOL DAP-PRAM ROC
    ('\u{1a00}', '\u{1a16}', BidiClass::LeftToRight), // Lo  [23] BUGINESE LETTER KA..BUGINESE LETTER HA
    ('\u{1a17}', '\u{1a18}', BidiClass::NonspacingMark), // Mn   [2] BUGINESE VOWEL SIGN I..BUGINESE VOWEL SIGN U
    ('\u{1a19}', '\u{1a1a}', BidiClass::LeftToRight), // Mc   [2] BUGINESE VOWEL SIGN E..BUGINESE VOWEL SIGN O
    ('\u{1a1b}', '\u{1a1b}', BidiClass::NonspacingMark), // Mn       BUGINESE VOWEL SIGN AE
    ('\u{1a1e}', '\u{1a1f}', BidiClass::LeftToRight), // Po   [2] BUGINESE PALLAWA..BUGINESE END OF SECTION
    ('\u{1a20}', '\u{1a54}', BidiClass::LeftToRight), // Lo  [53] TAI THAM LETTER HIGH KA..TAI THAM LETTER GREAT SA
    ('\u{1a55}', '\u{1a55}', BidiClass::LeftToRight), // Mc       TAI THAM CONSONANT SIGN MEDIAL RA
    ('\u{1a56}', '\u{1a56}', BidiClass::NonspacingMark), // Mn       TAI THAM CONSONANT SIGN MEDIAL LA
    ('\u{1a57}', '\u{1a57}', BidiClass::LeftToRight), // Mc       TAI THAM CONSONANT SIGN LA TANG LAI
    ('\u{1a58}', '\u{1a5e}', BidiClass::NonspacingMark), // Mn   [7] TAI THAM SIGN MAI KANG LAI..TAI THAM CONSONANT SIGN SA
    ('\u{1a60}', '\u{1a60}', BidiClass::NonspacingMark), // Mn       TAI THAM SIGN SAKOT
    ('\u{1a61}', '\u{1a61}', BidiClass::LeftToRight),    // Mc       TAI THAM VOWEL SIGN A
    ('\u{1a62}', '\u{1a62}', BidiClass::NonspacingMark), // Mn       TAI THAM VOWEL SIGN MAI SAT
    ('\u{1a63}', '\u{1a64}', BidiClass::LeftToRight), // Mc   [2] TAI THAM VOWEL SIGN AA..TAI THAM VOWEL SIGN TALL AA
    ('\u{1a65}', '\u{1a6c}', BidiClass::NonspacingMark), // Mn   [8] TAI THAM VOWEL SIGN I..TAI THAM VOWEL SIGN OA BELOW
    ('\u{1a6d}', '\u{1a72}', BidiClass::LeftToRight), // Mc   [6] TAI THAM VOWEL SIGN OY..TAI THAM VOWEL SIGN THAM AI
    ('\u{1a73}', '\u{1a7c}', BidiClass::NonspacingMark), // Mn  [10] TAI THAM VOWEL SIGN OA ABOVE..TAI THAM SIGN KHUEN-LUE KARAN
    ('\u{1a7f}', '\u{1a7f}', BidiClass::NonspacingMark), // Mn       TAI THAM COMBINING CRYPTOGRAMMIC DOT
    ('\u{1a80}', '\u{1a89}', BidiClass::LeftToRight), // Nd  [10] TAI THAM HORA DIGIT ZERO..TAI THAM HORA DIGIT NINE
    ('\u{1a90}', '\u{1a99}', BidiClass::LeftToRight), // Nd  [10] TAI THAM THAM DIGIT ZERO..TAI THAM THAM DIGIT NINE
    ('\u{1aa0}', '\u{1aa6}', BidiClass::LeftToRight), // Po   [7] TAI THAM SIGN WIANG..TAI THAM SIGN REVERSED ROTATED RANA
    ('\u{1aa7}', '\u{1aa7}', BidiClass::LeftToRight), // Lm       TAI THAM SIGN MAI YAMOK
    ('\u{1aa8}', '\u{1aad}', BidiClass::LeftToRight), // Po   [6] TAI THAM SIGN KAAN..TAI THAM SIGN CAANG
    ('\u{1ab0}', '\u{1abd}', BidiClass::NonspacingMark), // Mn  [14] COMBINING DOUBLED CIRCUMFLEX ACCENT..COMBINING PARENTHESES BELOW
    ('\u{1abe}', '\u{1abe}', BidiClass::NonspacingMark), // Me       COMBINING PARENTHESES OVERLAY
    ('\u{1abf}', '\u{1ace}', BidiClass::NonspacingMark), // Mn  [16] COMBINING LATIN SMALL LETTER W BELOW..COMBINING LATIN SMALL LETTER INSULAR T
    ('\u{1b00}', '\u{1b03}', BidiClass::NonspacingMark), // Mn   [4] BALINESE SIGN ULU RICEM..BALINESE SIGN SURANG
    ('\u{1b04}', '\u{1b04}', BidiClass::LeftToRight),    // Mc       BALINESE SIGN BISAH
    ('\u{1b05}', '\u{1b33}', BidiClass::LeftToRight), // Lo  [47] BALINESE LETTER AKARA..BALINESE LETTER HA
    ('\u{1b34}', '\u{1b34}', BidiClass::NonspacingMark), // Mn       BALINESE SIGN REREKAN
    ('\u{1b35}', '\u{1b35}', BidiClass::LeftToRight), // Mc       BALINESE VOWEL SIGN TEDUNG
    ('\u{1b36}', '\u{1b3a}', BidiClass::NonspacingMark), // Mn   [5] BALINESE VOWEL SIGN ULU..BALINESE VOWEL SIGN RA REPA
    ('\u{1b3b}', '\u{1b3b}', BidiClass::LeftToRight), // Mc       BALINESE VOWEL SIGN RA REPA TEDUNG
    ('\u{1b3c}', '\u{1b3c}', BidiClass::NonspacingMark), // Mn       BALINESE VOWEL SIGN LA LENGA
    ('\u{1b3d}', '\u{1b41}', BidiClass::LeftToRight), // Mc   [5] BALINESE VOWEL SIGN LA LENGA TEDUNG..BALINESE VOWEL SIGN TALING REPA TEDUNG
    ('\u{1b42}', '\u{1b42}', BidiClass::NonspacingMark), // Mn       BALINESE VOWEL SIGN PEPET
    ('\u{1b43}', '\u{1b44}', BidiClass::LeftToRight), // Mc   [2] BALINESE VOWEL SIGN PEPET TEDUNG..BALINESE ADEG ADEG
    ('\u{1b45}', '\u{1b4c}', BidiClass::LeftToRight), // Lo   [8] BALINESE LETTER KAF SASAK..BALINESE LETTER ARCHAIC JNYA
    ('\u{1b50}', '\u{1b59}', BidiClass::LeftToRight), // Nd  [10] BALINESE DIGIT ZERO..BALINESE DIGIT NINE
    ('\u{1b5a}', '\u{1b60}', BidiClass::LeftToRight), // Po   [7] BALINESE PANTI..BALINESE PAMENENG
    ('\u{1b61}', '\u{1b6a}', BidiClass::LeftToRight), // So  [10] BALINESE MUSICAL SYMBOL DONG..BALINESE MUSICAL SYMBOL DANG GEDE
    ('\u{1b6b}', '\u{1b73}', BidiClass::NonspacingMark), // Mn   [9] BALINESE MUSICAL SYMBOL COMBINING TEGEH..BALINESE MUSICAL SYMBOL COMBINING GONG
    ('\u{1b74}', '\u{1b7c}', BidiClass::LeftToRight), // So   [9] BALINESE MUSICAL SYMBOL RIGHT-HAND OPEN DUG..BALINESE MUSICAL SYMBOL LEFT-HAND OPEN PING
    ('\u{1b7d}', '\u{1b7e}', BidiClass::LeftToRight), // Po   [2] BALINESE PANTI LANTANG..BALINESE PAMADA LANTANG
    ('\u{1b80}', '\u{1b81}', BidiClass::NonspacingMark), // Mn   [2] SUNDANESE SIGN PANYECEK..SUNDANESE SIGN PANGLAYAR
    ('\u{1b82}', '\u{1b82}', BidiClass::LeftToRight),    // Mc       SUNDANESE SIGN PANGWISAD
    ('\u{1b83}', '\u{1ba0}', BidiClass::LeftToRight), // Lo  [30] SUNDANESE LETTER A..SUNDANESE LETTER HA
    ('\u{1ba1}', '\u{1ba1}', BidiClass::LeftToRight), // Mc       SUNDANESE CONSONANT SIGN PAMINGKAL
    ('\u{1ba2}', '\u{1ba5}', BidiClass::NonspacingMark), // Mn   [4] SUNDANESE CONSONANT SIGN PANYAKRA..SUNDANESE VOWEL SIGN PANYUKU
    ('\u{1ba6}', '\u{1ba7}', BidiClass::LeftToRight), // Mc   [2] SUNDANESE VOWEL SIGN PANAELAENG..SUNDANESE VOWEL SIGN PANOLONG
    ('\u{1ba8}', '\u{1ba9}', BidiClass::NonspacingMark), // Mn   [2] SUNDANESE VOWEL SIGN PAMEPET..SUNDANESE VOWEL SIGN PANEULEUNG
    ('\u{1baa}', '\u{1baa}', BidiClass::LeftToRight),    // Mc       SUNDANESE SIGN PAMAAEH
    ('\u{1bab}', '\u{1bad}', BidiClass::NonspacingMark), // Mn   [3] SUNDANESE SIGN VIRAMA..SUNDANESE CONSONANT SIGN PASANGAN WA
    ('\u{1bae}', '\u{1baf}', BidiClass::LeftToRight), // Lo   [2] SUNDANESE LETTER KHA..SUNDANESE LETTER SYA
    ('\u{1bb0}', '\u{1bb9}', BidiClass::LeftToRight), // Nd  [10] SUNDANESE DIGIT ZERO..SUNDANESE DIGIT NINE
    ('\u{1bba}', '\u{1be5}', BidiClass::LeftToRight), // Lo  [44] SUNDANESE AVAGRAHA..BATAK LETTER U
    ('\u{1be6}', '\u{1be6}', BidiClass::NonspacingMark), // Mn       BATAK SIGN TOMPI
    ('\u{1be7}', '\u{1be7}', BidiClass::LeftToRight), // Mc       BATAK VOWEL SIGN E
    ('\u{1be8}', '\u{1be9}', BidiClass::NonspacingMark), // Mn   [2] BATAK VOWEL SIGN PAKPAK E..BATAK VOWEL SIGN EE
    ('\u{1bea}', '\u{1bec}', BidiClass::LeftToRight), // Mc   [3] BATAK VOWEL SIGN I..BATAK VOWEL SIGN O
    ('\u{1bed}', '\u{1bed}', BidiClass::NonspacingMark), // Mn       BATAK VOWEL SIGN KARO O
    ('\u{1bee}', '\u{1bee}', BidiClass::LeftToRight), // Mc       BATAK VOWEL SIGN U
    ('\u{1bef}', '\u{1bf1}', BidiClass::NonspacingMark), // Mn   [3] BATAK VOWEL SIGN U FOR SIMALUNGUN SA..BATAK CONSONANT SIGN H
    ('\u{1bf2}', '\u{1bf3}', BidiClass::LeftToRight), // Mc   [2] BATAK PANGOLAT..BATAK PANONGONAN
    ('\u{1bfc}', '\u{1bff}', BidiClass::LeftToRight), // Po   [4] BATAK SYMBOL BINDU NA METEK..BATAK SYMBOL BINDU PANGOLAT
    ('\u{1c00}', '\u{1c23}', BidiClass::LeftToRight), // Lo  [36] LEPCHA LETTER KA..LEPCHA LETTER A
    ('\u{1c24}', '\u{1c2b}', BidiClass::LeftToRight), // Mc   [8] LEPCHA SUBJOINED LETTER YA..LEPCHA VOWEL SIGN UU
    ('\u{1c2c}', '\u{1c33}', BidiClass::NonspacingMark), // Mn   [8] LEPCHA VOWEL SIGN E..LEPCHA CONSONANT SIGN T
    ('\u{1c34}', '\u{1c35}', BidiClass::LeftToRight), // Mc   [2] LEPCHA CONSONANT SIGN NYIN-DO..LEPCHA CONSONANT SIGN KANG
    ('\u{1c36}', '\u{1c37}', BidiClass::NonspacingMark), // Mn   [2] LEPCHA SIGN RAN..LEPCHA SIGN NUKTA
    ('\u{1c3b}', '\u{1c3f}', BidiClass::LeftToRight), // Po   [5] LEPCHA PUNCTUATION TA-ROL..LEPCHA PUNCTUATION TSHOOK
    ('\u{1c40}', '\u{1c49}', BidiClass::LeftToRight), // Nd  [10] LEPCHA DIGIT ZERO..LEPCHA DIGIT NINE
    ('\u{1c4d}', '\u{1c4f}', BidiClass::LeftToRight), // Lo   [3] LEPCHA LETTER TTA..LEPCHA LETTER DDA
    ('\u{1c50}', '\u{1c59}', BidiClass::LeftToRight), // Nd  [10] OL CHIKI DIGIT ZERO..OL CHIKI DIGIT NINE
    ('\u{1c5a}', '\u{1c77}', BidiClass::LeftToRight), // Lo  [30] OL CHIKI LETTER LA..OL CHIKI LETTER OH
    ('\u{1c78}', '\u{1c7d}', BidiClass::LeftToRight), // Lm   [6] OL CHIKI MU TTUDDAG..OL CHIKI AHAD
    ('\u{1c7e}', '\u{1c7f}', BidiClass::LeftToRight), // Po   [2] OL CHIKI PUNCTUATION MUCAAD..OL CHIKI PUNCTUATION DOUBLE MUCAAD
    ('\u{1c80}', '\u{1c88}', BidiClass::LeftToRight), // L&   [9] CYRILLIC SMALL LETTER ROUNDED VE..CYRILLIC SMALL LETTER UNBLENDED UK
    ('\u{1c90}', '\u{1cba}', BidiClass::LeftToRight), // L&  [43] GEORGIAN MTAVRULI CAPITAL LETTER AN..GEORGIAN MTAVRULI CAPITAL LETTER AIN
    ('\u{1cbd}', '\u{1cbf}', BidiClass::LeftToRight), // L&   [3] GEORGIAN MTAVRULI CAPITAL LETTER AEN..GEORGIAN MTAVRULI CAPITAL LETTER LABIAL SIGN
    ('\u{1cc0}', '\u{1cc7}', BidiClass::LeftToRight), // Po   [8] SUNDANESE PUNCTUATION BINDU SURYA..SUNDANESE PUNCTUATION BINDU BA SATANGA
    ('\u{1cd0}', '\u{1cd2}', BidiClass::NonspacingMark), // Mn   [3] VEDIC TONE KARSHANA..VEDIC TONE PRENKHA
    ('\u{1cd3}', '\u{1cd3}', BidiClass::LeftToRight),    // Po       VEDIC SIGN NIHSHVASA
    ('\u{1cd4}', '\u{1ce0}', BidiClass::NonspacingMark), // Mn  [13] VEDIC SIGN YAJURVEDIC MIDLINE SVARITA..VEDIC TONE RIGVEDIC KASHMIRI INDEPENDENT SVARITA
    ('\u{1ce1}', '\u{1ce1}', BidiClass::LeftToRight), // Mc       VEDIC TONE ATHARVAVEDIC INDEPENDENT SVARITA
    ('\u{1ce2}', '\u{1ce8}', BidiClass::NonspacingMark), // Mn   [7] VEDIC SIGN VISARGA SVARITA..VEDIC SIGN VISARGA ANUDATTA WITH TAIL
    ('\u{1ce9}', '\u{1cec}', BidiClass::LeftToRight), // Lo   [4] VEDIC SIGN ANUSVARA ANTARGOMUKHA..VEDIC SIGN ANUSVARA VAMAGOMUKHA WITH TAIL
    ('\u{1ced}', '\u{1ced}', BidiClass::NonspacingMark), // Mn       VEDIC SIGN TIRYAK
    ('\u{1cee}', '\u{1cf3}', BidiClass::LeftToRight), // Lo   [6] VEDIC SIGN HEXIFORM LONG ANUSVARA..VEDIC SIGN ROTATED ARDHAVISARGA
    ('\u{1cf4}', '\u{1cf4}', BidiClass::NonspacingMark), // Mn       VEDIC TONE CANDRA ABOVE
    ('\u{1cf5}', '\u{1cf6}', BidiClass::LeftToRight), // Lo   [2] VEDIC SIGN JIHVAMULIYA..VEDIC SIGN UPADHMANIYA
    ('\u{1cf7}', '\u{1cf7}', BidiClass::LeftToRight), // Mc       VEDIC SIGN ATIKRAMA
    ('\u{1cf8}', '\u{1cf9}', BidiClass::NonspacingMark), // Mn   [2] VEDIC TONE RING ABOVE..VEDIC TONE DOUBLE RING ABOVE
    ('\u{1cfa}', '\u{1cfa}', BidiClass::LeftToRight), // Lo       VEDIC SIGN DOUBLE ANUSVARA ANTARGOMUKHA
    ('\u{1d00}', '\u{1d2b}', BidiClass::LeftToRight), // L&  [44] LATIN LETTER SMALL CAPITAL A..CYRILLIC LETTER SMALL CAPITAL EL
    ('\u{1d2c}', '\u{1d6a}', BidiClass::LeftToRight), // Lm  [63] MODIFIER LETTER CAPITAL A..GREEK SUBSCRIPT SMALL LETTER CHI
    ('\u{1d6b}', '\u{1d77}', BidiClass::LeftToRight), // L&  [13] LATIN SMALL LETTER UE..LATIN SMALL LETTER TURNED G
    ('\u{1d78}', '\u{1d78}', BidiClass::LeftToRight), // Lm       MODIFIER LETTER CYRILLIC EN
    ('\u{1d79}', '\u{1d9a}', BidiClass::LeftToRight), // L&  [34] LATIN SMALL LETTER INSULAR G..LATIN SMALL LETTER EZH WITH RETROFLEX HOOK
    ('\u{1d9b}', '\u{1dbf}', BidiClass::LeftToRight), // Lm  [37] MODIFIER LETTER SMALL TURNED ALPHA..MODIFIER LETTER SMALL THETA
    ('\u{1dc0}', '\u{1dff}', BidiClass::NonspacingMark), // Mn  [64] COMBINING DOTTED GRAVE ACCENT..COMBINING RIGHT ARROWHEAD AND DOWN ARROWHEAD BELOW
    ('\u{1e00}', '\u{1f15}', BidiClass::LeftToRight), // L& [278] LATIN CAPITAL LETTER A WITH RING BELOW..GREEK SMALL LETTER EPSILON WITH DASIA AND OXIA
    ('\u{1f18}', '\u{1f1d}', BidiClass::LeftToRight), // L&   [6] GREEK CAPITAL LETTER EPSILON WITH PSILI..GREEK CAPITAL LETTER EPSILON WITH DASIA AND OXIA
    ('\u{1f20}', '\u{1f45}', BidiClass::LeftToRight), // L&  [38] GREEK SMALL LETTER ETA WITH PSILI..GREEK SMALL LETTER OMICRON WITH DASIA AND OXIA
    ('\u{1f48}', '\u{1f4d}', BidiClass::LeftToRight), // L&   [6] GREEK CAPITAL LETTER OMICRON WITH PSILI..GREEK CAPITAL LETTER OMICRON WITH DASIA AND OXIA
    ('\u{1f50}', '\u{1f57}', BidiClass::LeftToRight), // L&   [8] GREEK SMALL LETTER UPSILON WITH PSILI..GREEK SMALL LETTER UPSILON WITH DASIA AND PERISPOMENI
    ('\u{1f59}', '\u{1f59}', BidiClass::LeftToRight), // L&       GREEK CAPITAL LETTER UPSILON WITH DASIA
    ('\u{1f5b}', '\u{1f5b}', BidiClass::LeftToRight), // L&       GREEK CAPITAL LETTER UPSILON WITH DASIA AND VARIA
    ('\u{1f5d}', '\u{1f5d}', BidiClass::LeftToRight), // L&       GREEK CAPITAL LETTER UPSILON WITH DASIA AND OXIA
    ('\u{1f5f}', '\u{1f7d}', BidiClass::LeftToRight), // L&  [31] GREEK CAPITAL LETTER UPSILON WITH DASIA AND PERISPOMENI..GREEK SMALL LETTER OMEGA WITH OXIA
    ('\u{1f80}', '\u{1fb4}', BidiClass::LeftToRight), // L&  [53] GREEK SMALL LETTER ALPHA WITH PSILI AND YPOGEGRAMMENI..GREEK SMALL LETTER ALPHA WITH OXIA AND YPOGEGRAMMENI
    ('\u{1fb6}', '\u{1fbc}', BidiClass::LeftToRight), // L&   [7] GREEK SMALL LETTER ALPHA WITH PERISPOMENI..GREEK CAPITAL LETTER ALPHA WITH PROSGEGRAMMENI
    ('\u{1fbd}', '\u{1fbd}', BidiClass::OtherNeutral), // Sk       GREEK KORONIS
    ('\u{1fbe}', '\u{1fbe}', BidiClass::LeftToRight), // L&       GREEK PROSGEGRAMMENI
    ('\u{1fbf}', '\u{1fc1}', BidiClass::OtherNeutral), // Sk   [3] GREEK PSILI..GREEK DIALYTIKA AND PERISPOMENI
    ('\u{1fc2}', '\u{1fc4}', BidiClass::LeftToRight), // L&   [3] GREEK SMALL LETTER ETA WITH VARIA AND YPOGEGRAMMENI..GREEK SMALL LETTER ETA WITH OXIA AND YPOGEGRAMMENI
    ('\u{1fc6}', '\u{1fcc}', BidiClass::LeftToRight), // L&   [7] GREEK SMALL LETTER ETA WITH PERISPOMENI..GREEK CAPITAL LETTER ETA WITH PROSGEGRAMMENI
    ('\u{1fcd}', '\u{1fcf}', BidiClass::OtherNeutral), // Sk   [3] GREEK PSILI AND VARIA..GREEK PSILI AND PERISPOMENI
    ('\u{1fd0}', '\u{1fd3}', BidiClass::LeftToRight), // L&   [4] GREEK SMALL LETTER IOTA WITH VRACHY..GREEK SMALL LETTER IOTA WITH DIALYTIKA AND OXIA
    ('\u{1fd6}', '\u{1fdb}', BidiClass::LeftToRight), // L&   [6] GREEK SMALL LETTER IOTA WITH PERISPOMENI..GREEK CAPITAL LETTER IOTA WITH OXIA
    ('\u{1fdd}', '\u{1fdf}', BidiClass::OtherNeutral), // Sk   [3] GREEK DASIA AND VARIA..GREEK DASIA AND PERISPOMENI
    ('\u{1fe0}', '\u{1fec}', BidiClass::LeftToRight), // L&  [13] GREEK SMALL LETTER UPSILON WITH VRACHY..GREEK CAPITAL LETTER RHO WITH DASIA
    ('\u{1fed}', '\u{1fef}', BidiClass::OtherNeutral), // Sk   [3] GREEK DIALYTIKA AND VARIA..GREEK VARIA
    ('\u{1ff2}', '\u{1ff4}', BidiClass::LeftToRight), // L&   [3] GREEK SMALL LETTER OMEGA WITH VARIA AND YPOGEGRAMMENI..GREEK SMALL LETTER OMEGA WITH OXIA AND YPOGEGRAMMENI
    ('\u{1ff6}', '\u{1ffc}', BidiClass::LeftToRight), // L&   [7] GREEK SMALL LETTER OMEGA WITH PERISPOMENI..GREEK CAPITAL LETTER OMEGA WITH PROSGEGRAMMENI
    ('\u{1ffd}', '\u{1ffe}', BidiClass::OtherNeutral), // Sk   [2] GREEK OXIA..GREEK DASIA
    ('\u{2000}', '\u{200a}', BidiClass::WhiteSpace),  // Zs  [11] EN QUAD..HAIR SPACE
    ('\u{200b}', '\u{200d}', BidiClass::BoundaryNeutral), // Cf   [3] ZERO WIDTH SPACE..ZERO WIDTH JOINER
    ('\u{200e}', '\u{200e}', BidiClass::LeftToRight),     // Cf       LEFT-TO-RIGHT MARK
    ('\u{200f}', '\u{200f}', BidiClass::RightToLeft),     // Cf       RIGHT-TO-LEFT MARK
    ('\u{2010}', '\u{2015}', BidiClass::OtherNeutral),    // Pd   [6] HYPHEN..HORIZONTAL BAR
    ('\u{2016}', '\u{2017}', BidiClass::OtherNeutral), // Po   [2] DOUBLE VERTICAL LINE..DOUBLE LOW LINE
    ('\u{2018}', '\u{2018}', BidiClass::OtherNeutral), // Pi       LEFT SINGLE QUOTATION MARK
    ('\u{2019}', '\u{2019}', BidiClass::OtherNeutral), // Pf       RIGHT SINGLE QUOTATION MARK
    ('\u{201a}', '\u{201a}', BidiClass::OtherNeutral), // Ps       SINGLE LOW-9 QUOTATION MARK
    ('\u{201b}', '\u{201c}', BidiClass::OtherNeutral), // Pi   [2] SINGLE HIGH-REVERSED-9 QUOTATION MARK..LEFT DOUBLE QUOTATION MARK
    ('\u{201d}', '\u{201d}', BidiClass::OtherNeutral), // Pf       RIGHT DOUBLE QUOTATION MARK
    ('\u{201e}', '\u{201e}', BidiClass::OtherNeutral), // Ps       DOUBLE LOW-9 QUOTATION MARK
    ('\u{201f}', '\u{201f}', BidiClass::OtherNeutral), // Pi       DOUBLE HIGH-REVERSED-9 QUOTATION MARK
    ('\u{2020}', '\u{2027}', BidiClass::OtherNeutral), // Po   [8] DAGGER..HYPHENATION POINT
    ('\u{2028}', '\u{2028}', BidiClass::WhiteSpace),   // Zl       LINE SEPARATOR
    ('\u{2029}', '\u{2029}', BidiClass::ParagraphSeparator), // Zp       PARAGRAPH SEPARATOR
    ('\u{202a}', '\u{202a}', BidiClass::LeftToRightEmbedding), // Cf       LEFT-TO-RIGHT EMBEDDING
    ('\u{202b}', '\u{202b}', BidiClass::RightToLeftEmbedding), // Cf       RIGHT-TO-LEFT EMBEDDING
    ('\u{202c}', '\u{202c}', BidiClass::PopDirectionalFormat), // Cf       POP DIRECTIONAL FORMATTING
    ('\u{202d}', '\u{202d}', BidiClass::LeftToRightOverride),  // Cf       LEFT-TO-RIGHT OVERRIDE
    ('\u{202e}', '\u{202e}', BidiClass::RightToLeftOverride),  // Cf       RIGHT-TO-LEFT OVERRIDE
    ('\u{202f}', '\u{202f}', BidiClass::CommonSeparator),      // Zs       NARROW NO-BREAK SPACE
    ('\u{2030}', '\u{2034}', BidiClass::EuropeanTerminator), // Po   [5] PER MILLE SIGN..TRIPLE PRIME
    ('\u{2035}', '\u{2038}', BidiClass::OtherNeutral),       // Po   [4] REVERSED PRIME..CARET
    ('\u{2039}', '\u{2039}', BidiClass::OtherNeutral), // Pi       SINGLE LEFT-POINTING ANGLE QUOTATION MARK
    ('\u{203a}', '\u{203a}', BidiClass::OtherNeutral), // Pf       SINGLE RIGHT-POINTING ANGLE QUOTATION MARK
    ('\u{203b}', '\u{203e}', BidiClass::OtherNeutral), // Po   [4] REFERENCE MARK..OVERLINE
    ('\u{203f}', '\u{2040}', BidiClass::OtherNeutral), // Pc   [2] UNDERTIE..CHARACTER TIE
    ('\u{2041}', '\u{2043}', BidiClass::OtherNeutral), // Po   [3] CARET INSERTION POINT..HYPHEN BULLET
    ('\u{2044}', '\u{2044}', BidiClass::CommonSeparator), // Sm       FRACTION SLASH
    ('\u{2045}', '\u{2045}', BidiClass::OtherNeutral), // Ps       LEFT SQUARE BRACKET WITH QUILL
    ('\u{2046}', '\u{2046}', BidiClass::OtherNeutral), // Pe       RIGHT SQUARE BRACKET WITH QUILL
    ('\u{2047}', '\u{2051}', BidiClass::OtherNeutral), // Po  [11] DOUBLE QUESTION MARK..TWO ASTERISKS ALIGNED VERTICALLY
    ('\u{2052}', '\u{2052}', BidiClass::OtherNeutral), // Sm       COMMERCIAL MINUS SIGN
    ('\u{2053}', '\u{2053}', BidiClass::OtherNeutral), // Po       SWUNG DASH
    ('\u{2054}', '\u{2054}', BidiClass::OtherNeutral), // Pc       INVERTED UNDERTIE
    ('\u{2055}', '\u{205e}', BidiClass::OtherNeutral), // Po  [10] FLOWER PUNCTUATION MARK..VERTICAL FOUR DOTS
    ('\u{205f}', '\u{205f}', BidiClass::WhiteSpace),   // Zs       MEDIUM MATHEMATICAL SPACE
    ('\u{2060}', '\u{2064}', BidiClass::BoundaryNeutral), // Cf   [5] WORD JOINER..INVISIBLE PLUS
    ('\u{2065}', '\u{2065}', BidiClass::BoundaryNeutral), // Cn       <reserved-2065>
    ('\u{2066}', '\u{2066}', BidiClass::LeftToRightIsolate), // Cf       LEFT-TO-RIGHT ISOLATE
    ('\u{2067}', '\u{2067}', BidiClass::RightToLeftIsolate), // Cf       RIGHT-TO-LEFT ISOLATE
    ('\u{2068}', '\u{2068}', BidiClass::FirstStrongIsolate), // Cf       FIRST STRONG ISOLATE
    ('\u{2069}', '\u{2069}', BidiClass::PopDirectionalIsolate), // Cf       POP DIRECTIONAL ISOLATE
    ('\u{206a}', '\u{206f}', BidiClass::BoundaryNeutral), // Cf   [6] INHIBIT SYMMETRIC SWAPPING..NOMINAL DIGIT SHAPES
    ('\u{2070}', '\u{2070}', BidiClass::EuropeanNumber),  // No       SUPERSCRIPT ZERO
    ('\u{2071}', '\u{2071}', BidiClass::LeftToRight), // Lm       SUPERSCRIPT LATIN SMALL LETTER I
    ('\u{2074}', '\u{2079}', BidiClass::EuropeanNumber), // No   [6] SUPERSCRIPT FOUR..SUPERSCRIPT NINE
    ('\u{207a}', '\u{207b}', BidiClass::EuropeanSeparator), // Sm   [2] SUPERSCRIPT PLUS SIGN..SUPERSCRIPT MINUS
    ('\u{207c}', '\u{207c}', BidiClass::OtherNeutral),      // Sm       SUPERSCRIPT EQUALS SIGN
    ('\u{207d}', '\u{207d}', BidiClass::OtherNeutral),      // Ps       SUPERSCRIPT LEFT PARENTHESIS
    ('\u{207e}', '\u{207e}', BidiClass::OtherNeutral), // Pe       SUPERSCRIPT RIGHT PARENTHESIS
    ('\u{207f}', '\u{207f}', BidiClass::LeftToRight),  // Lm       SUPERSCRIPT LATIN SMALL LETTER N
    ('\u{2080}', '\u{2089}', BidiClass::EuropeanNumber), // No  [10] SUBSCRIPT ZERO..SUBSCRIPT NINE
    ('\u{208a}', '\u{208b}', BidiClass::EuropeanSeparator), // Sm   [2] SUBSCRIPT PLUS SIGN..SUBSCRIPT MINUS
    ('\u{208c}', '\u{208c}', BidiClass::OtherNeutral),      // Sm       SUBSCRIPT EQUALS SIGN
    ('\u{208d}', '\u{208d}', BidiClass::OtherNeutral),      // Ps       SUBSCRIPT LEFT PARENTHESIS
    ('\u{208e}', '\u{208e}', BidiClass::OtherNeutral),      // Pe       SUBSCRIPT RIGHT PARENTHESIS
    ('\u{2090}', '\u{209c}', BidiClass::LeftToRight), // Lm  [13] LATIN SUBSCRIPT SMALL LETTER A..LATIN SUBSCRIPT SMALL LETTER T
    ('\u{20a0}', '\u{20c0}', BidiClass::EuropeanTerminator), // Sc  [33] EURO-CURRENCY SIGN..SOM SIGN
    ('\u{20c1}', '\u{20cf}', BidiClass::EuropeanTerminator), // Cn  [15] <reserved-20C1>..<reserved-20CF>
    ('\u{20d0}', '\u{20dc}', BidiClass::NonspacingMark), // Mn  [13] COMBINING LEFT HARPOON ABOVE..COMBINING FOUR DOTS ABOVE
    ('\u{20dd}', '\u{20e0}', BidiClass::NonspacingMark), // Me   [4] COMBINING ENCLOSING CIRCLE..COMBINING ENCLOSING CIRCLE BACKSLASH
    ('\u{20e1}', '\u{20e1}', BidiClass::NonspacingMark), // Mn       COMBINING LEFT RIGHT ARROW ABOVE
    ('\u{20e2}', '\u{20e4}', BidiClass::NonspacingMark), // Me   [3] COMBINING ENCLOSING SCREEN..COMBINING ENCLOSING UPWARD POINTING TRIANGLE
    ('\u{20e5}', '\u{20f0}', BidiClass::NonspacingMark), // Mn  [12] COMBINING REVERSE SOLIDUS OVERLAY..COMBINING ASTERISK ABOVE
    ('\u{2100}', '\u{2101}', BidiClass::OtherNeutral), // So   [2] ACCOUNT OF..ADDRESSED TO THE SUBJECT
    ('\u{2102}', '\u{2102}', BidiClass::LeftToRight),  // L&       DOUBLE-STRUCK CAPITAL C
    ('\u{2103}', '\u{2106}', BidiClass::OtherNeutral), // So   [4] DEGREE CELSIUS..CADA UNA
    ('\u{2107}', '\u{2107}', BidiClass::LeftToRight),  // L&       EULER CONSTANT
    ('\u{2108}', '\u{2109}', BidiClass::OtherNeutral), // So   [2] SCRUPLE..DEGREE FAHRENHEIT
    ('\u{210a}', '\u{2113}', BidiClass::LeftToRight),  // L&  [10] SCRIPT SMALL G..SCRIPT SMALL L
    ('\u{2114}', '\u{2114}', BidiClass::OtherNeutral), // So       L B BAR SYMBOL
    ('\u{2115}', '\u{2115}', BidiClass::LeftToRight),  // L&       DOUBLE-STRUCK CAPITAL N
    ('\u{2116}', '\u{2117}', BidiClass::OtherNeutral), // So   [2] NUMERO SIGN..SOUND RECORDING COPYRIGHT
    ('\u{2118}', '\u{2118}', BidiClass::OtherNeutral), // Sm       SCRIPT CAPITAL P
    ('\u{2119}', '\u{211d}', BidiClass::LeftToRight), // L&   [5] DOUBLE-STRUCK CAPITAL P..DOUBLE-STRUCK CAPITAL R
    ('\u{211e}', '\u{2123}', BidiClass::OtherNeutral), // So   [6] PRESCRIPTION TAKE..VERSICLE
    ('\u{2124}', '\u{2124}', BidiClass::LeftToRight), // L&       DOUBLE-STRUCK CAPITAL Z
    ('\u{2125}', '\u{2125}', BidiClass::OtherNeutral), // So       OUNCE SIGN
    ('\u{2126}', '\u{2126}', BidiClass::LeftToRight), // L&       OHM SIGN
    ('\u{2127}', '\u{2127}', BidiClass::OtherNeutral), // So       INVERTED OHM SIGN
    ('\u{2128}', '\u{2128}', BidiClass::LeftToRight), // L&       BLACK-LETTER CAPITAL Z
    ('\u{2129}', '\u{2129}', BidiClass::OtherNeutral), // So       TURNED GREEK SMALL LETTER IOTA
    ('\u{212a}', '\u{212d}', BidiClass::LeftToRight), // L&   [4] KELVIN SIGN..BLACK-LETTER CAPITAL C
    ('\u{212e}', '\u{212e}', BidiClass::EuropeanTerminator), // So       ESTIMATED SYMBOL
    ('\u{212f}', '\u{2134}', BidiClass::LeftToRight), // L&   [6] SCRIPT SMALL E..SCRIPT SMALL O
    ('\u{2135}', '\u{2138}', BidiClass::LeftToRight), // Lo   [4] ALEF SYMBOL..DALET SYMBOL
    ('\u{2139}', '\u{2139}', BidiClass::LeftToRight), // L&       INFORMATION SOURCE
    ('\u{213a}', '\u{213b}', BidiClass::OtherNeutral), // So   [2] ROTATED CAPITAL Q..FACSIMILE SIGN
    ('\u{213c}', '\u{213f}', BidiClass::LeftToRight), // L&   [4] DOUBLE-STRUCK SMALL PI..DOUBLE-STRUCK CAPITAL PI
    ('\u{2140}', '\u{2144}', BidiClass::OtherNeutral), // Sm   [5] DOUBLE-STRUCK N-ARY SUMMATION..TURNED SANS-SERIF CAPITAL Y
    ('\u{2145}', '\u{2149}', BidiClass::LeftToRight), // L&   [5] DOUBLE-STRUCK ITALIC CAPITAL D..DOUBLE-STRUCK ITALIC SMALL J
    ('\u{214a}', '\u{214a}', BidiClass::OtherNeutral), // So       PROPERTY LINE
    ('\u{214b}', '\u{214b}', BidiClass::OtherNeutral), // Sm       TURNED AMPERSAND
    ('\u{214c}', '\u{214d}', BidiClass::OtherNeutral), // So   [2] PER SIGN..AKTIESELSKAB
    ('\u{214e}', '\u{214e}', BidiClass::LeftToRight), // L&       TURNED SMALL F
    ('\u{214f}', '\u{214f}', BidiClass::LeftToRight), // So       SYMBOL FOR SAMARITAN SOURCE
    ('\u{2150}', '\u{215f}', BidiClass::OtherNeutral), // No  [16] VULGAR FRACTION ONE SEVENTH..FRACTION NUMERATOR ONE
    ('\u{2160}', '\u{2182}', BidiClass::LeftToRight), // Nl  [35] ROMAN NUMERAL ONE..ROMAN NUMERAL TEN THOUSAND
    ('\u{2183}', '\u{2184}', BidiClass::LeftToRight), // L&   [2] ROMAN NUMERAL REVERSED ONE HUNDRED..LATIN SMALL LETTER REVERSED C
    ('\u{2185}', '\u{2188}', BidiClass::LeftToRight), // Nl   [4] ROMAN NUMERAL SIX LATE FORM..ROMAN NUMERAL ONE HUNDRED THOUSAND
    ('\u{2189}', '\u{2189}', BidiClass::OtherNeutral), // No       VULGAR FRACTION ZERO THIRDS
    ('\u{218a}', '\u{218b}', BidiClass::OtherNeutral), // So   [2] TURNED DIGIT TWO..TURNED DIGIT THREE
    ('\u{2190}', '\u{2194}', BidiClass::OtherNeutral), // Sm   [5] LEFTWARDS ARROW..LEFT RIGHT ARROW
    ('\u{2195}', '\u{2199}', BidiClass::OtherNeutral), // So   [5] UP DOWN ARROW..SOUTH WEST ARROW
    ('\u{219a}', '\u{219b}', BidiClass::OtherNeutral), // Sm   [2] LEFTWARDS ARROW WITH STROKE..RIGHTWARDS ARROW WITH STROKE
    ('\u{219c}', '\u{219f}', BidiClass::OtherNeutral), // So   [4] LEFTWARDS WAVE ARROW..UPWARDS TWO HEADED ARROW
    ('\u{21a0}', '\u{21a0}', BidiClass::OtherNeutral), // Sm       RIGHTWARDS TWO HEADED ARROW
    ('\u{21a1}', '\u{21a2}', BidiClass::OtherNeutral), // So   [2] DOWNWARDS TWO HEADED ARROW..LEFTWARDS ARROW WITH TAIL
    ('\u{21a3}', '\u{21a3}', BidiClass::OtherNeutral), // Sm       RIGHTWARDS ARROW WITH TAIL
    ('\u{21a4}', '\u{21a5}', BidiClass::OtherNeutral), // So   [2] LEFTWARDS ARROW FROM BAR..UPWARDS ARROW FROM BAR
    ('\u{21a6}', '\u{21a6}', BidiClass::OtherNeutral), // Sm       RIGHTWARDS ARROW FROM BAR
    ('\u{21a7}', '\u{21ad}', BidiClass::OtherNeutral), // So   [7] DOWNWARDS ARROW FROM BAR..LEFT RIGHT WAVE ARROW
    ('\u{21ae}', '\u{21ae}', BidiClass::OtherNeutral), // Sm       LEFT RIGHT ARROW WITH STROKE
    ('\u{21af}', '\u{21cd}', BidiClass::OtherNeutral), // So  [31] DOWNWARDS ZIGZAG ARROW..LEFTWARDS DOUBLE ARROW WITH STROKE
    ('\u{21ce}', '\u{21cf}', BidiClass::OtherNeutral), // Sm   [2] LEFT RIGHT DOUBLE ARROW WITH STROKE..RIGHTWARDS DOUBLE ARROW WITH STROKE
    ('\u{21d0}', '\u{21d1}', BidiClass::OtherNeutral), // So   [2] LEFTWARDS DOUBLE ARROW..UPWARDS DOUBLE ARROW
    ('\u{21d2}', '\u{21d2}', BidiClass::OtherNeutral), // Sm       RIGHTWARDS DOUBLE ARROW
    ('\u{21d3}', '\u{21d3}', BidiClass::OtherNeutral), // So       DOWNWARDS DOUBLE ARROW
    ('\u{21d4}', '\u{21d4}', BidiClass::OtherNeutral), // Sm       LEFT RIGHT DOUBLE ARROW
    ('\u{21d5}', '\u{21f3}', BidiClass::OtherNeutral), // So  [31] UP DOWN DOUBLE ARROW..UP DOWN WHITE ARROW
    ('\u{21f4}', '\u{2211}', BidiClass::OtherNeutral), // Sm  [30] RIGHT ARROW WITH SMALL CIRCLE..N-ARY SUMMATION
    ('\u{2212}', '\u{2212}', BidiClass::EuropeanSeparator), // Sm       MINUS SIGN
    ('\u{2213}', '\u{2213}', BidiClass::EuropeanTerminator), // Sm       MINUS-OR-PLUS SIGN
    ('\u{2214}', '\u{22ff}', BidiClass::OtherNeutral), // Sm [236] DOT PLUS..Z NOTATION BAG MEMBERSHIP
    ('\u{2300}', '\u{2307}', BidiClass::OtherNeutral), // So   [8] DIAMETER SIGN..WAVY LINE
    ('\u{2308}', '\u{2308}', BidiClass::OtherNeutral), // Ps       LEFT CEILING
    ('\u{2309}', '\u{2309}', BidiClass::OtherNeutral), // Pe       RIGHT CEILING
    ('\u{230a}', '\u{230a}', BidiClass::OtherNeutral), // Ps       LEFT FLOOR
    ('\u{230b}', '\u{230b}', BidiClass::OtherNeutral), // Pe       RIGHT FLOOR
    ('\u{230c}', '\u{231f}', BidiClass::OtherNeutral), // So  [20] BOTTOM RIGHT CROP..BOTTOM RIGHT CORNER
    ('\u{2320}', '\u{2321}', BidiClass::OtherNeutral), // Sm   [2] TOP HALF INTEGRAL..BOTTOM HALF INTEGRAL
    ('\u{2322}', '\u{2328}', BidiClass::OtherNeutral), // So   [7] FROWN..KEYBOARD
    ('\u{2329}', '\u{2329}', BidiClass::OtherNeutral), // Ps       LEFT-POINTING ANGLE BRACKET
    ('\u{232a}', '\u{232a}', BidiClass::OtherNeutral), // Pe       RIGHT-POINTING ANGLE BRACKET
    ('\u{232b}', '\u{2335}', BidiClass::OtherNeutral), // So  [11] ERASE TO THE LEFT..COUNTERSINK
    ('\u{2336}', '\u{237a}', BidiClass::LeftToRight), // So  [69] APL FUNCTIONAL SYMBOL I-BEAM..APL FUNCTIONAL SYMBOL ALPHA
    ('\u{237b}', '\u{237b}', BidiClass::OtherNeutral), // So       NOT CHECK MARK
    ('\u{237c}', '\u{237c}', BidiClass::OtherNeutral), // Sm       RIGHT ANGLE WITH DOWNWARDS ZIGZAG ARROW
    ('\u{237d}', '\u{2394}', BidiClass::OtherNeutral), // So  [24] SHOULDERED OPEN BOX..SOFTWARE-FUNCTION SYMBOL
    ('\u{2395}', '\u{2395}', BidiClass::LeftToRight),  // So       APL FUNCTIONAL SYMBOL QUAD
    ('\u{2396}', '\u{239a}', BidiClass::OtherNeutral), // So   [5] DECIMAL SEPARATOR KEY SYMBOL..CLEAR SCREEN SYMBOL
    ('\u{239b}', '\u{23b3}', BidiClass::OtherNeutral), // Sm  [25] LEFT PARENTHESIS UPPER HOOK..SUMMATION BOTTOM
    ('\u{23b4}', '\u{23db}', BidiClass::OtherNeutral), // So  [40] TOP SQUARE BRACKET..FUSE
    ('\u{23dc}', '\u{23e1}', BidiClass::OtherNeutral), // Sm   [6] TOP PARENTHESIS..BOTTOM TORTOISE SHELL BRACKET
    ('\u{23e2}', '\u{2426}', BidiClass::OtherNeutral), // So  [69] WHITE TRAPEZIUM..SYMBOL FOR SUBSTITUTE FORM TWO
    ('\u{2440}', '\u{244a}', BidiClass::OtherNeutral), // So  [11] OCR HOOK..OCR DOUBLE BACKSLASH
    ('\u{2460}', '\u{2487}', BidiClass::OtherNeutral), // No  [40] CIRCLED DIGIT ONE..PARENTHESIZED NUMBER TWENTY
    ('\u{2488}', '\u{249b}', BidiClass::EuropeanNumber), // No  [20] DIGIT ONE FULL STOP..NUMBER TWENTY FULL STOP
    ('\u{249c}', '\u{24e9}', BidiClass::LeftToRight), // So  [78] PARENTHESIZED LATIN SMALL LETTER A..CIRCLED LATIN SMALL LETTER Z
    ('\u{24ea}', '\u{24ff}', BidiClass::OtherNeutral), // No  [22] CIRCLED DIGIT ZERO..NEGATIVE CIRCLED DIGIT ZERO
    ('\u{2500}', '\u{25b6}', BidiClass::OtherNeutral), // So [183] BOX DRAWINGS LIGHT HORIZONTAL..BLACK RIGHT-POINTING TRIANGLE
    ('\u{25b7}', '\u{25b7}', BidiClass::OtherNeutral), // Sm       WHITE RIGHT-POINTING TRIANGLE
    ('\u{25b8}', '\u{25c0}', BidiClass::OtherNeutral), // So   [9] BLACK RIGHT-POINTING SMALL TRIANGLE..BLACK LEFT-POINTING TRIANGLE
    ('\u{25c1}', '\u{25c1}', BidiClass::OtherNeutral), // Sm       WHITE LEFT-POINTING TRIANGLE
    ('\u{25c2}', '\u{25f7}', BidiClass::OtherNeutral), // So  [54] BLACK LEFT-POINTING SMALL TRIANGLE..WHITE CIRCLE WITH UPPER RIGHT QUADRANT
    ('\u{25f8}', '\u{25ff}', BidiClass::OtherNeutral), // Sm   [8] UPPER LEFT TRIANGLE..LOWER RIGHT TRIANGLE
    ('\u{2600}', '\u{266e}', BidiClass::OtherNeutral), // So [111] BLACK SUN WITH RAYS..MUSIC NATURAL SIGN
    ('\u{266f}', '\u{266f}', BidiClass::OtherNeutral), // Sm       MUSIC SHARP SIGN
    ('\u{2670}', '\u{26ab}', BidiClass::OtherNeutral), // So  [60] WEST SYRIAC CROSS..MEDIUM BLACK CIRCLE
    ('\u{26ac}', '\u{26ac}', BidiClass::LeftToRight),  // So       MEDIUM SMALL WHITE CIRCLE
    ('\u{26ad}', '\u{2767}', BidiClass::OtherNeutral), // So [187] MARRIAGE SYMBOL..ROTATED FLORAL HEART BULLET
    ('\u{2768}', '\u{2768}', BidiClass::OtherNeutral), // Ps       MEDIUM LEFT PARENTHESIS ORNAMENT
    ('\u{2769}', '\u{2769}', BidiClass::OtherNeutral), // Pe       MEDIUM RIGHT PARENTHESIS ORNAMENT
    ('\u{276a}', '\u{276a}', BidiClass::OtherNeutral), // Ps       MEDIUM FLATTENED LEFT PARENTHESIS ORNAMENT
    ('\u{276b}', '\u{276b}', BidiClass::OtherNeutral), // Pe       MEDIUM FLATTENED RIGHT PARENTHESIS ORNAMENT
    ('\u{276c}', '\u{276c}', BidiClass::OtherNeutral), // Ps       MEDIUM LEFT-POINTING ANGLE BRACKET ORNAMENT
    ('\u{276d}', '\u{276d}', BidiClass::OtherNeutral), // Pe       MEDIUM RIGHT-POINTING ANGLE BRACKET ORNAMENT
    ('\u{276e}', '\u{276e}', BidiClass::OtherNeutral), // Ps       HEAVY LEFT-POINTING ANGLE QUOTATION MARK ORNAMENT
    ('\u{276f}', '\u{276f}', BidiClass::OtherNeutral), // Pe       HEAVY RIGHT-POINTING ANGLE QUOTATION MARK ORNAMENT
    ('\u{2770}', '\u{2770}', BidiClass::OtherNeutral), // Ps       HEAVY LEFT-POINTING ANGLE BRACKET ORNAMENT
    ('\u{2771}', '\u{2771}', BidiClass::OtherNeutral), // Pe       HEAVY RIGHT-POINTING ANGLE BRACKET ORNAMENT
    ('\u{2772}', '\u{2772}', BidiClass::OtherNeutral), // Ps       LIGHT LEFT TORTOISE SHELL BRACKET ORNAMENT
    ('\u{2773}', '\u{2773}', BidiClass::OtherNeutral), // Pe       LIGHT RIGHT TORTOISE SHELL BRACKET ORNAMENT
    ('\u{2774}', '\u{2774}', BidiClass::OtherNeutral), // Ps       MEDIUM LEFT CURLY BRACKET ORNAMENT
    ('\u{2775}', '\u{2775}', BidiClass::OtherNeutral), // Pe       MEDIUM RIGHT CURLY BRACKET ORNAMENT
    ('\u{2776}', '\u{2793}', BidiClass::OtherNeutral), // No  [30] DINGBAT NEGATIVE CIRCLED DIGIT ONE..DINGBAT NEGATIVE CIRCLED SANS-SERIF NUMBER TEN
    ('\u{2794}', '\u{27bf}', BidiClass::OtherNeutral), // So  [44] HEAVY WIDE-HEADED RIGHTWARDS ARROW..DOUBLE CURLY LOOP
    ('\u{27c0}', '\u{27c4}', BidiClass::OtherNeutral), // Sm   [5] THREE DIMENSIONAL ANGLE..OPEN SUPERSET
    ('\u{27c5}', '\u{27c5}', BidiClass::OtherNeutral), // Ps       LEFT S-SHAPED BAG DELIMITER
    ('\u{27c6}', '\u{27c6}', BidiClass::OtherNeutral), // Pe       RIGHT S-SHAPED BAG DELIMITER
    ('\u{27c7}', '\u{27e5}', BidiClass::OtherNeutral), // Sm  [31] OR WITH DOT INSIDE..WHITE SQUARE WITH RIGHTWARDS TICK
    ('\u{27e6}', '\u{27e6}', BidiClass::OtherNeutral), // Ps       MATHEMATICAL LEFT WHITE SQUARE BRACKET
    ('\u{27e7}', '\u{27e7}', BidiClass::OtherNeutral), // Pe       MATHEMATICAL RIGHT WHITE SQUARE BRACKET
    ('\u{27e8}', '\u{27e8}', BidiClass::OtherNeutral), // Ps       MATHEMATICAL LEFT ANGLE BRACKET
    ('\u{27e9}', '\u{27e9}', BidiClass::OtherNeutral), // Pe       MATHEMATICAL RIGHT ANGLE BRACKET
    ('\u{27ea}', '\u{27ea}', BidiClass::OtherNeutral), // Ps       MATHEMATICAL LEFT DOUBLE ANGLE BRACKET
    ('\u{27eb}', '\u{27eb}', BidiClass::OtherNeutral), // Pe       MATHEMATICAL RIGHT DOUBLE ANGLE BRACKET
    ('\u{27ec}', '\u{27ec}', BidiClass::OtherNeutral), // Ps       MATHEMATICAL LEFT WHITE TORTOISE SHELL BRACKET
    ('\u{27ed}', '\u{27ed}', BidiClass::OtherNeutral), // Pe       MATHEMATICAL RIGHT WHITE TORTOISE SHELL BRACKET
    ('\u{27ee}', '\u{27ee}', BidiClass::OtherNeutral), // Ps       MATHEMATICAL LEFT FLATTENED PARENTHESIS
    ('\u{27ef}', '\u{27ef}', BidiClass::OtherNeutral), // Pe       MATHEMATICAL RIGHT FLATTENED PARENTHESIS
    ('\u{27f0}', '\u{27ff}', BidiClass::OtherNeutral), // Sm  [16] UPWARDS QUADRUPLE ARROW..LONG RIGHTWARDS SQUIGGLE ARROW
    ('\u{2800}', '\u{28ff}', BidiClass::LeftToRight), // So [256] BRAILLE PATTERN BLANK..BRAILLE PATTERN DOTS-12345678
    ('\u{2900}', '\u{2982}', BidiClass::OtherNeutral), // Sm [131] RIGHTWARDS TWO-HEADED ARROW WITH VERTICAL STROKE..Z NOTATION TYPE COLON
    ('\u{2983}', '\u{2983}', BidiClass::OtherNeutral), // Ps       LEFT WHITE CURLY BRACKET
    ('\u{2984}', '\u{2984}', BidiClass::OtherNeutral), // Pe       RIGHT WHITE CURLY BRACKET
    ('\u{2985}', '\u{2985}', BidiClass::OtherNeutral), // Ps       LEFT WHITE PARENTHESIS
    ('\u{2986}', '\u{2986}', BidiClass::OtherNeutral), // Pe       RIGHT WHITE PARENTHESIS
    ('\u{2987}', '\u{2987}', BidiClass::OtherNeutral), // Ps       Z NOTATION LEFT IMAGE BRACKET
    ('\u{2988}', '\u{2988}', BidiClass::OtherNeutral), // Pe       Z NOTATION RIGHT IMAGE BRACKET
    ('\u{2989}', '\u{2989}', BidiClass::OtherNeutral), // Ps       Z NOTATION LEFT BINDING BRACKET
    ('\u{298a}', '\u{298a}', BidiClass::OtherNeutral), // Pe       Z NOTATION RIGHT BINDING BRACKET
    ('\u{298b}', '\u{298b}', BidiClass::OtherNeutral), // Ps       LEFT SQUARE BRACKET WITH UNDERBAR
    ('\u{298c}', '\u{298c}', BidiClass::OtherNeutral), // Pe       RIGHT SQUARE BRACKET WITH UNDERBAR
    ('\u{298d}', '\u{298d}', BidiClass::OtherNeutral), // Ps       LEFT SQUARE BRACKET WITH TICK IN TOP CORNER
    ('\u{298e}', '\u{298e}', BidiClass::OtherNeutral), // Pe       RIGHT SQUARE BRACKET WITH TICK IN BOTTOM CORNER
    ('\u{298f}', '\u{298f}', BidiClass::OtherNeutral), // Ps       LEFT SQUARE BRACKET WITH TICK IN BOTTOM CORNER
    ('\u{2990}', '\u{2990}', BidiClass::OtherNeutral), // Pe       RIGHT SQUARE BRACKET WITH TICK IN TOP CORNER
    ('\u{2991}', '\u{2991}', BidiClass::OtherNeutral), // Ps       LEFT ANGLE BRACKET WITH DOT
    ('\u{2992}', '\u{2992}', BidiClass::OtherNeutral), // Pe       RIGHT ANGLE BRACKET WITH DOT
    ('\u{2993}', '\u{2993}', BidiClass::OtherNeutral), // Ps       LEFT ARC LESS-THAN BRACKET
    ('\u{2994}', '\u{2994}', BidiClass::OtherNeutral), // Pe       RIGHT ARC GREATER-THAN BRACKET
    ('\u{2995}', '\u{2995}', BidiClass::OtherNeutral), // Ps       DOUBLE LEFT ARC GREATER-THAN BRACKET
    ('\u{2996}', '\u{2996}', BidiClass::OtherNeutral), // Pe       DOUBLE RIGHT ARC LESS-THAN BRACKET
    ('\u{2997}', '\u{2997}', BidiClass::OtherNeutral), // Ps       LEFT BLACK TORTOISE SHELL BRACKET
    ('\u{2998}', '\u{2998}', BidiClass::OtherNeutral), // Pe       RIGHT BLACK TORTOISE SHELL BRACKET
    ('\u{2999}', '\u{29d7}', BidiClass::OtherNeutral), // Sm  [63] DOTTED FENCE..BLACK HOURGLASS
    ('\u{29d8}', '\u{29d8}', BidiClass::OtherNeutral), // Ps       LEFT WIGGLY FENCE
    ('\u{29d9}', '\u{29d9}', BidiClass::OtherNeutral), // Pe       RIGHT WIGGLY FENCE
    ('\u{29da}', '\u{29da}', BidiClass::OtherNeutral), // Ps       LEFT DOUBLE WIGGLY FENCE
    ('\u{29db}', '\u{29db}', BidiClass::OtherNeutral), // Pe       RIGHT DOUBLE WIGGLY FENCE
    ('\u{29dc}', '\u{29fb}', BidiClass::OtherNeutral), // Sm  [32] INCOMPLETE INFINITY..TRIPLE PLUS
    ('\u{29fc}', '\u{29fc}', BidiClass::OtherNeutral), // Ps       LEFT-POINTING CURVED ANGLE BRACKET
    ('\u{29fd}', '\u{29fd}', BidiClass::OtherNeutral), // Pe       RIGHT-POINTING CURVED ANGLE BRACKET
    ('\u{29fe}', '\u{2aff}', BidiClass::OtherNeutral), // Sm [258] TINY..N-ARY WHITE VERTICAL BAR
    ('\u{2b00}', '\u{2b2f}', BidiClass::OtherNeutral), // So  [48] NORTH EAST WHITE ARROW..WHITE VERTICAL ELLIPSE
    ('\u{2b30}', '\u{2b44}', BidiClass::OtherNeutral), // Sm  [21] LEFT ARROW WITH SMALL CIRCLE..RIGHTWARDS ARROW THROUGH SUPERSET
    ('\u{2b45}', '\u{2b46}', BidiClass::OtherNeutral), // So   [2] LEFTWARDS QUADRUPLE ARROW..RIGHTWARDS QUADRUPLE ARROW
    ('\u{2b47}', '\u{2b4c}', BidiClass::OtherNeutral), // Sm   [6] REVERSE TILDE OPERATOR ABOVE RIGHTWARDS ARROW..RIGHTWARDS ARROW ABOVE REVERSE TILDE OPERATOR
    ('\u{2b4d}', '\u{2b73}', BidiClass::OtherNeutral), // So  [39] DOWNWARDS TRIANGLE-HEADED ZIGZAG ARROW..DOWNWARDS TRIANGLE-HEADED ARROW TO BAR
    ('\u{2b76}', '\u{2b95}', BidiClass::OtherNeutral), // So  [32] NORTH WEST TRIANGLE-HEADED ARROW TO BAR..RIGHTWARDS BLACK ARROW
    ('\u{2b97}', '\u{2bff}', BidiClass::OtherNeutral), // So [105] SYMBOL FOR TYPE A ELECTRONICS..HELLSCHREIBER PAUSE SYMBOL
    ('\u{2c00}', '\u{2c7b}', BidiClass::LeftToRight), // L& [124] GLAGOLITIC CAPITAL LETTER AZU..LATIN LETTER SMALL CAPITAL TURNED E
    ('\u{2c7c}', '\u{2c7d}', BidiClass::LeftToRight), // Lm   [2] LATIN SUBSCRIPT SMALL LETTER J..MODIFIER LETTER CAPITAL V
    ('\u{2c7e}', '\u{2ce4}', BidiClass::LeftToRight), // L& [103] LATIN CAPITAL LETTER S WITH SWASH TAIL..COPTIC SYMBOL KAI
    ('\u{2ce5}', '\u{2cea}', BidiClass::OtherNeutral), // So   [6] COPTIC SYMBOL MI RO..COPTIC SYMBOL SHIMA SIMA
    ('\u{2ceb}', '\u{2cee}', BidiClass::LeftToRight), // L&   [4] COPTIC CAPITAL LETTER CRYPTOGRAMMIC SHEI..COPTIC SMALL LETTER CRYPTOGRAMMIC GANGIA
    ('\u{2cef}', '\u{2cf1}', BidiClass::NonspacingMark), // Mn   [3] COPTIC COMBINING NI ABOVE..COPTIC COMBINING SPIRITUS LENIS
    ('\u{2cf2}', '\u{2cf3}', BidiClass::LeftToRight), // L&   [2] COPTIC CAPITAL LETTER BOHAIRIC KHEI..COPTIC SMALL LETTER BOHAIRIC KHEI
    ('\u{2cf9}', '\u{2cfc}', BidiClass::OtherNeutral), // Po   [4] COPTIC OLD NUBIAN FULL STOP..COPTIC OLD NUBIAN VERSE DIVIDER
    ('\u{2cfd}', '\u{2cfd}', BidiClass::OtherNeutral), // No       COPTIC FRACTION ONE HALF
    ('\u{2cfe}', '\u{2cff}', BidiClass::OtherNeutral), // Po   [2] COPTIC FULL STOP..COPTIC MORPHOLOGICAL DIVIDER
    ('\u{2d00}', '\u{2d25}', BidiClass::LeftToRight), // L&  [38] GEORGIAN SMALL LETTER AN..GEORGIAN SMALL LETTER HOE
    ('\u{2d27}', '\u{2d27}', BidiClass::LeftToRight), // L&       GEORGIAN SMALL LETTER YN
    ('\u{2d2d}', '\u{2d2d}', BidiClass::LeftToRight), // L&       GEORGIAN SMALL LETTER AEN
    ('\u{2d30}', '\u{2d67}', BidiClass::LeftToRight), // Lo  [56] TIFINAGH LETTER YA..TIFINAGH LETTER YO
    ('\u{2d6f}', '\u{2d6f}', BidiClass::LeftToRight), // Lm       TIFINAGH MODIFIER LETTER LABIALIZATION MARK
    ('\u{2d70}', '\u{2d70}', BidiClass::LeftToRight), // Po       TIFINAGH SEPARATOR MARK
    ('\u{2d7f}', '\u{2d7f}', BidiClass::NonspacingMark), // Mn       TIFINAGH CONSONANT JOINER
    ('\u{2d80}', '\u{2d96}', BidiClass::LeftToRight), // Lo  [23] ETHIOPIC SYLLABLE LOA..ETHIOPIC SYLLABLE GGWE
    ('\u{2da0}', '\u{2da6}', BidiClass::LeftToRight), // Lo   [7] ETHIOPIC SYLLABLE SSA..ETHIOPIC SYLLABLE SSO
    ('\u{2da8}', '\u{2dae}', BidiClass::LeftToRight), // Lo   [7] ETHIOPIC SYLLABLE CCA..ETHIOPIC SYLLABLE CCO
    ('\u{2db0}', '\u{2db6}', BidiClass::LeftToRight), // Lo   [7] ETHIOPIC SYLLABLE ZZA..ETHIOPIC SYLLABLE ZZO
    ('\u{2db8}', '\u{2dbe}', BidiClass::LeftToRight), // Lo   [7] ETHIOPIC SYLLABLE CCHA..ETHIOPIC SYLLABLE CCHO
    ('\u{2dc0}', '\u{2dc6}', BidiClass::LeftToRight), // Lo   [7] ETHIOPIC SYLLABLE QYA..ETHIOPIC SYLLABLE QYO
    ('\u{2dc8}', '\u{2dce}', BidiClass::LeftToRight), // Lo   [7] ETHIOPIC SYLLABLE KYA..ETHIOPIC SYLLABLE KYO
    ('\u{2dd0}', '\u{2dd6}', BidiClass::LeftToRight), // Lo   [7] ETHIOPIC SYLLABLE XYA..ETHIOPIC SYLLABLE XYO
    ('\u{2dd8}', '\u{2dde}', BidiClass::LeftToRight), // Lo   [7] ETHIOPIC SYLLABLE GYA..ETHIOPIC SYLLABLE GYO
    ('\u{2de0}', '\u{2dff}', BidiClass::NonspacingMark), // Mn  [32] COMBINING CYRILLIC LETTER BE..COMBINING CYRILLIC LETTER IOTIFIED BIG YUS
    ('\u{2e00}', '\u{2e01}', BidiClass::OtherNeutral), // Po   [2] RIGHT ANGLE SUBSTITUTION MARKER..RIGHT ANGLE DOTTED SUBSTITUTION MARKER
    ('\u{2e02}', '\u{2e02}', BidiClass::OtherNeutral), // Pi       LEFT SUBSTITUTION BRACKET
    ('\u{2e03}', '\u{2e03}', BidiClass::OtherNeutral), // Pf       RIGHT SUBSTITUTION BRACKET
    ('\u{2e04}', '\u{2e04}', BidiClass::OtherNeutral), // Pi       LEFT DOTTED SUBSTITUTION BRACKET
    ('\u{2e05}', '\u{2e05}', BidiClass::OtherNeutral), // Pf       RIGHT DOTTED SUBSTITUTION BRACKET
    ('\u{2e06}', '\u{2e08}', BidiClass::OtherNeutral), // Po   [3] RAISED INTERPOLATION MARKER..DOTTED TRANSPOSITION MARKER
    ('\u{2e09}', '\u{2e09}', BidiClass::OtherNeutral), // Pi       LEFT TRANSPOSITION BRACKET
    ('\u{2e0a}', '\u{2e0a}', BidiClass::OtherNeutral), // Pf       RIGHT TRANSPOSITION BRACKET
    ('\u{2e0b}', '\u{2e0b}', BidiClass::OtherNeutral), // Po       RAISED SQUARE
    ('\u{2e0c}', '\u{2e0c}', BidiClass::OtherNeutral), // Pi       LEFT RAISED OMISSION BRACKET
    ('\u{2e0d}', '\u{2e0d}', BidiClass::OtherNeutral), // Pf       RIGHT RAISED OMISSION BRACKET
    ('\u{2e0e}', '\u{2e16}', BidiClass::OtherNeutral), // Po   [9] EDITORIAL CORONIS..DOTTED RIGHT-POINTING ANGLE
    ('\u{2e17}', '\u{2e17}', BidiClass::OtherNeutral), // Pd       DOUBLE OBLIQUE HYPHEN
    ('\u{2e18}', '\u{2e19}', BidiClass::OtherNeutral), // Po   [2] INVERTED INTERROBANG..PALM BRANCH
    ('\u{2e1a}', '\u{2e1a}', BidiClass::OtherNeutral), // Pd       HYPHEN WITH DIAERESIS
    ('\u{2e1b}', '\u{2e1b}', BidiClass::OtherNeutral), // Po       TILDE WITH RING ABOVE
    ('\u{2e1c}', '\u{2e1c}', BidiClass::OtherNeutral), // Pi       LEFT LOW PARAPHRASE BRACKET
    ('\u{2e1d}', '\u{2e1d}', BidiClass::OtherNeutral), // Pf       RIGHT LOW PARAPHRASE BRACKET
    ('\u{2e1e}', '\u{2e1f}', BidiClass::OtherNeutral), // Po   [2] TILDE WITH DOT ABOVE..TILDE WITH DOT BELOW
    ('\u{2e20}', '\u{2e20}', BidiClass::OtherNeutral), // Pi       LEFT VERTICAL BAR WITH QUILL
    ('\u{2e21}', '\u{2e21}', BidiClass::OtherNeutral), // Pf       RIGHT VERTICAL BAR WITH QUILL
    ('\u{2e22}', '\u{2e22}', BidiClass::OtherNeutral), // Ps       TOP LEFT HALF BRACKET
    ('\u{2e23}', '\u{2e23}', BidiClass::OtherNeutral), // Pe       TOP RIGHT HALF BRACKET
    ('\u{2e24}', '\u{2e24}', BidiClass::OtherNeutral), // Ps       BOTTOM LEFT HALF BRACKET
    ('\u{2e25}', '\u{2e25}', BidiClass::OtherNeutral), // Pe       BOTTOM RIGHT HALF BRACKET
    ('\u{2e26}', '\u{2e26}', BidiClass::OtherNeutral), // Ps       LEFT SIDEWAYS U BRACKET
    ('\u{2e27}', '\u{2e27}', BidiClass::OtherNeutral), // Pe       RIGHT SIDEWAYS U BRACKET
    ('\u{2e28}', '\u{2e28}', BidiClass::OtherNeutral), // Ps       LEFT DOUBLE PARENTHESIS
    ('\u{2e29}', '\u{2e29}', BidiClass::OtherNeutral), // Pe       RIGHT DOUBLE PARENTHESIS
    ('\u{2e2a}', '\u{2e2e}', BidiClass::OtherNeutral), // Po   [5] TWO DOTS OVER ONE DOT PUNCTUATION..REVERSED QUESTION MARK
    ('\u{2e2f}', '\u{2e2f}', BidiClass::OtherNeutral), // Lm       VERTICAL TILDE
    ('\u{2e30}', '\u{2e39}', BidiClass::OtherNeutral), // Po  [10] RING POINT..TOP HALF SECTION SIGN
    ('\u{2e3a}', '\u{2e3b}', BidiClass::OtherNeutral), // Pd   [2] TWO-EM DASH..THREE-EM DASH
    ('\u{2e3c}', '\u{2e3f}', BidiClass::OtherNeutral), // Po   [4] STENOGRAPHIC FULL STOP..CAPITULUM
    ('\u{2e40}', '\u{2e40}', BidiClass::OtherNeutral), // Pd       DOUBLE HYPHEN
    ('\u{2e41}', '\u{2e41}', BidiClass::OtherNeutral), // Po       REVERSED COMMA
    ('\u{2e42}', '\u{2e42}', BidiClass::OtherNeutral), // Ps       DOUBLE LOW-REVERSED-9 QUOTATION MARK
    ('\u{2e43}', '\u{2e4f}', BidiClass::OtherNeutral), // Po  [13] DASH WITH LEFT UPTURN..CORNISH VERSE DIVIDER
    ('\u{2e50}', '\u{2e51}', BidiClass::OtherNeutral), // So   [2] CROSS PATTY WITH RIGHT CROSSBAR..CROSS PATTY WITH LEFT CROSSBAR
    ('\u{2e52}', '\u{2e54}', BidiClass::OtherNeutral), // Po   [3] TIRONIAN SIGN CAPITAL ET..MEDIEVAL QUESTION MARK
    ('\u{2e55}', '\u{2e55}', BidiClass::OtherNeutral), // Ps       LEFT SQUARE BRACKET WITH STROKE
    ('\u{2e56}', '\u{2e56}', BidiClass::OtherNeutral), // Pe       RIGHT SQUARE BRACKET WITH STROKE
    ('\u{2e57}', '\u{2e57}', BidiClass::OtherNeutral), // Ps       LEFT SQUARE BRACKET WITH DOUBLE STROKE
    ('\u{2e58}', '\u{2e58}', BidiClass::OtherNeutral), // Pe       RIGHT SQUARE BRACKET WITH DOUBLE STROKE
    ('\u{2e59}', '\u{2e59}', BidiClass::OtherNeutral), // Ps       TOP HALF LEFT PARENTHESIS
    ('\u{2e5a}', '\u{2e5a}', BidiClass::OtherNeutral), // Pe       TOP HALF RIGHT PARENTHESIS
    ('\u{2e5b}', '\u{2e5b}', BidiClass::OtherNeutral), // Ps       BOTTOM HALF LEFT PARENTHESIS
    ('\u{2e5c}', '\u{2e5c}', BidiClass::OtherNeutral), // Pe       BOTTOM HALF RIGHT PARENTHESIS
    ('\u{2e5d}', '\u{2e5d}', BidiClass::OtherNeutral), // Pd       OBLIQUE HYPHEN
    ('\u{2e80}', '\u{2e99}', BidiClass::OtherNeutral), // So  [26] CJK RADICAL REPEAT..CJK RADICAL RAP
    ('\u{2e9b}', '\u{2ef3}', BidiClass::OtherNeutral), // So  [89] CJK RADICAL CHOKE..CJK RADICAL C-SIMPLIFIED TURTLE
    ('\u{2f00}', '\u{2fd5}', BidiClass::OtherNeutral), // So [214] KANGXI RADICAL ONE..KANGXI RADICAL FLUTE
    ('\u{2ff0}', '\u{2ffb}', BidiClass::OtherNeutral), // So  [12] IDEOGRAPHIC DESCRIPTION CHARACTER LEFT TO RIGHT..IDEOGRAPHIC DESCRIPTION CHARACTER OVERLAID
    ('\u{3000}', '\u{3000}', BidiClass::WhiteSpace),   // Zs       IDEOGRAPHIC SPACE
    ('\u{3001}', '\u{3003}', BidiClass::OtherNeutral), // Po   [3] IDEOGRAPHIC COMMA..DITTO MARK
    ('\u{3004}', '\u{3004}', BidiClass::OtherNeutral), // So       JAPANESE INDUSTRIAL STANDARD SYMBOL
    ('\u{3005}', '\u{3005}', BidiClass::LeftToRight),  // Lm       IDEOGRAPHIC ITERATION MARK
    ('\u{3006}', '\u{3006}', BidiClass::LeftToRight),  // Lo       IDEOGRAPHIC CLOSING MARK
    ('\u{3007}', '\u{3007}', BidiClass::LeftToRight),  // Nl       IDEOGRAPHIC NUMBER ZERO
    ('\u{3008}', '\u{3008}', BidiClass::OtherNeutral), // Ps       LEFT ANGLE BRACKET
    ('\u{3009}', '\u{3009}', BidiClass::OtherNeutral), // Pe       RIGHT ANGLE BRACKET
    ('\u{300a}', '\u{300a}', BidiClass::OtherNeutral), // Ps       LEFT DOUBLE ANGLE BRACKET
    ('\u{300b}', '\u{300b}', BidiClass::OtherNeutral), // Pe       RIGHT DOUBLE ANGLE BRACKET
    ('\u{300c}', '\u{300c}', BidiClass::OtherNeutral), // Ps       LEFT CORNER BRACKET
    ('\u{300d}', '\u{300d}', BidiClass::OtherNeutral), // Pe       RIGHT CORNER BRACKET
    ('\u{300e}', '\u{300e}', BidiClass::OtherNeutral), // Ps       LEFT WHITE CORNER BRACKET
    ('\u{300f}', '\u{300f}', BidiClass::OtherNeutral), // Pe       RIGHT WHITE CORNER BRACKET
    ('\u{3010}', '\u{3010}', BidiClass::OtherNeutral), // Ps       LEFT BLACK LENTICULAR BRACKET
    ('\u{3011}', '\u{3011}', BidiClass::OtherNeutral), // Pe       RIGHT BLACK LENTICULAR BRACKET
    ('\u{3012}', '\u{3013}', BidiClass::OtherNeutral), // So   [2] POSTAL MARK..GETA MARK
    ('\u{3014}', '\u{3014}', BidiClass::OtherNeutral), // Ps       LEFT TORTOISE SHELL BRACKET
    ('\u{3015}', '\u{3015}', BidiClass::OtherNeutral), // Pe       RIGHT TORTOISE SHELL BRACKET
    ('\u{3016}', '\u{3016}', BidiClass::OtherNeutral), // Ps       LEFT WHITE LENTICULAR BRACKET
    ('\u{3017}', '\u{3017}', BidiClass::OtherNeutral), // Pe       RIGHT WHITE LENTICULAR BRACKET
    ('\u{3018}', '\u{3018}', BidiClass::OtherNeutral), // Ps       LEFT WHITE TORTOISE SHELL BRACKET
    ('\u{3019}', '\u{3019}', BidiClass::OtherNeutral), // Pe       RIGHT WHITE TORTOISE SHELL BRACKET
    ('\u{301a}', '\u{301a}', BidiClass::OtherNeutral), // Ps       LEFT WHITE SQUARE BRACKET
    ('\u{301b}', '\u{301b}', BidiClass::OtherNeutral), // Pe       RIGHT WHITE SQUARE BRACKET
    ('\u{301c}', '\u{301c}', BidiClass::OtherNeutral), // Pd       WAVE DASH
    ('\u{301d}', '\u{301d}', BidiClass::OtherNeutral), // Ps       REVERSED DOUBLE PRIME QUOTATION MARK
    ('\u{301e}', '\u{301f}', BidiClass::OtherNeutral), // Pe   [2] DOUBLE PRIME QUOTATION MARK..LOW DOUBLE PRIME QUOTATION MARK
    ('\u{3020}', '\u{3020}', BidiClass::OtherNeutral), // So       POSTAL MARK FACE
    ('\u{3021}', '\u{3029}', BidiClass::LeftToRight), // Nl   [9] HANGZHOU NUMERAL ONE..HANGZHOU NUMERAL NINE
    ('\u{302a}', '\u{302d}', BidiClass::NonspacingMark), // Mn   [4] IDEOGRAPHIC LEVEL TONE MARK..IDEOGRAPHIC ENTERING TONE MARK
    ('\u{302e}', '\u{302f}', BidiClass::LeftToRight), // Mc   [2] HANGUL SINGLE DOT TONE MARK..HANGUL DOUBLE DOT TONE MARK
    ('\u{3030}', '\u{3030}', BidiClass::OtherNeutral), // Pd       WAVY DASH
    ('\u{3031}', '\u{3035}', BidiClass::LeftToRight), // Lm   [5] VERTICAL KANA REPEAT MARK..VERTICAL KANA REPEAT MARK LOWER HALF
    ('\u{3036}', '\u{3037}', BidiClass::OtherNeutral), // So   [2] CIRCLED POSTAL MARK..IDEOGRAPHIC TELEGRAPH LINE FEED SEPARATOR SYMBOL
    ('\u{3038}', '\u{303a}', BidiClass::LeftToRight), // Nl   [3] HANGZHOU NUMERAL TEN..HANGZHOU NUMERAL THIRTY
    ('\u{303b}', '\u{303b}', BidiClass::LeftToRight), // Lm       VERTICAL IDEOGRAPHIC ITERATION MARK
    ('\u{303c}', '\u{303c}', BidiClass::LeftToRight), // Lo       MASU MARK
    ('\u{303d}', '\u{303d}', BidiClass::OtherNeutral), // Po       PART ALTERNATION MARK
    ('\u{303e}', '\u{303f}', BidiClass::OtherNeutral), // So   [2] IDEOGRAPHIC VARIATION INDICATOR..IDEOGRAPHIC HALF FILL SPACE
    ('\u{3041}', '\u{3096}', BidiClass::LeftToRight), // Lo  [86] HIRAGANA LETTER SMALL A..HIRAGANA LETTER SMALL KE
    ('\u{3099}', '\u{309a}', BidiClass::NonspacingMark), // Mn   [2] COMBINING KATAKANA-HIRAGANA VOICED SOUND MARK..COMBINING KATAKANA-HIRAGANA SEMI-VOICED SOUND MARK
    ('\u{309b}', '\u{309c}', BidiClass::OtherNeutral), // Sk   [2] KATAKANA-HIRAGANA VOICED SOUND MARK..KATAKANA-HIRAGANA SEMI-VOICED SOUND MARK
    ('\u{309d}', '\u{309e}', BidiClass::LeftToRight), // Lm   [2] HIRAGANA ITERATION MARK..HIRAGANA VOICED ITERATION MARK
    ('\u{309f}', '\u{309f}', BidiClass::LeftToRight), // Lo       HIRAGANA DIGRAPH YORI
    ('\u{30a0}', '\u{30a0}', BidiClass::OtherNeutral), // Pd       KATAKANA-HIRAGANA DOUBLE HYPHEN
    ('\u{30a1}', '\u{30fa}', BidiClass::LeftToRight), // Lo  [90] KATAKANA LETTER SMALL A..KATAKANA LETTER VO
    ('\u{30fb}', '\u{30fb}', BidiClass::OtherNeutral), // Po       KATAKANA MIDDLE DOT
    ('\u{30fc}', '\u{30fe}', BidiClass::LeftToRight), // Lm   [3] KATAKANA-HIRAGANA PROLONGED SOUND MARK..KATAKANA VOICED ITERATION MARK
    ('\u{30ff}', '\u{30ff}', BidiClass::LeftToRight), // Lo       KATAKANA DIGRAPH KOTO
    ('\u{3105}', '\u{312f}', BidiClass::LeftToRight), // Lo  [43] BOPOMOFO LETTER B..BOPOMOFO LETTER NN
    ('\u{3131}', '\u{318e}', BidiClass::LeftToRight), // Lo  [94] HANGUL LETTER KIYEOK..HANGUL LETTER ARAEAE
    ('\u{3190}', '\u{3191}', BidiClass::LeftToRight), // So   [2] IDEOGRAPHIC ANNOTATION LINKING MARK..IDEOGRAPHIC ANNOTATION REVERSE MARK
    ('\u{3192}', '\u{3195}', BidiClass::LeftToRight), // No   [4] IDEOGRAPHIC ANNOTATION ONE MARK..IDEOGRAPHIC ANNOTATION FOUR MARK
    ('\u{3196}', '\u{319f}', BidiClass::LeftToRight), // So  [10] IDEOGRAPHIC ANNOTATION TOP MARK..IDEOGRAPHIC ANNOTATION MAN MARK
    ('\u{31a0}', '\u{31bf}', BidiClass::LeftToRight), // Lo  [32] BOPOMOFO LETTER BU..BOPOMOFO LETTER AH
    ('\u{31c0}', '\u{31e3}', BidiClass::OtherNeutral), // So  [36] CJK STROKE T..CJK STROKE Q
    ('\u{31f0}', '\u{31ff}', BidiClass::LeftToRight), // Lo  [16] KATAKANA LETTER SMALL KU..KATAKANA LETTER SMALL RO
    ('\u{3200}', '\u{321c}', BidiClass::LeftToRight), // So  [29] PARENTHESIZED HANGUL KIYEOK..PARENTHESIZED HANGUL CIEUC U
    ('\u{321d}', '\u{321e}', BidiClass::OtherNeutral), // So   [2] PARENTHESIZED KOREAN CHARACTER OJEON..PARENTHESIZED KOREAN CHARACTER O HU
    ('\u{3220}', '\u{3229}', BidiClass::LeftToRight), // No  [10] PARENTHESIZED IDEOGRAPH ONE..PARENTHESIZED IDEOGRAPH TEN
    ('\u{322a}', '\u{3247}', BidiClass::LeftToRight), // So  [30] PARENTHESIZED IDEOGRAPH MOON..CIRCLED IDEOGRAPH KOTO
    ('\u{3248}', '\u{324f}', BidiClass::LeftToRight), // No   [8] CIRCLED NUMBER TEN ON BLACK SQUARE..CIRCLED NUMBER EIGHTY ON BLACK SQUARE
    ('\u{3250}', '\u{3250}', BidiClass::OtherNeutral), // So       PARTNERSHIP SIGN
    ('\u{3251}', '\u{325f}', BidiClass::OtherNeutral), // No  [15] CIRCLED NUMBER TWENTY ONE..CIRCLED NUMBER THIRTY FIVE
    ('\u{3260}', '\u{327b}', BidiClass::LeftToRight), // So  [28] CIRCLED HANGUL KIYEOK..CIRCLED HANGUL HIEUH A
    ('\u{327c}', '\u{327e}', BidiClass::OtherNeutral), // So   [3] CIRCLED KOREAN CHARACTER CHAMKO..CIRCLED HANGUL IEUNG U
    ('\u{327f}', '\u{327f}', BidiClass::LeftToRight),  // So       KOREAN STANDARD SYMBOL
    ('\u{3280}', '\u{3289}', BidiClass::LeftToRight), // No  [10] CIRCLED IDEOGRAPH ONE..CIRCLED IDEOGRAPH TEN
    ('\u{328a}', '\u{32b0}', BidiClass::LeftToRight), // So  [39] CIRCLED IDEOGRAPH MOON..CIRCLED IDEOGRAPH NIGHT
    ('\u{32b1}', '\u{32bf}', BidiClass::OtherNeutral), // No  [15] CIRCLED NUMBER THIRTY SIX..CIRCLED NUMBER FIFTY
    ('\u{32c0}', '\u{32cb}', BidiClass::LeftToRight), // So  [12] IDEOGRAPHIC TELEGRAPH SYMBOL FOR JANUARY..IDEOGRAPHIC TELEGRAPH SYMBOL FOR DECEMBER
    ('\u{32cc}', '\u{32cf}', BidiClass::OtherNeutral), // So   [4] SQUARE HG..LIMITED LIABILITY SIGN
    ('\u{32d0}', '\u{3376}', BidiClass::LeftToRight), // So [167] CIRCLED KATAKANA A..SQUARE PC
    ('\u{3377}', '\u{337a}', BidiClass::OtherNeutral), // So   [4] SQUARE DM..SQUARE IU
    ('\u{337b}', '\u{33dd}', BidiClass::LeftToRight), // So  [99] SQUARE ERA NAME HEISEI..SQUARE WB
    ('\u{33de}', '\u{33df}', BidiClass::OtherNeutral), // So   [2] SQUARE V OVER M..SQUARE A OVER M
    ('\u{33e0}', '\u{33fe}', BidiClass::LeftToRight), // So  [31] IDEOGRAPHIC TELEGRAPH SYMBOL FOR DAY ONE..IDEOGRAPHIC TELEGRAPH SYMBOL FOR DAY THIRTY-ONE
    ('\u{33ff}', '\u{33ff}', BidiClass::OtherNeutral), // So       SQUARE GAL
    ('\u{3400}', '\u{4dbf}', BidiClass::LeftToRight), // Lo [6592] CJK UNIFIED IDEOGRAPH-3400..CJK UNIFIED IDEOGRAPH-4DBF
    ('\u{4dc0}', '\u{4dff}', BidiClass::OtherNeutral), // So  [64] HEXAGRAM FOR THE CREATIVE HEAVEN..HEXAGRAM FOR BEFORE COMPLETION
    ('\u{4e00}', '\u{a014}', BidiClass::LeftToRight), // Lo [21013] CJK UNIFIED IDEOGRAPH-4E00..YI SYLLABLE E
    ('\u{a015}', '\u{a015}', BidiClass::LeftToRight), // Lm       YI SYLLABLE WU
    ('\u{a016}', '\u{a48c}', BidiClass::LeftToRight), // Lo [1143] YI SYLLABLE BIT..YI SYLLABLE YYR
    ('\u{a490}', '\u{a4c6}', BidiClass::OtherNeutral), // So  [55] YI RADICAL QOT..YI RADICAL KE
    ('\u{a4d0}', '\u{a4f7}', BidiClass::LeftToRight), // Lo  [40] LISU LETTER BA..LISU LETTER OE
    ('\u{a4f8}', '\u{a4fd}', BidiClass::LeftToRight), // Lm   [6] LISU LETTER TONE MYA TI..LISU LETTER TONE MYA JEU
    ('\u{a4fe}', '\u{a4ff}', BidiClass::LeftToRight), // Po   [2] LISU PUNCTUATION COMMA..LISU PUNCTUATION FULL STOP
    ('\u{a500}', '\u{a60b}', BidiClass::LeftToRight), // Lo [268] VAI SYLLABLE EE..VAI SYLLABLE NG
    ('\u{a60c}', '\u{a60c}', BidiClass::LeftToRight), // Lm       VAI SYLLABLE LENGTHENER
    ('\u{a60d}', '\u{a60f}', BidiClass::OtherNeutral), // Po   [3] VAI COMMA..VAI QUESTION MARK
    ('\u{a610}', '\u{a61f}', BidiClass::LeftToRight), // Lo  [16] VAI SYLLABLE NDOLE FA..VAI SYMBOL JONG
    ('\u{a620}', '\u{a629}', BidiClass::LeftToRight), // Nd  [10] VAI DIGIT ZERO..VAI DIGIT NINE
    ('\u{a62a}', '\u{a62b}', BidiClass::LeftToRight), // Lo   [2] VAI SYLLABLE NDOLE MA..VAI SYLLABLE NDOLE DO
    ('\u{a640}', '\u{a66d}', BidiClass::LeftToRight), // L&  [46] CYRILLIC CAPITAL LETTER ZEMLYA..CYRILLIC SMALL LETTER DOUBLE MONOCULAR O
    ('\u{a66e}', '\u{a66e}', BidiClass::LeftToRight), // Lo       CYRILLIC LETTER MULTIOCULAR O
    ('\u{a66f}', '\u{a66f}', BidiClass::NonspacingMark), // Mn       COMBINING CYRILLIC VZMET
    ('\u{a670}', '\u{a672}', BidiClass::NonspacingMark), // Me   [3] COMBINING CYRILLIC TEN MILLIONS SIGN..COMBINING CYRILLIC THOUSAND MILLIONS SIGN
    ('\u{a673}', '\u{a673}', BidiClass::OtherNeutral),   // Po       SLAVONIC ASTERISK
    ('\u{a674}', '\u{a67d}', BidiClass::NonspacingMark), // Mn  [10] COMBINING CYRILLIC LETTER UKRAINIAN IE..COMBINING CYRILLIC PAYEROK
    ('\u{a67e}', '\u{a67e}', BidiClass::OtherNeutral),   // Po       CYRILLIC KAVYKA
    ('\u{a67f}', '\u{a67f}', BidiClass::OtherNeutral),   // Lm       CYRILLIC PAYEROK
    ('\u{a680}', '\u{a69b}', BidiClass::LeftToRight), // L&  [28] CYRILLIC CAPITAL LETTER DWE..CYRILLIC SMALL LETTER CROSSED O
    ('\u{a69c}', '\u{a69d}', BidiClass::LeftToRight), // Lm   [2] MODIFIER LETTER CYRILLIC HARD SIGN..MODIFIER LETTER CYRILLIC SOFT SIGN
    ('\u{a69e}', '\u{a69f}', BidiClass::NonspacingMark), // Mn   [2] COMBINING CYRILLIC LETTER EF..COMBINING CYRILLIC LETTER IOTIFIED E
    ('\u{a6a0}', '\u{a6e5}', BidiClass::LeftToRight),    // Lo  [70] BAMUM LETTER A..BAMUM LETTER KI
    ('\u{a6e6}', '\u{a6ef}', BidiClass::LeftToRight), // Nl  [10] BAMUM LETTER MO..BAMUM LETTER KOGHOM
    ('\u{a6f0}', '\u{a6f1}', BidiClass::NonspacingMark), // Mn   [2] BAMUM COMBINING MARK KOQNDON..BAMUM COMBINING MARK TUKWENTIS
    ('\u{a6f2}', '\u{a6f7}', BidiClass::LeftToRight), // Po   [6] BAMUM NJAEMLI..BAMUM QUESTION MARK
    ('\u{a700}', '\u{a716}', BidiClass::OtherNeutral), // Sk  [23] MODIFIER LETTER CHINESE TONE YIN PING..MODIFIER LETTER EXTRA-LOW LEFT-STEM TONE BAR
    ('\u{a717}', '\u{a71f}', BidiClass::OtherNeutral), // Lm   [9] MODIFIER LETTER DOT VERTICAL BAR..MODIFIER LETTER LOW INVERTED EXCLAMATION MARK
    ('\u{a720}', '\u{a721}', BidiClass::OtherNeutral), // Sk   [2] MODIFIER LETTER STRESS AND HIGH TONE..MODIFIER LETTER STRESS AND LOW TONE
    ('\u{a722}', '\u{a76f}', BidiClass::LeftToRight), // L&  [78] LATIN CAPITAL LETTER EGYPTOLOGICAL ALEF..LATIN SMALL LETTER CON
    ('\u{a770}', '\u{a770}', BidiClass::LeftToRight), // Lm       MODIFIER LETTER US
    ('\u{a771}', '\u{a787}', BidiClass::LeftToRight), // L&  [23] LATIN SMALL LETTER DUM..LATIN SMALL LETTER INSULAR T
    ('\u{a788}', '\u{a788}', BidiClass::OtherNeutral), // Lm       MODIFIER LETTER LOW CIRCUMFLEX ACCENT
    ('\u{a789}', '\u{a78a}', BidiClass::LeftToRight), // Sk   [2] MODIFIER LETTER COLON..MODIFIER LETTER SHORT EQUALS SIGN
    ('\u{a78b}', '\u{a78e}', BidiClass::LeftToRight), // L&   [4] LATIN CAPITAL LETTER SALTILLO..LATIN SMALL LETTER L WITH RETROFLEX HOOK AND BELT
    ('\u{a78f}', '\u{a78f}', BidiClass::LeftToRight), // Lo       LATIN LETTER SINOLOGICAL DOT
    ('\u{a790}', '\u{a7ca}', BidiClass::LeftToRight), // L&  [59] LATIN CAPITAL LETTER N WITH DESCENDER..LATIN SMALL LETTER S WITH SHORT STROKE OVERLAY
    ('\u{a7d0}', '\u{a7d1}', BidiClass::LeftToRight), // L&   [2] LATIN CAPITAL LETTER CLOSED INSULAR G..LATIN SMALL LETTER CLOSED INSULAR G
    ('\u{a7d3}', '\u{a7d3}', BidiClass::LeftToRight), // L&       LATIN SMALL LETTER DOUBLE THORN
    ('\u{a7d5}', '\u{a7d9}', BidiClass::LeftToRight), // L&   [5] LATIN SMALL LETTER DOUBLE WYNN..LATIN SMALL LETTER SIGMOID S
    ('\u{a7f2}', '\u{a7f4}', BidiClass::LeftToRight), // Lm   [3] MODIFIER LETTER CAPITAL C..MODIFIER LETTER CAPITAL Q
    ('\u{a7f5}', '\u{a7f6}', BidiClass::LeftToRight), // L&   [2] LATIN CAPITAL LETTER REVERSED HALF H..LATIN SMALL LETTER REVERSED HALF H
    ('\u{a7f7}', '\u{a7f7}', BidiClass::LeftToRight), // Lo       LATIN EPIGRAPHIC LETTER SIDEWAYS I
    ('\u{a7f8}', '\u{a7f9}', BidiClass::LeftToRight), // Lm   [2] MODIFIER LETTER CAPITAL H WITH STROKE..MODIFIER LETTER SMALL LIGATURE OE
    ('\u{a7fa}', '\u{a7fa}', BidiClass::LeftToRight), // L&       LATIN LETTER SMALL CAPITAL TURNED M
    ('\u{a7fb}', '\u{a801}', BidiClass::LeftToRight), // Lo   [7] LATIN EPIGRAPHIC LETTER REVERSED F..SYLOTI NAGRI LETTER I
    ('\u{a802}', '\u{a802}', BidiClass::NonspacingMark), // Mn       SYLOTI NAGRI SIGN DVISVARA
    ('\u{a803}', '\u{a805}', BidiClass::LeftToRight), // Lo   [3] SYLOTI NAGRI LETTER U..SYLOTI NAGRI LETTER O
    ('\u{a806}', '\u{a806}', BidiClass::NonspacingMark), // Mn       SYLOTI NAGRI SIGN HASANTA
    ('\u{a807}', '\u{a80a}', BidiClass::LeftToRight), // Lo   [4] SYLOTI NAGRI LETTER KO..SYLOTI NAGRI LETTER GHO
    ('\u{a80b}', '\u{a80b}', BidiClass::NonspacingMark), // Mn       SYLOTI NAGRI SIGN ANUSVARA
    ('\u{a80c}', '\u{a822}', BidiClass::LeftToRight), // Lo  [23] SYLOTI NAGRI LETTER CO..SYLOTI NAGRI LETTER HO
    ('\u{a823}', '\u{a824}', BidiClass::LeftToRight), // Mc   [2] SYLOTI NAGRI VOWEL SIGN A..SYLOTI NAGRI VOWEL SIGN I
    ('\u{a825}', '\u{a826}', BidiClass::NonspacingMark), // Mn   [2] SYLOTI NAGRI VOWEL SIGN U..SYLOTI NAGRI VOWEL SIGN E
    ('\u{a827}', '\u{a827}', BidiClass::LeftToRight),    // Mc       SYLOTI NAGRI VOWEL SIGN OO
    ('\u{a828}', '\u{a82b}', BidiClass::OtherNeutral), // So   [4] SYLOTI NAGRI POETRY MARK-1..SYLOTI NAGRI POETRY MARK-4
    ('\u{a82c}', '\u{a82c}', BidiClass::NonspacingMark), // Mn       SYLOTI NAGRI SIGN ALTERNATE HASANTA
    ('\u{a830}', '\u{a835}', BidiClass::LeftToRight), // No   [6] NORTH INDIC FRACTION ONE QUARTER..NORTH INDIC FRACTION THREE SIXTEENTHS
    ('\u{a836}', '\u{a837}', BidiClass::LeftToRight), // So   [2] NORTH INDIC QUARTER MARK..NORTH INDIC PLACEHOLDER MARK
    ('\u{a838}', '\u{a838}', BidiClass::EuropeanTerminator), // Sc       NORTH INDIC RUPEE MARK
    ('\u{a839}', '\u{a839}', BidiClass::EuropeanTerminator), // So       NORTH INDIC QUANTITY MARK
    ('\u{a840}', '\u{a873}', BidiClass::LeftToRight), // Lo  [52] PHAGS-PA LETTER KA..PHAGS-PA LETTER CANDRABINDU
    ('\u{a874}', '\u{a877}', BidiClass::OtherNeutral), // Po   [4] PHAGS-PA SINGLE HEAD MARK..PHAGS-PA MARK DOUBLE SHAD
    ('\u{a880}', '\u{a881}', BidiClass::LeftToRight), // Mc   [2] SAURASHTRA SIGN ANUSVARA..SAURASHTRA SIGN VISARGA
    ('\u{a882}', '\u{a8b3}', BidiClass::LeftToRight), // Lo  [50] SAURASHTRA LETTER A..SAURASHTRA LETTER LLA
    ('\u{a8b4}', '\u{a8c3}', BidiClass::LeftToRight), // Mc  [16] SAURASHTRA CONSONANT SIGN HAARU..SAURASHTRA VOWEL SIGN AU
    ('\u{a8c4}', '\u{a8c5}', BidiClass::NonspacingMark), // Mn   [2] SAURASHTRA SIGN VIRAMA..SAURASHTRA SIGN CANDRABINDU
    ('\u{a8ce}', '\u{a8cf}', BidiClass::LeftToRight), // Po   [2] SAURASHTRA DANDA..SAURASHTRA DOUBLE DANDA
    ('\u{a8d0}', '\u{a8d9}', BidiClass::LeftToRight), // Nd  [10] SAURASHTRA DIGIT ZERO..SAURASHTRA DIGIT NINE
    ('\u{a8e0}', '\u{a8f1}', BidiClass::NonspacingMark), // Mn  [18] COMBINING DEVANAGARI DIGIT ZERO..COMBINING DEVANAGARI SIGN AVAGRAHA
    ('\u{a8f2}', '\u{a8f7}', BidiClass::LeftToRight), // Lo   [6] DEVANAGARI SIGN SPACING CANDRABINDU..DEVANAGARI SIGN CANDRABINDU AVAGRAHA
    ('\u{a8f8}', '\u{a8fa}', BidiClass::LeftToRight), // Po   [3] DEVANAGARI SIGN PUSHPIKA..DEVANAGARI CARET
    ('\u{a8fb}', '\u{a8fb}', BidiClass::LeftToRight), // Lo       DEVANAGARI HEADSTROKE
    ('\u{a8fc}', '\u{a8fc}', BidiClass::LeftToRight), // Po       DEVANAGARI SIGN SIDDHAM
    ('\u{a8fd}', '\u{a8fe}', BidiClass::LeftToRight), // Lo   [2] DEVANAGARI JAIN OM..DEVANAGARI LETTER AY
    ('\u{a8ff}', '\u{a8ff}', BidiClass::NonspacingMark), // Mn       DEVANAGARI VOWEL SIGN AY
    ('\u{a900}', '\u{a909}', BidiClass::LeftToRight), // Nd  [10] KAYAH LI DIGIT ZERO..KAYAH LI DIGIT NINE
    ('\u{a90a}', '\u{a925}', BidiClass::LeftToRight), // Lo  [28] KAYAH LI LETTER KA..KAYAH LI LETTER OO
    ('\u{a926}', '\u{a92d}', BidiClass::NonspacingMark), // Mn   [8] KAYAH LI VOWEL UE..KAYAH LI TONE CALYA PLOPHU
    ('\u{a92e}', '\u{a92f}', BidiClass::LeftToRight), // Po   [2] KAYAH LI SIGN CWI..KAYAH LI SIGN SHYA
    ('\u{a930}', '\u{a946}', BidiClass::LeftToRight), // Lo  [23] REJANG LETTER KA..REJANG LETTER A
    ('\u{a947}', '\u{a951}', BidiClass::NonspacingMark), // Mn  [11] REJANG VOWEL SIGN I..REJANG CONSONANT SIGN R
    ('\u{a952}', '\u{a953}', BidiClass::LeftToRight), // Mc   [2] REJANG CONSONANT SIGN H..REJANG VIRAMA
    ('\u{a95f}', '\u{a95f}', BidiClass::LeftToRight), // Po       REJANG SECTION MARK
    ('\u{a960}', '\u{a97c}', BidiClass::LeftToRight), // Lo  [29] HANGUL CHOSEONG TIKEUT-MIEUM..HANGUL CHOSEONG SSANGYEORINHIEUH
    ('\u{a980}', '\u{a982}', BidiClass::NonspacingMark), // Mn   [3] JAVANESE SIGN PANYANGGA..JAVANESE SIGN LAYAR
    ('\u{a983}', '\u{a983}', BidiClass::LeftToRight),    // Mc       JAVANESE SIGN WIGNYAN
    ('\u{a984}', '\u{a9b2}', BidiClass::LeftToRight), // Lo  [47] JAVANESE LETTER A..JAVANESE LETTER HA
    ('\u{a9b3}', '\u{a9b3}', BidiClass::NonspacingMark), // Mn       JAVANESE SIGN CECAK TELU
    ('\u{a9b4}', '\u{a9b5}', BidiClass::LeftToRight), // Mc   [2] JAVANESE VOWEL SIGN TARUNG..JAVANESE VOWEL SIGN TOLONG
    ('\u{a9b6}', '\u{a9b9}', BidiClass::NonspacingMark), // Mn   [4] JAVANESE VOWEL SIGN WULU..JAVANESE VOWEL SIGN SUKU MENDUT
    ('\u{a9ba}', '\u{a9bb}', BidiClass::LeftToRight), // Mc   [2] JAVANESE VOWEL SIGN TALING..JAVANESE VOWEL SIGN DIRGA MURE
    ('\u{a9bc}', '\u{a9bd}', BidiClass::NonspacingMark), // Mn   [2] JAVANESE VOWEL SIGN PEPET..JAVANESE CONSONANT SIGN KERET
    ('\u{a9be}', '\u{a9c0}', BidiClass::LeftToRight), // Mc   [3] JAVANESE CONSONANT SIGN PENGKAL..JAVANESE PANGKON
    ('\u{a9c1}', '\u{a9cd}', BidiClass::LeftToRight), // Po  [13] JAVANESE LEFT RERENGGAN..JAVANESE TURNED PADA PISELEH
    ('\u{a9cf}', '\u{a9cf}', BidiClass::LeftToRight), // Lm       JAVANESE PANGRANGKEP
    ('\u{a9d0}', '\u{a9d9}', BidiClass::LeftToRight), // Nd  [10] JAVANESE DIGIT ZERO..JAVANESE DIGIT NINE
    ('\u{a9de}', '\u{a9df}', BidiClass::LeftToRight), // Po   [2] JAVANESE PADA TIRTA TUMETES..JAVANESE PADA ISEN-ISEN
    ('\u{a9e0}', '\u{a9e4}', BidiClass::LeftToRight), // Lo   [5] MYANMAR LETTER SHAN GHA..MYANMAR LETTER SHAN BHA
    ('\u{a9e5}', '\u{a9e5}', BidiClass::NonspacingMark), // Mn       MYANMAR SIGN SHAN SAW
    ('\u{a9e6}', '\u{a9e6}', BidiClass::LeftToRight), // Lm       MYANMAR MODIFIER LETTER SHAN REDUPLICATION
    ('\u{a9e7}', '\u{a9ef}', BidiClass::LeftToRight), // Lo   [9] MYANMAR LETTER TAI LAING NYA..MYANMAR LETTER TAI LAING NNA
    ('\u{a9f0}', '\u{a9f9}', BidiClass::LeftToRight), // Nd  [10] MYANMAR TAI LAING DIGIT ZERO..MYANMAR TAI LAING DIGIT NINE
    ('\u{a9fa}', '\u{a9fe}', BidiClass::LeftToRight), // Lo   [5] MYANMAR LETTER TAI LAING LLA..MYANMAR LETTER TAI LAING BHA
    ('\u{aa00}', '\u{aa28}', BidiClass::LeftToRight), // Lo  [41] CHAM LETTER A..CHAM LETTER HA
    ('\u{aa29}', '\u{aa2e}', BidiClass::NonspacingMark), // Mn   [6] CHAM VOWEL SIGN AA..CHAM VOWEL SIGN OE
    ('\u{aa2f}', '\u{aa30}', BidiClass::LeftToRight), // Mc   [2] CHAM VOWEL SIGN O..CHAM VOWEL SIGN AI
    ('\u{aa31}', '\u{aa32}', BidiClass::NonspacingMark), // Mn   [2] CHAM VOWEL SIGN AU..CHAM VOWEL SIGN UE
    ('\u{aa33}', '\u{aa34}', BidiClass::LeftToRight), // Mc   [2] CHAM CONSONANT SIGN YA..CHAM CONSONANT SIGN RA
    ('\u{aa35}', '\u{aa36}', BidiClass::NonspacingMark), // Mn   [2] CHAM CONSONANT SIGN LA..CHAM CONSONANT SIGN WA
    ('\u{aa40}', '\u{aa42}', BidiClass::LeftToRight), // Lo   [3] CHAM LETTER FINAL K..CHAM LETTER FINAL NG
    ('\u{aa43}', '\u{aa43}', BidiClass::NonspacingMark), // Mn       CHAM CONSONANT SIGN FINAL NG
    ('\u{aa44}', '\u{aa4b}', BidiClass::LeftToRight), // Lo   [8] CHAM LETTER FINAL CH..CHAM LETTER FINAL SS
    ('\u{aa4c}', '\u{aa4c}', BidiClass::NonspacingMark), // Mn       CHAM CONSONANT SIGN FINAL M
    ('\u{aa4d}', '\u{aa4d}', BidiClass::LeftToRight), // Mc       CHAM CONSONANT SIGN FINAL H
    ('\u{aa50}', '\u{aa59}', BidiClass::LeftToRight), // Nd  [10] CHAM DIGIT ZERO..CHAM DIGIT NINE
    ('\u{aa5c}', '\u{aa5f}', BidiClass::LeftToRight), // Po   [4] CHAM PUNCTUATION SPIRAL..CHAM PUNCTUATION TRIPLE DANDA
    ('\u{aa60}', '\u{aa6f}', BidiClass::LeftToRight), // Lo  [16] MYANMAR LETTER KHAMTI GA..MYANMAR LETTER KHAMTI FA
    ('\u{aa70}', '\u{aa70}', BidiClass::LeftToRight), // Lm       MYANMAR MODIFIER LETTER KHAMTI REDUPLICATION
    ('\u{aa71}', '\u{aa76}', BidiClass::LeftToRight), // Lo   [6] MYANMAR LETTER KHAMTI XA..MYANMAR LOGOGRAM KHAMTI HM
    ('\u{aa77}', '\u{aa79}', BidiClass::LeftToRight), // So   [3] MYANMAR SYMBOL AITON EXCLAMATION..MYANMAR SYMBOL AITON TWO
    ('\u{aa7a}', '\u{aa7a}', BidiClass::LeftToRight), // Lo       MYANMAR LETTER AITON RA
    ('\u{aa7b}', '\u{aa7b}', BidiClass::LeftToRight), // Mc       MYANMAR SIGN PAO KAREN TONE
    ('\u{aa7c}', '\u{aa7c}', BidiClass::NonspacingMark), // Mn       MYANMAR SIGN TAI LAING TONE-2
    ('\u{aa7d}', '\u{aa7d}', BidiClass::LeftToRight), // Mc       MYANMAR SIGN TAI LAING TONE-5
    ('\u{aa7e}', '\u{aaaf}', BidiClass::LeftToRight), // Lo  [50] MYANMAR LETTER SHWE PALAUNG CHA..TAI VIET LETTER HIGH O
    ('\u{aab0}', '\u{aab0}', BidiClass::NonspacingMark), // Mn       TAI VIET MAI KANG
    ('\u{aab1}', '\u{aab1}', BidiClass::LeftToRight), // Lo       TAI VIET VOWEL AA
    ('\u{aab2}', '\u{aab4}', BidiClass::NonspacingMark), // Mn   [3] TAI VIET VOWEL I..TAI VIET VOWEL U
    ('\u{aab5}', '\u{aab6}', BidiClass::LeftToRight), // Lo   [2] TAI VIET VOWEL E..TAI VIET VOWEL O
    ('\u{aab7}', '\u{aab8}', BidiClass::NonspacingMark), // Mn   [2] TAI VIET MAI KHIT..TAI VIET VOWEL IA
    ('\u{aab9}', '\u{aabd}', BidiClass::LeftToRight), // Lo   [5] TAI VIET VOWEL UEA..TAI VIET VOWEL AN
    ('\u{aabe}', '\u{aabf}', BidiClass::NonspacingMark), // Mn   [2] TAI VIET VOWEL AM..TAI VIET TONE MAI EK
    ('\u{aac0}', '\u{aac0}', BidiClass::LeftToRight),    // Lo       TAI VIET TONE MAI NUENG
    ('\u{aac1}', '\u{aac1}', BidiClass::NonspacingMark), // Mn       TAI VIET TONE MAI THO
    ('\u{aac2}', '\u{aac2}', BidiClass::LeftToRight),    // Lo       TAI VIET TONE MAI SONG
    ('\u{aadb}', '\u{aadc}', BidiClass::LeftToRight), // Lo   [2] TAI VIET SYMBOL KON..TAI VIET SYMBOL NUENG
    ('\u{aadd}', '\u{aadd}', BidiClass::LeftToRight), // Lm       TAI VIET SYMBOL SAM
    ('\u{aade}', '\u{aadf}', BidiClass::LeftToRight), // Po   [2] TAI VIET SYMBOL HO HOI..TAI VIET SYMBOL KOI KOI
    ('\u{aae0}', '\u{aaea}', BidiClass::LeftToRight), // Lo  [11] MEETEI MAYEK LETTER E..MEETEI MAYEK LETTER SSA
    ('\u{aaeb}', '\u{aaeb}', BidiClass::LeftToRight), // Mc       MEETEI MAYEK VOWEL SIGN II
    ('\u{aaec}', '\u{aaed}', BidiClass::NonspacingMark), // Mn   [2] MEETEI MAYEK VOWEL SIGN UU..MEETEI MAYEK VOWEL SIGN AAI
    ('\u{aaee}', '\u{aaef}', BidiClass::LeftToRight), // Mc   [2] MEETEI MAYEK VOWEL SIGN AU..MEETEI MAYEK VOWEL SIGN AAU
    ('\u{aaf0}', '\u{aaf1}', BidiClass::LeftToRight), // Po   [2] MEETEI MAYEK CHEIKHAN..MEETEI MAYEK AHANG KHUDAM
    ('\u{aaf2}', '\u{aaf2}', BidiClass::LeftToRight), // Lo       MEETEI MAYEK ANJI
    ('\u{aaf3}', '\u{aaf4}', BidiClass::LeftToRight), // Lm   [2] MEETEI MAYEK SYLLABLE REPETITION MARK..MEETEI MAYEK WORD REPETITION MARK
    ('\u{aaf5}', '\u{aaf5}', BidiClass::LeftToRight), // Mc       MEETEI MAYEK VOWEL SIGN VISARGA
    ('\u{aaf6}', '\u{aaf6}', BidiClass::NonspacingMark), // Mn       MEETEI MAYEK VIRAMA
    ('\u{ab01}', '\u{ab06}', BidiClass::LeftToRight), // Lo   [6] ETHIOPIC SYLLABLE TTHU..ETHIOPIC SYLLABLE TTHO
    ('\u{ab09}', '\u{ab0e}', BidiClass::LeftToRight), // Lo   [6] ETHIOPIC SYLLABLE DDHU..ETHIOPIC SYLLABLE DDHO
    ('\u{ab11}', '\u{ab16}', BidiClass::LeftToRight), // Lo   [6] ETHIOPIC SYLLABLE DZU..ETHIOPIC SYLLABLE DZO
    ('\u{ab20}', '\u{ab26}', BidiClass::LeftToRight), // Lo   [7] ETHIOPIC SYLLABLE CCHHA..ETHIOPIC SYLLABLE CCHHO
    ('\u{ab28}', '\u{ab2e}', BidiClass::LeftToRight), // Lo   [7] ETHIOPIC SYLLABLE BBA..ETHIOPIC SYLLABLE BBO
    ('\u{ab30}', '\u{ab5a}', BidiClass::LeftToRight), // L&  [43] LATIN SMALL LETTER BARRED ALPHA..LATIN SMALL LETTER Y WITH SHORT RIGHT LEG
    ('\u{ab5b}', '\u{ab5b}', BidiClass::LeftToRight), // Sk       MODIFIER BREVE WITH INVERTED BREVE
    ('\u{ab5c}', '\u{ab5f}', BidiClass::LeftToRight), // Lm   [4] MODIFIER LETTER SMALL HENG..MODIFIER LETTER SMALL U WITH LEFT HOOK
    ('\u{ab60}', '\u{ab68}', BidiClass::LeftToRight), // L&   [9] LATIN SMALL LETTER SAKHA YAT..LATIN SMALL LETTER TURNED R WITH MIDDLE TILDE
    ('\u{ab69}', '\u{ab69}', BidiClass::LeftToRight), // Lm       MODIFIER LETTER SMALL TURNED W
    ('\u{ab6a}', '\u{ab6b}', BidiClass::OtherNeutral), // Sk   [2] MODIFIER LETTER LEFT TACK..MODIFIER LETTER RIGHT TACK
    ('\u{ab70}', '\u{abbf}', BidiClass::LeftToRight), // L&  [80] CHEROKEE SMALL LETTER A..CHEROKEE SMALL LETTER YA
    ('\u{abc0}', '\u{abe2}', BidiClass::LeftToRight), // Lo  [35] MEETEI MAYEK LETTER KOK..MEETEI MAYEK LETTER I LONSUM
    ('\u{abe3}', '\u{abe4}', BidiClass::LeftToRight), // Mc   [2] MEETEI MAYEK VOWEL SIGN ONAP..MEETEI MAYEK VOWEL SIGN INAP
    ('\u{abe5}', '\u{abe5}', BidiClass::NonspacingMark), // Mn       MEETEI MAYEK VOWEL SIGN ANAP
    ('\u{abe6}', '\u{abe7}', BidiClass::LeftToRight), // Mc   [2] MEETEI MAYEK VOWEL SIGN YENAP..MEETEI MAYEK VOWEL SIGN SOUNAP
    ('\u{abe8}', '\u{abe8}', BidiClass::NonspacingMark), // Mn       MEETEI MAYEK VOWEL SIGN UNAP
    ('\u{abe9}', '\u{abea}', BidiClass::LeftToRight), // Mc   [2] MEETEI MAYEK VOWEL SIGN CHEINAP..MEETEI MAYEK VOWEL SIGN NUNG
    ('\u{abeb}', '\u{abeb}', BidiClass::LeftToRight), // Po       MEETEI MAYEK CHEIKHEI
    ('\u{abec}', '\u{abec}', BidiClass::LeftToRight), // Mc       MEETEI MAYEK LUM IYEK
    ('\u{abed}', '\u{abed}', BidiClass::NonspacingMark), // Mn       MEETEI MAYEK APUN IYEK
    ('\u{abf0}', '\u{abf9}', BidiClass::LeftToRight), // Nd  [10] MEETEI MAYEK DIGIT ZERO..MEETEI MAYEK DIGIT NINE
    ('\u{ac00}', '\u{d7a3}', BidiClass::LeftToRight), // Lo [11172] HANGUL SYLLABLE GA..HANGUL SYLLABLE HIH
    ('\u{d7b0}', '\u{d7c6}', BidiClass::LeftToRight), // Lo  [23] HANGUL JUNGSEONG O-YEO..HANGUL JUNGSEONG ARAEA-E
    ('\u{d7cb}', '\u{d7fb}', BidiClass::LeftToRight), // Lo  [49] HANGUL JONGSEONG NIEUN-RIEUL..HANGUL JONGSEONG PHIEUPH-THIEUTH
    ('\u{e000}', '\u{f8ff}', BidiClass::LeftToRight), // Co [6400] <private-use-E000>..<private-use-F8FF>
    ('\u{f900}', '\u{fa6d}', BidiClass::LeftToRight), // Lo [366] CJK COMPATIBILITY IDEOGRAPH-F900..CJK COMPATIBILITY IDEOGRAPH-FA6D
    ('\u{fa70}', '\u{fad9}', BidiClass::LeftToRight), // Lo [106] CJK COMPATIBILITY IDEOGRAPH-FA70..CJK COMPATIBILITY IDEOGRAPH-FAD9
    ('\u{fb00}', '\u{fb06}', BidiClass::LeftToRight), // L&   [7] LATIN SMALL LIGATURE FF..LATIN SMALL LIGATURE ST
    ('\u{fb13}', '\u{fb17}', BidiClass::LeftToRight), // L&   [5] ARMENIAN SMALL LIGATURE MEN NOW..ARMENIAN SMALL LIGATURE MEN XEH
    ('\u{fb1d}', '\u{fb1d}', BidiClass::RightToLeft), // Lo       HEBREW LETTER YOD WITH HIRIQ
    ('\u{fb1e}', '\u{fb1e}', BidiClass::NonspacingMark), // Mn       HEBREW POINT JUDEO-SPANISH VARIKA
    ('\u{fb1f}', '\u{fb28}', BidiClass::RightToLeft), // Lo  [10] HEBREW LIGATURE YIDDISH YOD YOD PATAH..HEBREW LETTER WIDE TAV
    ('\u{fb29}', '\u{fb29}', BidiClass::EuropeanSeparator), // Sm       HEBREW LETTER ALTERNATIVE PLUS SIGN
    ('\u{fb2a}', '\u{fb36}', BidiClass::RightToLeft), // Lo  [13] HEBREW LETTER SHIN WITH SHIN DOT..HEBREW LETTER ZAYIN WITH DAGESH
    ('\u{fb37}', '\u{fb37}', BidiClass::RightToLeft), // Cn       <reserved-FB37>
    ('\u{fb38}', '\u{fb3c}', BidiClass::RightToLeft), // Lo   [5] HEBREW LETTER TET WITH DAGESH..HEBREW LETTER LAMED WITH DAGESH
    ('\u{fb3d}', '\u{fb3d}', BidiClass::RightToLeft), // Cn       <reserved-FB3D>
    ('\u{fb3e}', '\u{fb3e}', BidiClass::RightToLeft), // Lo       HEBREW LETTER MEM WITH DAGESH
    ('\u{fb3f}', '\u{fb3f}', BidiClass::RightToLeft), // Cn       <reserved-FB3F>
    ('\u{fb40}', '\u{fb41}', BidiClass::RightToLeft), // Lo   [2] HEBREW LETTER NUN WITH DAGESH..HEBREW LETTER SAMEKH WITH DAGESH
    ('\u{fb42}', '\u{fb42}', BidiClass::RightToLeft), // Cn       <reserved-FB42>
    ('\u{fb43}', '\u{fb44}', BidiClass::RightToLeft), // Lo   [2] HEBREW LETTER FINAL PE WITH DAGESH..HEBREW LETTER PE WITH DAGESH
    ('\u{fb45}', '\u{fb45}', BidiClass::RightToLeft), // Cn       <reserved-FB45>
    ('\u{fb46}', '\u{fb4f}', BidiClass::RightToLeft), // Lo  [10] HEBREW LETTER TSADI WITH DAGESH..HEBREW LIGATURE ALEF LAMED
    ('\u{fb50}', '\u{fbb1}', BidiClass::ArabicLetter), // Lo  [98] ARABIC LETTER ALEF WASLA ISOLATED FORM..ARABIC LETTER YEH BARREE WITH HAMZA ABOVE FINAL FORM
    ('\u{fbb2}', '\u{fbc2}', BidiClass::ArabicLetter), // Sk  [17] ARABIC SYMBOL DOT ABOVE..ARABIC SYMBOL WASLA ABOVE
    ('\u{fbc3}', '\u{fbd2}', BidiClass::ArabicLetter), // Cn  [16] <reserved-FBC3>..<reserved-FBD2>
    ('\u{fbd3}', '\u{fd3d}', BidiClass::ArabicLetter), // Lo [363] ARABIC LETTER NG ISOLATED FORM..ARABIC LIGATURE ALEF WITH FATHATAN ISOLATED FORM
    ('\u{fd3e}', '\u{fd3e}', BidiClass::OtherNeutral), // Pe       ORNATE LEFT PARENTHESIS
    ('\u{fd3f}', '\u{fd3f}', BidiClass::OtherNeutral), // Ps       ORNATE RIGHT PARENTHESIS
    ('\u{fd40}', '\u{fd4f}', BidiClass::OtherNeutral), // So  [16] ARABIC LIGATURE RAHIMAHU ALLAAH..ARABIC LIGATURE RAHIMAHUM ALLAAH
    ('\u{fd50}', '\u{fd8f}', BidiClass::ArabicLetter), // Lo  [64] ARABIC LIGATURE TEH WITH JEEM WITH MEEM INITIAL FORM..ARABIC LIGATURE MEEM WITH KHAH WITH MEEM INITIAL FORM
    ('\u{fd90}', '\u{fd91}', BidiClass::ArabicLetter), // Cn   [2] <reserved-FD90>..<reserved-FD91>
    ('\u{fd92}', '\u{fdc7}', BidiClass::ArabicLetter), // Lo  [54] ARABIC LIGATURE MEEM WITH JEEM WITH KHAH INITIAL FORM..ARABIC LIGATURE NOON WITH JEEM WITH YEH FINAL FORM
    ('\u{fdc8}', '\u{fdce}', BidiClass::ArabicLetter), // Cn   [7] <reserved-FDC8>..<reserved-FDCE>
    ('\u{fdcf}', '\u{fdcf}', BidiClass::OtherNeutral), // So       ARABIC LIGATURE SALAAMUHU ALAYNAA
    ('\u{fdd0}', '\u{fdef}', BidiClass::BoundaryNeutral), // Cn  [32] <noncharacter-FDD0>..<noncharacter-FDEF>
    ('\u{fdf0}', '\u{fdfb}', BidiClass::ArabicLetter), // Lo  [12] ARABIC LIGATURE SALLA USED AS KORANIC STOP SIGN ISOLATED FORM..ARABIC LIGATURE JALLAJALALOUHOU
    ('\u{fdfc}', '\u{fdfc}', BidiClass::ArabicLetter), // Sc       RIAL SIGN
    ('\u{fdfd}', '\u{fdff}', BidiClass::OtherNeutral), // So   [3] ARABIC LIGATURE BISMILLAH AR-RAHMAN AR-RAHEEM..ARABIC LIGATURE AZZA WA JALL
    ('\u{fe00}', '\u{fe0f}', BidiClass::NonspacingMark), // Mn  [16] VARIATION SELECTOR-1..VARIATION SELECTOR-16
    ('\u{fe10}', '\u{fe16}', BidiClass::OtherNeutral), // Po   [7] PRESENTATION FORM FOR VERTICAL COMMA..PRESENTATION FORM FOR VERTICAL QUESTION MARK
    ('\u{fe17}', '\u{fe17}', BidiClass::OtherNeutral), // Ps       PRESENTATION FORM FOR VERTICAL LEFT WHITE LENTICULAR BRACKET
    ('\u{fe18}', '\u{fe18}', BidiClass::OtherNeutral), // Pe       PRESENTATION FORM FOR VERTICAL RIGHT WHITE LENTICULAR BRAKCET
    ('\u{fe19}', '\u{fe19}', BidiClass::OtherNeutral), // Po       PRESENTATION FORM FOR VERTICAL HORIZONTAL ELLIPSIS
    ('\u{fe20}', '\u{fe2f}', BidiClass::NonspacingMark), // Mn  [16] COMBINING LIGATURE LEFT HALF..COMBINING CYRILLIC TITLO RIGHT HALF
    ('\u{fe30}', '\u{fe30}', BidiClass::OtherNeutral), // Po       PRESENTATION FORM FOR VERTICAL TWO DOT LEADER
    ('\u{fe31}', '\u{fe32}', BidiClass::OtherNeutral), // Pd   [2] PRESENTATION FORM FOR VERTICAL EM DASH..PRESENTATION FORM FOR VERTICAL EN DASH
    ('\u{fe33}', '\u{fe34}', BidiClass::OtherNeutral), // Pc   [2] PRESENTATION FORM FOR VERTICAL LOW LINE..PRESENTATION FORM FOR VERTICAL WAVY LOW LINE
    ('\u{fe35}', '\u{fe35}', BidiClass::OtherNeutral), // Ps       PRESENTATION FORM FOR VERTICAL LEFT PARENTHESIS
    ('\u{fe36}', '\u{fe36}', BidiClass::OtherNeutral), // Pe       PRESENTATION FORM FOR VERTICAL RIGHT PARENTHESIS
    ('\u{fe37}', '\u{fe37}', BidiClass::OtherNeutral), // Ps       PRESENTATION FORM FOR VERTICAL LEFT CURLY BRACKET
    ('\u{fe38}', '\u{fe38}', BidiClass::OtherNeutral), // Pe       PRESENTATION FORM FOR VERTICAL RIGHT CURLY BRACKET
    ('\u{fe39}', '\u{fe39}', BidiClass::OtherNeutral), // Ps       PRESENTATION FORM FOR VERTICAL LEFT TORTOISE SHELL BRACKET
    ('\u{fe3a}', '\u{fe3a}', BidiClass::OtherNeutral), // Pe       PRESENTATION FORM FOR VERTICAL RIGHT TORTOISE SHELL BRACKET
    ('\u{fe3b}', '\u{fe3b}', BidiClass::OtherNeutral), // Ps       PRESENTATION FORM FOR VERTICAL LEFT BLACK LENTICULAR BRACKET
    ('\u{fe3c}', '\u{fe3c}', BidiClass::OtherNeutral), // Pe       PRESENTATION FORM FOR VERTICAL RIGHT BLACK LENTICULAR BRACKET
    ('\u{fe3d}', '\u{fe3d}', BidiClass::OtherNeutral), // Ps       PRESENTATION FORM FOR VERTICAL LEFT DOUBLE ANGLE BRACKET
    ('\u{fe3e}', '\u{fe3e}', BidiClass::OtherNeutral), // Pe       PRESENTATION FORM FOR VERTICAL RIGHT DOUBLE ANGLE BRACKET
    ('\u{fe3f}', '\u{fe3f}', BidiClass::OtherNeutral), // Ps       PRESENTATION FORM FOR VERTICAL LEFT ANGLE BRACKET
    ('\u{fe40}', '\u{fe40}', BidiClass::OtherNeutral), // Pe       PRESENTATION FORM FOR VERTICAL RIGHT ANGLE BRACKET
    ('\u{fe41}', '\u{fe41}', BidiClass::OtherNeutral), // Ps       PRESENTATION FORM FOR VERTICAL LEFT CORNER BRACKET
    ('\u{fe42}', '\u{fe42}', BidiClass::OtherNeutral), // Pe       PRESENTATION FORM FOR VERTICAL RIGHT CORNER BRACKET
    ('\u{fe43}', '\u{fe43}', BidiClass::OtherNeutral), // Ps       PRESENTATION FORM FOR VERTICAL LEFT WHITE CORNER BRACKET
    ('\u{fe44}', '\u{fe44}', BidiClass::OtherNeutral), // Pe       PRESENTATION FORM FOR VERTICAL RIGHT WHITE CORNER BRACKET
    ('\u{fe45}', '\u{fe46}', BidiClass::OtherNeutral), // Po   [2] SESAME DOT..WHITE SESAME DOT
    ('\u{fe47}', '\u{fe47}', BidiClass::OtherNeutral), // Ps       PRESENTATION FORM FOR VERTICAL LEFT SQUARE BRACKET
    ('\u{fe48}', '\u{fe48}', BidiClass::OtherNeutral), // Pe       PRESENTATION FORM FOR VERTICAL RIGHT SQUARE BRACKET
    ('\u{fe49}', '\u{fe4c}', BidiClass::OtherNeutral), // Po   [4] DASHED OVERLINE..DOUBLE WAVY OVERLINE
    ('\u{fe4d}', '\u{fe4f}', BidiClass::OtherNeutral), // Pc   [3] DASHED LOW LINE..WAVY LOW LINE
    ('\u{fe50}', '\u{fe50}', BidiClass::CommonSeparator), // Po       SMALL COMMA
    ('\u{fe51}', '\u{fe51}', BidiClass::OtherNeutral), // Po       SMALL IDEOGRAPHIC COMMA
    ('\u{fe52}', '\u{fe52}', BidiClass::CommonSeparator), // Po       SMALL FULL STOP
    ('\u{fe54}', '\u{fe54}', BidiClass::OtherNeutral), // Po       SMALL SEMICOLON
    ('\u{fe55}', '\u{fe55}', BidiClass::CommonSeparator), // Po       SMALL COLON
    ('\u{fe56}', '\u{fe57}', BidiClass::OtherNeutral), // Po   [2] SMALL QUESTION MARK..SMALL EXCLAMATION MARK
    ('\u{fe58}', '\u{fe58}', BidiClass::OtherNeutral), // Pd       SMALL EM DASH
    ('\u{fe59}', '\u{fe59}', BidiClass::OtherNeutral), // Ps       SMALL LEFT PARENTHESIS
    ('\u{fe5a}', '\u{fe5a}', BidiClass::OtherNeutral), // Pe       SMALL RIGHT PARENTHESIS
    ('\u{fe5b}', '\u{fe5b}', BidiClass::OtherNeutral), // Ps       SMALL LEFT CURLY BRACKET
    ('\u{fe5c}', '\u{fe5c}', BidiClass::OtherNeutral), // Pe       SMALL RIGHT CURLY BRACKET
    ('\u{fe5d}', '\u{fe5d}', BidiClass::OtherNeutral), // Ps       SMALL LEFT TORTOISE SHELL BRACKET
    ('\u{fe5e}', '\u{fe5e}', BidiClass::OtherNeutral), // Pe       SMALL RIGHT TORTOISE SHELL BRACKET
    ('\u{fe5f}', '\u{fe5f}', BidiClass::EuropeanTerminator), // Po       SMALL NUMBER SIGN
    ('\u{fe60}', '\u{fe61}', BidiClass::OtherNeutral), // Po   [2] SMALL AMPERSAND..SMALL ASTERISK
    ('\u{fe62}', '\u{fe62}', BidiClass::EuropeanSeparator), // Sm       SMALL PLUS SIGN
    ('\u{fe63}', '\u{fe63}', BidiClass::EuropeanSeparator), // Pd       SMALL HYPHEN-MINUS
    ('\u{fe64}', '\u{fe66}', BidiClass::OtherNeutral), // Sm   [3] SMALL LESS-THAN SIGN..SMALL EQUALS SIGN
    ('\u{fe68}', '\u{fe68}', BidiClass::OtherNeutral), // Po       SMALL REVERSE SOLIDUS
    ('\u{fe69}', '\u{fe69}', BidiClass::EuropeanTerminator), // Sc       SMALL DOLLAR SIGN
    ('\u{fe6a}', '\u{fe6a}', BidiClass::EuropeanTerminator), // Po       SMALL PERCENT SIGN
    ('\u{fe6b}', '\u{fe6b}', BidiClass::OtherNeutral), // Po       SMALL COMMERCIAL AT
    ('\u{fe70}', '\u{fe74}', BidiClass::ArabicLetter), // Lo   [5] ARABIC FATHATAN ISOLATED FORM..ARABIC KASRATAN ISOLATED FORM
    ('\u{fe75}', '\u{fe75}', BidiClass::ArabicLetter), // Cn       <reserved-FE75>
    ('\u{fe76}', '\u{fefc}', BidiClass::ArabicLetter), // Lo [135] ARABIC FATHA ISOLATED FORM..ARABIC LIGATURE LAM WITH ALEF FINAL FORM
    ('\u{fefd}', '\u{fefe}', BidiClass::ArabicLetter), // Cn   [2] <reserved-FEFD>..<reserved-FEFE>
    ('\u{feff}', '\u{feff}', BidiClass::BoundaryNeutral), // Cf       ZERO WIDTH NO-BREAK SPACE
    ('\u{ff01}', '\u{ff02}', BidiClass::OtherNeutral), // Po   [2] FULLWIDTH EXCLAMATION MARK..FULLWIDTH QUOTATION MARK
    ('\u{ff03}', '\u{ff03}', BidiClass::EuropeanTerminator), // Po       FULLWIDTH NUMBER SIGN
    ('\u{ff04}', '\u{ff04}', BidiClass::EuropeanTerminator), // Sc       FULLWIDTH DOLLAR SIGN
    ('\u{ff05}', '\u{ff05}', BidiClass::EuropeanTerminator), // Po       FULLWIDTH PERCENT SIGN
    ('\u{ff06}', '\u{ff07}', BidiClass::OtherNeutral), // Po   [2] FULLWIDTH AMPERSAND..FULLWIDTH APOSTROPHE
    ('\u{ff08}', '\u{ff08}', BidiClass::OtherNeutral), // Ps       FULLWIDTH LEFT PARENTHESIS
    ('\u{ff09}', '\u{ff09}', BidiClass::OtherNeutral), // Pe       FULLWIDTH RIGHT PARENTHESIS
    ('\u{ff0a}', '\u{ff0a}', BidiClass::OtherNeutral), // Po       FULLWIDTH ASTERISK
    ('\u{ff0b}', '\u{ff0b}', BidiClass::EuropeanSeparator), // Sm       FULLWIDTH PLUS SIGN
    ('\u{ff0c}', '\u{ff0c}', BidiClass::CommonSeparator), // Po       FULLWIDTH COMMA
    ('\u{ff0d}', '\u{ff0d}', BidiClass::EuropeanSeparator), // Pd       FULLWIDTH HYPHEN-MINUS
    ('\u{ff0e}', '\u{ff0f}', BidiClass::CommonSeparator), // Po   [2] FULLWIDTH FULL STOP..FULLWIDTH SOLIDUS
    ('\u{ff10}', '\u{ff19}', BidiClass::EuropeanNumber), // Nd  [10] FULLWIDTH DIGIT ZERO..FULLWIDTH DIGIT NINE
    ('\u{ff1a}', '\u{ff1a}', BidiClass::CommonSeparator), // Po       FULLWIDTH COLON
    ('\u{ff1b}', '\u{ff1b}', BidiClass::OtherNeutral),   // Po       FULLWIDTH SEMICOLON
    ('\u{ff1c}', '\u{ff1e}', BidiClass::OtherNeutral), // Sm   [3] FULLWIDTH LESS-THAN SIGN..FULLWIDTH GREATER-THAN SIGN
    ('\u{ff1f}', '\u{ff20}', BidiClass::OtherNeutral), // Po   [2] FULLWIDTH QUESTION MARK..FULLWIDTH COMMERCIAL AT
    ('\u{ff21}', '\u{ff3a}', BidiClass::LeftToRight), // L&  [26] FULLWIDTH LATIN CAPITAL LETTER A..FULLWIDTH LATIN CAPITAL LETTER Z
    ('\u{ff3b}', '\u{ff3b}', BidiClass::OtherNeutral), // Ps       FULLWIDTH LEFT SQUARE BRACKET
    ('\u{ff3c}', '\u{ff3c}', BidiClass::OtherNeutral), // Po       FULLWIDTH REVERSE SOLIDUS
    ('\u{ff3d}', '\u{ff3d}', BidiClass::OtherNeutral), // Pe       FULLWIDTH RIGHT SQUARE BRACKET
    ('\u{ff3e}', '\u{ff3e}', BidiClass::OtherNeutral), // Sk       FULLWIDTH CIRCUMFLEX ACCENT
    ('\u{ff3f}', '\u{ff3f}', BidiClass::OtherNeutral), // Pc       FULLWIDTH LOW LINE
    ('\u{ff40}', '\u{ff40}', BidiClass::OtherNeutral), // Sk       FULLWIDTH GRAVE ACCENT
    ('\u{ff41}', '\u{ff5a}', BidiClass::LeftToRight), // L&  [26] FULLWIDTH LATIN SMALL LETTER A..FULLWIDTH LATIN SMALL LETTER Z
    ('\u{ff5b}', '\u{ff5b}', BidiClass::OtherNeutral), // Ps       FULLWIDTH LEFT CURLY BRACKET
    ('\u{ff5c}', '\u{ff5c}', BidiClass::OtherNeutral), // Sm       FULLWIDTH VERTICAL LINE
    ('\u{ff5d}', '\u{ff5d}', BidiClass::OtherNeutral), // Pe       FULLWIDTH RIGHT CURLY BRACKET
    ('\u{ff5e}', '\u{ff5e}', BidiClass::OtherNeutral), // Sm       FULLWIDTH TILDE
    ('\u{ff5f}', '\u{ff5f}', BidiClass::OtherNeutral), // Ps       FULLWIDTH LEFT WHITE PARENTHESIS
    ('\u{ff60}', '\u{ff60}', BidiClass::OtherNeutral), // Pe       FULLWIDTH RIGHT WHITE PARENTHESIS
    ('\u{ff61}', '\u{ff61}', BidiClass::OtherNeutral), // Po       HALFWIDTH IDEOGRAPHIC FULL STOP
    ('\u{ff62}', '\u{ff62}', BidiClass::OtherNeutral), // Ps       HALFWIDTH LEFT CORNER BRACKET
    ('\u{ff63}', '\u{ff63}', BidiClass::OtherNeutral), // Pe       HALFWIDTH RIGHT CORNER BRACKET
    ('\u{ff64}', '\u{ff65}', BidiClass::OtherNeutral), // Po   [2] HALFWIDTH IDEOGRAPHIC COMMA..HALFWIDTH KATAKANA MIDDLE DOT
    ('\u{ff66}', '\u{ff6f}', BidiClass::LeftToRight), // Lo  [10] HALFWIDTH KATAKANA LETTER WO..HALFWIDTH KATAKANA LETTER SMALL TU
    ('\u{ff70}', '\u{ff70}', BidiClass::LeftToRight), // Lm       HALFWIDTH KATAKANA-HIRAGANA PROLONGED SOUND MARK
    ('\u{ff71}', '\u{ff9d}', BidiClass::LeftToRight), // Lo  [45] HALFWIDTH KATAKANA LETTER A..HALFWIDTH KATAKANA LETTER N
    ('\u{ff9e}', '\u{ff9f}', BidiClass::LeftToRight), // Lm   [2] HALFWIDTH KATAKANA VOICED SOUND MARK..HALFWIDTH KATAKANA SEMI-VOICED SOUND MARK
    ('\u{ffa0}', '\u{ffbe}', BidiClass::LeftToRight), // Lo  [31] HALFWIDTH HANGUL FILLER..HALFWIDTH HANGUL LETTER HIEUH
    ('\u{ffc2}', '\u{ffc7}', BidiClass::LeftToRight), // Lo   [6] HALFWIDTH HANGUL LETTER A..HALFWIDTH HANGUL LETTER E
    ('\u{ffca}', '\u{ffcf}', BidiClass::LeftToRight), // Lo   [6] HALFWIDTH HANGUL LETTER YEO..HALFWIDTH HANGUL LETTER OE
    ('\u{ffd2}', '\u{ffd7}', BidiClass::LeftToRight), // Lo   [6] HALFWIDTH HANGUL LETTER YO..HALFWIDTH HANGUL LETTER YU
    ('\u{ffda}', '\u{ffdc}', BidiClass::LeftToRight), // Lo   [3] HALFWIDTH HANGUL LETTER EU..HALFWIDTH HANGUL LETTER I
    ('\u{ffe0}', '\u{ffe1}', BidiClass::EuropeanTerminator), // Sc   [2] FULLWIDTH CENT SIGN..FULLWIDTH POUND SIGN
    ('\u{ffe2}', '\u{ffe2}', BidiClass::OtherNeutral),       // Sm       FULLWIDTH NOT SIGN
    ('\u{ffe3}', '\u{ffe3}', BidiClass::OtherNeutral),       // Sk       FULLWIDTH MACRON
    ('\u{ffe4}', '\u{ffe4}', BidiClass::OtherNeutral),       // So       FULLWIDTH BROKEN BAR
    ('\u{ffe5}', '\u{ffe6}', BidiClass::EuropeanTerminator), // Sc   [2] FULLWIDTH YEN SIGN..FULLWIDTH WON SIGN
    ('\u{ffe8}', '\u{ffe8}', BidiClass::OtherNeutral), // So       HALFWIDTH FORMS LIGHT VERTICAL
    ('\u{ffe9}', '\u{ffec}', BidiClass::OtherNeutral), // Sm   [4] HALFWIDTH LEFTWARDS ARROW..HALFWIDTH DOWNWARDS ARROW
    ('\u{ffed}', '\u{ffee}', BidiClass::OtherNeutral), // So   [2] HALFWIDTH BLACK SQUARE..HALFWIDTH WHITE CIRCLE
    ('\u{fff0}', '\u{fff8}', BidiClass::BoundaryNeutral), // Cn   [9] <reserved-FFF0>..<reserved-FFF8>
    ('\u{fff9}', '\u{fffb}', BidiClass::OtherNeutral), // Cf   [3] INTERLINEAR ANNOTATION ANCHOR..INTERLINEAR ANNOTATION TERMINATOR
    ('\u{fffc}', '\u{fffd}', BidiClass::OtherNeutral), // So   [2] OBJECT REPLACEMENT CHARACTER..REPLACEMENT CHARACTER
    ('\u{fffe}', '\u{ffff}', BidiClass::BoundaryNeutral), // Cn   [2] <noncharacter-FFFE>..<noncharacter-FFFF>
    ('\u{10000}', '\u{1000b}', BidiClass::LeftToRight), // Lo  [12] LINEAR B SYLLABLE B008 A..LINEAR B SYLLABLE B046 JE
    ('\u{1000d}', '\u{10026}', BidiClass::LeftToRight), // Lo  [26] LINEAR B SYLLABLE B036 JO..LINEAR B SYLLABLE B032 QO
    ('\u{10028}', '\u{1003a}', BidiClass::LeftToRight), // Lo  [19] LINEAR B SYLLABLE B060 RA..LINEAR B SYLLABLE B042 WO
    ('\u{1003c}', '\u{1003d}', BidiClass::LeftToRight), // Lo   [2] LINEAR B SYLLABLE B017 ZA..LINEAR B SYLLABLE B074 ZE
    ('\u{1003f}', '\u{1004d}', BidiClass::LeftToRight), // Lo  [15] LINEAR B SYLLABLE B020 ZO..LINEAR B SYLLABLE B091 TWO
    ('\u{10050}', '\u{1005d}', BidiClass::LeftToRight), // Lo  [14] LINEAR B SYMBOL B018..LINEAR B SYMBOL B089
    ('\u{10080}', '\u{100fa}', BidiClass::LeftToRight), // Lo [123] LINEAR B IDEOGRAM B100 MAN..LINEAR B IDEOGRAM VESSEL B305
    ('\u{10100}', '\u{10100}', BidiClass::LeftToRight), // Po       AEGEAN WORD SEPARATOR LINE
    ('\u{10101}', '\u{10101}', BidiClass::OtherNeutral), // Po       AEGEAN WORD SEPARATOR DOT
    ('\u{10102}', '\u{10102}', BidiClass::LeftToRight), // Po       AEGEAN CHECK MARK
    ('\u{10107}', '\u{10133}', BidiClass::LeftToRight), // No  [45] AEGEAN NUMBER ONE..AEGEAN NUMBER NINETY THOUSAND
    ('\u{10137}', '\u{1013f}', BidiClass::LeftToRight), // So   [9] AEGEAN WEIGHT BASE UNIT..AEGEAN MEASURE THIRD SUBUNIT
    ('\u{10140}', '\u{10174}', BidiClass::OtherNeutral), // Nl  [53] GREEK ACROPHONIC ATTIC ONE QUARTER..GREEK ACROPHONIC STRATIAN FIFTY MNAS
    ('\u{10175}', '\u{10178}', BidiClass::OtherNeutral), // No   [4] GREEK ONE HALF SIGN..GREEK THREE QUARTERS SIGN
    ('\u{10179}', '\u{10189}', BidiClass::OtherNeutral), // So  [17] GREEK YEAR SIGN..GREEK TRYBLION BASE SIGN
    ('\u{1018a}', '\u{1018b}', BidiClass::OtherNeutral), // No   [2] GREEK ZERO SIGN..GREEK ONE QUARTER SIGN
    ('\u{1018c}', '\u{1018c}', BidiClass::OtherNeutral), // So       GREEK SINUSOID SIGN
    ('\u{1018d}', '\u{1018e}', BidiClass::LeftToRight), // So   [2] GREEK INDICTION SIGN..NOMISMA SIGN
    ('\u{10190}', '\u{1019c}', BidiClass::OtherNeutral), // So  [13] ROMAN SEXTANS SIGN..ASCIA SYMBOL
    ('\u{101a0}', '\u{101a0}', BidiClass::OtherNeutral), // So       GREEK SYMBOL TAU RHO
    ('\u{101d0}', '\u{101fc}', BidiClass::LeftToRight), // So  [45] PHAISTOS DISC SIGN PEDESTRIAN..PHAISTOS DISC SIGN WAVY BAND
    ('\u{101fd}', '\u{101fd}', BidiClass::NonspacingMark), // Mn       PHAISTOS DISC SIGN COMBINING OBLIQUE STROKE
    ('\u{10280}', '\u{1029c}', BidiClass::LeftToRight), // Lo  [29] LYCIAN LETTER A..LYCIAN LETTER X
    ('\u{102a0}', '\u{102d0}', BidiClass::LeftToRight), // Lo  [49] CARIAN LETTER A..CARIAN LETTER UUU3
    ('\u{102e0}', '\u{102e0}', BidiClass::NonspacingMark), // Mn       COPTIC EPACT THOUSANDS MARK
    ('\u{102e1}', '\u{102fb}', BidiClass::EuropeanNumber), // No  [27] COPTIC EPACT DIGIT ONE..COPTIC EPACT NUMBER NINE HUNDRED
    ('\u{10300}', '\u{1031f}', BidiClass::LeftToRight), // Lo  [32] OLD ITALIC LETTER A..OLD ITALIC LETTER ESS
    ('\u{10320}', '\u{10323}', BidiClass::LeftToRight), // No   [4] OLD ITALIC NUMERAL ONE..OLD ITALIC NUMERAL FIFTY
    ('\u{1032d}', '\u{10340}', BidiClass::LeftToRight), // Lo  [20] OLD ITALIC LETTER YE..GOTHIC LETTER PAIRTHRA
    ('\u{10341}', '\u{10341}', BidiClass::LeftToRight), // Nl       GOTHIC LETTER NINETY
    ('\u{10342}', '\u{10349}', BidiClass::LeftToRight), // Lo   [8] GOTHIC LETTER RAIDA..GOTHIC LETTER OTHAL
    ('\u{1034a}', '\u{1034a}', BidiClass::LeftToRight), // Nl       GOTHIC LETTER NINE HUNDRED
    ('\u{10350}', '\u{10375}', BidiClass::LeftToRight), // Lo  [38] OLD PERMIC LETTER AN..OLD PERMIC LETTER IA
    ('\u{10376}', '\u{1037a}', BidiClass::NonspacingMark), // Mn   [5] COMBINING OLD PERMIC LETTER AN..COMBINING OLD PERMIC LETTER SII
    ('\u{10380}', '\u{1039d}', BidiClass::LeftToRight), // Lo  [30] UGARITIC LETTER ALPA..UGARITIC LETTER SSU
    ('\u{1039f}', '\u{1039f}', BidiClass::LeftToRight), // Po       UGARITIC WORD DIVIDER
    ('\u{103a0}', '\u{103c3}', BidiClass::LeftToRight), // Lo  [36] OLD PERSIAN SIGN A..OLD PERSIAN SIGN HA
    ('\u{103c8}', '\u{103cf}', BidiClass::LeftToRight), // Lo   [8] OLD PERSIAN SIGN AURAMAZDAA..OLD PERSIAN SIGN BUUMISH
    ('\u{103d0}', '\u{103d0}', BidiClass::LeftToRight), // Po       OLD PERSIAN WORD DIVIDER
    ('\u{103d1}', '\u{103d5}', BidiClass::LeftToRight), // Nl   [5] OLD PERSIAN NUMBER ONE..OLD PERSIAN NUMBER HUNDRED
    ('\u{10400}', '\u{1044f}', BidiClass::LeftToRight), // L&  [80] DESERET CAPITAL LETTER LONG I..DESERET SMALL LETTER EW
    ('\u{10450}', '\u{1049d}', BidiClass::LeftToRight), // Lo  [78] SHAVIAN LETTER PEEP..OSMANYA LETTER OO
    ('\u{104a0}', '\u{104a9}', BidiClass::LeftToRight), // Nd  [10] OSMANYA DIGIT ZERO..OSMANYA DIGIT NINE
    ('\u{104b0}', '\u{104d3}', BidiClass::LeftToRight), // L&  [36] OSAGE CAPITAL LETTER A..OSAGE CAPITAL LETTER ZHA
    ('\u{104d8}', '\u{104fb}', BidiClass::LeftToRight), // L&  [36] OSAGE SMALL LETTER A..OSAGE SMALL LETTER ZHA
    ('\u{10500}', '\u{10527}', BidiClass::LeftToRight), // Lo  [40] ELBASAN LETTER A..ELBASAN LETTER KHE
    ('\u{10530}', '\u{10563}', BidiClass::LeftToRight), // Lo  [52] CAUCASIAN ALBANIAN LETTER ALT..CAUCASIAN ALBANIAN LETTER KIW
    ('\u{1056f}', '\u{1056f}', BidiClass::LeftToRight), // Po       CAUCASIAN ALBANIAN CITATION MARK
    ('\u{10570}', '\u{1057a}', BidiClass::LeftToRight), // L&  [11] VITHKUQI CAPITAL LETTER A..VITHKUQI CAPITAL LETTER GA
    ('\u{1057c}', '\u{1058a}', BidiClass::LeftToRight), // L&  [15] VITHKUQI CAPITAL LETTER HA..VITHKUQI CAPITAL LETTER RE
    ('\u{1058c}', '\u{10592}', BidiClass::LeftToRight), // L&   [7] VITHKUQI CAPITAL LETTER SE..VITHKUQI CAPITAL LETTER XE
    ('\u{10594}', '\u{10595}', BidiClass::LeftToRight), // L&   [2] VITHKUQI CAPITAL LETTER Y..VITHKUQI CAPITAL LETTER ZE
    ('\u{10597}', '\u{105a1}', BidiClass::LeftToRight), // L&  [11] VITHKUQI SMALL LETTER A..VITHKUQI SMALL LETTER GA
    ('\u{105a3}', '\u{105b1}', BidiClass::LeftToRight), // L&  [15] VITHKUQI SMALL LETTER HA..VITHKUQI SMALL LETTER RE
    ('\u{105b3}', '\u{105b9}', BidiClass::LeftToRight), // L&   [7] VITHKUQI SMALL LETTER SE..VITHKUQI SMALL LETTER XE
    ('\u{105bb}', '\u{105bc}', BidiClass::LeftToRight), // L&   [2] VITHKUQI SMALL LETTER Y..VITHKUQI SMALL LETTER ZE
    ('\u{10600}', '\u{10736}', BidiClass::LeftToRight), // Lo [311] LINEAR A SIGN AB001..LINEAR A SIGN A664
    ('\u{10740}', '\u{10755}', BidiClass::LeftToRight), // Lo  [22] LINEAR A SIGN A701 A..LINEAR A SIGN A732 JE
    ('\u{10760}', '\u{10767}', BidiClass::LeftToRight), // Lo   [8] LINEAR A SIGN A800..LINEAR A SIGN A807
    ('\u{10780}', '\u{10785}', BidiClass::LeftToRight), // Lm   [6] MODIFIER LETTER SMALL CAPITAL AA..MODIFIER LETTER SMALL B WITH HOOK
    ('\u{10787}', '\u{107b0}', BidiClass::LeftToRight), // Lm  [42] MODIFIER LETTER SMALL DZ DIGRAPH..MODIFIER LETTER SMALL V WITH RIGHT HOOK
    ('\u{107b2}', '\u{107ba}', BidiClass::LeftToRight), // Lm   [9] MODIFIER LETTER SMALL CAPITAL Y..MODIFIER LETTER SMALL S WITH CURL
    ('\u{10800}', '\u{10805}', BidiClass::RightToLeft), // Lo   [6] CYPRIOT SYLLABLE A..CYPRIOT SYLLABLE JA
    ('\u{10806}', '\u{10807}', BidiClass::RightToLeft), // Cn   [2] <reserved-10806>..<reserved-10807>
    ('\u{10808}', '\u{10808}', BidiClass::RightToLeft), // Lo       CYPRIOT SYLLABLE JO
    ('\u{10809}', '\u{10809}', BidiClass::RightToLeft), // Cn       <reserved-10809>
    ('\u{1080a}', '\u{10835}', BidiClass::RightToLeft), // Lo  [44] CYPRIOT SYLLABLE KA..CYPRIOT SYLLABLE WO
    ('\u{10836}', '\u{10836}', BidiClass::RightToLeft), // Cn       <reserved-10836>
    ('\u{10837}', '\u{10838}', BidiClass::RightToLeft), // Lo   [2] CYPRIOT SYLLABLE XA..CYPRIOT SYLLABLE XE
    ('\u{10839}', '\u{1083b}', BidiClass::RightToLeft), // Cn   [3] <reserved-10839>..<reserved-1083B>
    ('\u{1083c}', '\u{1083c}', BidiClass::RightToLeft), // Lo       CYPRIOT SYLLABLE ZA
    ('\u{1083d}', '\u{1083e}', BidiClass::RightToLeft), // Cn   [2] <reserved-1083D>..<reserved-1083E>
    ('\u{1083f}', '\u{10855}', BidiClass::RightToLeft), // Lo  [23] CYPRIOT SYLLABLE ZO..IMPERIAL ARAMAIC LETTER TAW
    ('\u{10856}', '\u{10856}', BidiClass::RightToLeft), // Cn       <reserved-10856>
    ('\u{10857}', '\u{10857}', BidiClass::RightToLeft), // Po       IMPERIAL ARAMAIC SECTION SIGN
    ('\u{10858}', '\u{1085f}', BidiClass::RightToLeft), // No   [8] IMPERIAL ARAMAIC NUMBER ONE..IMPERIAL ARAMAIC NUMBER TEN THOUSAND
    ('\u{10860}', '\u{10876}', BidiClass::RightToLeft), // Lo  [23] PALMYRENE LETTER ALEPH..PALMYRENE LETTER TAW
    ('\u{10877}', '\u{10878}', BidiClass::RightToLeft), // So   [2] PALMYRENE LEFT-POINTING FLEURON..PALMYRENE RIGHT-POINTING FLEURON
    ('\u{10879}', '\u{1087f}', BidiClass::RightToLeft), // No   [7] PALMYRENE NUMBER ONE..PALMYRENE NUMBER TWENTY
    ('\u{10880}', '\u{1089e}', BidiClass::RightToLeft), // Lo  [31] NABATAEAN LETTER FINAL ALEPH..NABATAEAN LETTER TAW
    ('\u{1089f}', '\u{108a6}', BidiClass::RightToLeft), // Cn   [8] <reserved-1089F>..<reserved-108A6>
    ('\u{108a7}', '\u{108af}', BidiClass::RightToLeft), // No   [9] NABATAEAN NUMBER ONE..NABATAEAN NUMBER ONE HUNDRED
    ('\u{108b0}', '\u{108df}', BidiClass::RightToLeft), // Cn  [48] <reserved-108B0>..<reserved-108DF>
    ('\u{108e0}', '\u{108f2}', BidiClass::RightToLeft), // Lo  [19] HATRAN LETTER ALEPH..HATRAN LETTER QOPH
    ('\u{108f3}', '\u{108f3}', BidiClass::RightToLeft), // Cn       <reserved-108F3>
    ('\u{108f4}', '\u{108f5}', BidiClass::RightToLeft), // Lo   [2] HATRAN LETTER SHIN..HATRAN LETTER TAW
    ('\u{108f6}', '\u{108fa}', BidiClass::RightToLeft), // Cn   [5] <reserved-108F6>..<reserved-108FA>
    ('\u{108fb}', '\u{108ff}', BidiClass::RightToLeft), // No   [5] HATRAN NUMBER ONE..HATRAN NUMBER ONE HUNDRED
    ('\u{10900}', '\u{10915}', BidiClass::RightToLeft), // Lo  [22] PHOENICIAN LETTER ALF..PHOENICIAN LETTER TAU
    ('\u{10916}', '\u{1091b}', BidiClass::RightToLeft), // No   [6] PHOENICIAN NUMBER ONE..PHOENICIAN NUMBER THREE
    ('\u{1091c}', '\u{1091e}', BidiClass::RightToLeft), // Cn   [3] <reserved-1091C>..<reserved-1091E>
    ('\u{1091f}', '\u{1091f}', BidiClass::OtherNeutral), // Po       PHOENICIAN WORD SEPARATOR
    ('\u{10920}', '\u{10939}', BidiClass::RightToLeft), // Lo  [26] LYDIAN LETTER A..LYDIAN LETTER C
    ('\u{1093a}', '\u{1093e}', BidiClass::RightToLeft), // Cn   [5] <reserved-1093A>..<reserved-1093E>
    ('\u{1093f}', '\u{1093f}', BidiClass::RightToLeft), // Po       LYDIAN TRIANGULAR MARK
    ('\u{10940}', '\u{1097f}', BidiClass::RightToLeft), // Cn  [64] <reserved-10940>..<reserved-1097F>
    ('\u{10980}', '\u{109b7}', BidiClass::RightToLeft), // Lo  [56] MEROITIC HIEROGLYPHIC LETTER A..MEROITIC CURSIVE LETTER DA
    ('\u{109b8}', '\u{109bb}', BidiClass::RightToLeft), // Cn   [4] <reserved-109B8>..<reserved-109BB>
    ('\u{109bc}', '\u{109bd}', BidiClass::RightToLeft), // No   [2] MEROITIC CURSIVE FRACTION ELEVEN TWELFTHS..MEROITIC CURSIVE FRACTION ONE HALF
    ('\u{109be}', '\u{109bf}', BidiClass::RightToLeft), // Lo   [2] MEROITIC CURSIVE LOGOGRAM RMT..MEROITIC CURSIVE LOGOGRAM IMN
    ('\u{109c0}', '\u{109cf}', BidiClass::RightToLeft), // No  [16] MEROITIC CURSIVE NUMBER ONE..MEROITIC CURSIVE NUMBER SEVENTY
    ('\u{109d0}', '\u{109d1}', BidiClass::RightToLeft), // Cn   [2] <reserved-109D0>..<reserved-109D1>
    ('\u{109d2}', '\u{109ff}', BidiClass::RightToLeft), // No  [46] MEROITIC CURSIVE NUMBER ONE HUNDRED..MEROITIC CURSIVE FRACTION TEN TWELFTHS
    ('\u{10a00}', '\u{10a00}', BidiClass::RightToLeft), // Lo       KHAROSHTHI LETTER A
    ('\u{10a01}', '\u{10a03}', BidiClass::NonspacingMark), // Mn   [3] KHAROSHTHI VOWEL SIGN I..KHAROSHTHI VOWEL SIGN VOCALIC R
    ('\u{10a04}', '\u{10a04}', BidiClass::RightToLeft),    // Cn       <reserved-10A04>
    ('\u{10a05}', '\u{10a06}', BidiClass::NonspacingMark), // Mn   [2] KHAROSHTHI VOWEL SIGN E..KHAROSHTHI VOWEL SIGN O
    ('\u{10a07}', '\u{10a0b}', BidiClass::RightToLeft), // Cn   [5] <reserved-10A07>..<reserved-10A0B>
    ('\u{10a0c}', '\u{10a0f}', BidiClass::NonspacingMark), // Mn   [4] KHAROSHTHI VOWEL LENGTH MARK..KHAROSHTHI SIGN VISARGA
    ('\u{10a10}', '\u{10a13}', BidiClass::RightToLeft), // Lo   [4] KHAROSHTHI LETTER KA..KHAROSHTHI LETTER GHA
    ('\u{10a14}', '\u{10a14}', BidiClass::RightToLeft), // Cn       <reserved-10A14>
    ('\u{10a15}', '\u{10a17}', BidiClass::RightToLeft), // Lo   [3] KHAROSHTHI LETTER CA..KHAROSHTHI LETTER JA
    ('\u{10a18}', '\u{10a18}', BidiClass::RightToLeft), // Cn       <reserved-10A18>
    ('\u{10a19}', '\u{10a35}', BidiClass::RightToLeft), // Lo  [29] KHAROSHTHI LETTER NYA..KHAROSHTHI LETTER VHA
    ('\u{10a36}', '\u{10a37}', BidiClass::RightToLeft), // Cn   [2] <reserved-10A36>..<reserved-10A37>
    ('\u{10a38}', '\u{10a3a}', BidiClass::NonspacingMark), // Mn   [3] KHAROSHTHI SIGN BAR ABOVE..KHAROSHTHI SIGN DOT BELOW
    ('\u{10a3b}', '\u{10a3e}', BidiClass::RightToLeft), // Cn   [4] <reserved-10A3B>..<reserved-10A3E>
    ('\u{10a3f}', '\u{10a3f}', BidiClass::NonspacingMark), // Mn       KHAROSHTHI VIRAMA
    ('\u{10a40}', '\u{10a48}', BidiClass::RightToLeft), // No   [9] KHAROSHTHI DIGIT ONE..KHAROSHTHI FRACTION ONE HALF
    ('\u{10a49}', '\u{10a4f}', BidiClass::RightToLeft), // Cn   [7] <reserved-10A49>..<reserved-10A4F>
    ('\u{10a50}', '\u{10a58}', BidiClass::RightToLeft), // Po   [9] KHAROSHTHI PUNCTUATION DOT..KHAROSHTHI PUNCTUATION LINES
    ('\u{10a59}', '\u{10a5f}', BidiClass::RightToLeft), // Cn   [7] <reserved-10A59>..<reserved-10A5F>
    ('\u{10a60}', '\u{10a7c}', BidiClass::RightToLeft), // Lo  [29] OLD SOUTH ARABIAN LETTER HE..OLD SOUTH ARABIAN LETTER THETH
    ('\u{10a7d}', '\u{10a7e}', BidiClass::RightToLeft), // No   [2] OLD SOUTH ARABIAN NUMBER ONE..OLD SOUTH ARABIAN NUMBER FIFTY
    ('\u{10a7f}', '\u{10a7f}', BidiClass::RightToLeft), // Po       OLD SOUTH ARABIAN NUMERIC INDICATOR
    ('\u{10a80}', '\u{10a9c}', BidiClass::RightToLeft), // Lo  [29] OLD NORTH ARABIAN LETTER HEH..OLD NORTH ARABIAN LETTER ZAH
    ('\u{10a9d}', '\u{10a9f}', BidiClass::RightToLeft), // No   [3] OLD NORTH ARABIAN NUMBER ONE..OLD NORTH ARABIAN NUMBER TWENTY
    ('\u{10aa0}', '\u{10abf}', BidiClass::RightToLeft), // Cn  [32] <reserved-10AA0>..<reserved-10ABF>
    ('\u{10ac0}', '\u{10ac7}', BidiClass::RightToLeft), // Lo   [8] MANICHAEAN LETTER ALEPH..MANICHAEAN LETTER WAW
    ('\u{10ac8}', '\u{10ac8}', BidiClass::RightToLeft), // So       MANICHAEAN SIGN UD
    ('\u{10ac9}', '\u{10ae4}', BidiClass::RightToLeft), // Lo  [28] MANICHAEAN LETTER ZAYIN..MANICHAEAN LETTER TAW
    ('\u{10ae5}', '\u{10ae6}', BidiClass::NonspacingMark), // Mn   [2] MANICHAEAN ABBREVIATION MARK ABOVE..MANICHAEAN ABBREVIATION MARK BELOW
    ('\u{10ae7}', '\u{10aea}', BidiClass::RightToLeft), // Cn   [4] <reserved-10AE7>..<reserved-10AEA>
    ('\u{10aeb}', '\u{10aef}', BidiClass::RightToLeft), // No   [5] MANICHAEAN NUMBER ONE..MANICHAEAN NUMBER ONE HUNDRED
    ('\u{10af0}', '\u{10af6}', BidiClass::RightToLeft), // Po   [7] MANICHAEAN PUNCTUATION STAR..MANICHAEAN PUNCTUATION LINE FILLER
    ('\u{10af7}', '\u{10aff}', BidiClass::RightToLeft), // Cn   [9] <reserved-10AF7>..<reserved-10AFF>
    ('\u{10b00}', '\u{10b35}', BidiClass::RightToLeft), // Lo  [54] AVESTAN LETTER A..AVESTAN LETTER HE
    ('\u{10b36}', '\u{10b38}', BidiClass::RightToLeft), // Cn   [3] <reserved-10B36>..<reserved-10B38>
    ('\u{10b39}', '\u{10b3f}', BidiClass::OtherNeutral), // Po   [7] AVESTAN ABBREVIATION MARK..LARGE ONE RING OVER TWO RINGS PUNCTUATION
    ('\u{10b40}', '\u{10b55}', BidiClass::RightToLeft), // Lo  [22] INSCRIPTIONAL PARTHIAN LETTER ALEPH..INSCRIPTIONAL PARTHIAN LETTER TAW
    ('\u{10b56}', '\u{10b57}', BidiClass::RightToLeft), // Cn   [2] <reserved-10B56>..<reserved-10B57>
    ('\u{10b58}', '\u{10b5f}', BidiClass::RightToLeft), // No   [8] INSCRIPTIONAL PARTHIAN NUMBER ONE..INSCRIPTIONAL PARTHIAN NUMBER ONE THOUSAND
    ('\u{10b60}', '\u{10b72}', BidiClass::RightToLeft), // Lo  [19] INSCRIPTIONAL PAHLAVI LETTER ALEPH..INSCRIPTIONAL PAHLAVI LETTER TAW
    ('\u{10b73}', '\u{10b77}', BidiClass::RightToLeft), // Cn   [5] <reserved-10B73>..<reserved-10B77>
    ('\u{10b78}', '\u{10b7f}', BidiClass::RightToLeft), // No   [8] INSCRIPTIONAL PAHLAVI NUMBER ONE..INSCRIPTIONAL PAHLAVI NUMBER ONE THOUSAND
    ('\u{10b80}', '\u{10b91}', BidiClass::RightToLeft), // Lo  [18] PSALTER PAHLAVI LETTER ALEPH..PSALTER PAHLAVI LETTER TAW
    ('\u{10b92}', '\u{10b98}', BidiClass::RightToLeft), // Cn   [7] <reserved-10B92>..<reserved-10B98>
    ('\u{10b99}', '\u{10b9c}', BidiClass::RightToLeft), // Po   [4] PSALTER PAHLAVI SECTION MARK..PSALTER PAHLAVI FOUR DOTS WITH DOT
    ('\u{10b9d}', '\u{10ba8}', BidiClass::RightToLeft), // Cn  [12] <reserved-10B9D>..<reserved-10BA8>
    ('\u{10ba9}', '\u{10baf}', BidiClass::RightToLeft), // No   [7] PSALTER PAHLAVI NUMBER ONE..PSALTER PAHLAVI NUMBER ONE HUNDRED
    ('\u{10bb0}', '\u{10bff}', BidiClass::RightToLeft), // Cn  [80] <reserved-10BB0>..<reserved-10BFF>
    ('\u{10c00}', '\u{10c48}', BidiClass::RightToLeft), // Lo  [73] OLD TURKIC LETTER ORKHON A..OLD TURKIC LETTER ORKHON BASH
    ('\u{10c49}', '\u{10c7f}', BidiClass::RightToLeft), // Cn  [55] <reserved-10C49>..<reserved-10C7F>
    ('\u{10c80}', '\u{10cb2}', BidiClass::RightToLeft), // L&  [51] OLD HUNGARIAN CAPITAL LETTER A..OLD HUNGARIAN CAPITAL LETTER US
    ('\u{10cb3}', '\u{10cbf}', BidiClass::RightToLeft), // Cn  [13] <reserved-10CB3>..<reserved-10CBF>
    ('\u{10cc0}', '\u{10cf2}', BidiClass::RightToLeft), // L&  [51] OLD HUNGARIAN SMALL LETTER A..OLD HUNGARIAN SMALL LETTER US
    ('\u{10cf3}', '\u{10cf9}', BidiClass::RightToLeft), // Cn   [7] <reserved-10CF3>..<reserved-10CF9>
    ('\u{10cfa}', '\u{10cff}', BidiClass::RightToLeft), // No   [6] OLD HUNGARIAN NUMBER ONE..OLD HUNGARIAN NUMBER ONE THOUSAND
    ('\u{10d00}', '\u{10d23}', BidiClass::ArabicLetter), // Lo  [36] HANIFI ROHINGYA LETTER A..HANIFI ROHINGYA MARK NA KHONNA
    ('\u{10d24}', '\u{10d27}', BidiClass::NonspacingMark), // Mn   [4] HANIFI ROHINGYA SIGN HARBAHAY..HANIFI ROHINGYA SIGN TASSI
    ('\u{10d28}', '\u{10d2f}', BidiClass::ArabicLetter), // Cn   [8] <reserved-10D28>..<reserved-10D2F>
    ('\u{10d30}', '\u{10d39}', BidiClass::ArabicNumber), // Nd  [10] HANIFI ROHINGYA DIGIT ZERO..HANIFI ROHINGYA DIGIT NINE
    ('\u{10d3a}', '\u{10d3f}', BidiClass::ArabicLetter), // Cn   [6] <reserved-10D3A>..<reserved-10D3F>
    ('\u{10d40}', '\u{10e5f}', BidiClass::RightToLeft), // Cn [288] <reserved-10D40>..<reserved-10E5F>
    ('\u{10e60}', '\u{10e7e}', BidiClass::ArabicNumber), // No  [31] RUMI DIGIT ONE..RUMI FRACTION TWO THIRDS
    ('\u{10e7f}', '\u{10e7f}', BidiClass::RightToLeft),  // Cn       <reserved-10E7F>
    ('\u{10e80}', '\u{10ea9}', BidiClass::RightToLeft), // Lo  [42] YEZIDI LETTER ELIF..YEZIDI LETTER ET
    ('\u{10eaa}', '\u{10eaa}', BidiClass::RightToLeft), // Cn       <reserved-10EAA>
    ('\u{10eab}', '\u{10eac}', BidiClass::NonspacingMark), // Mn   [2] YEZIDI COMBINING HAMZA MARK..YEZIDI COMBINING MADDA MARK
    ('\u{10ead}', '\u{10ead}', BidiClass::RightToLeft),    // Pd       YEZIDI HYPHENATION MARK
    ('\u{10eae}', '\u{10eaf}', BidiClass::RightToLeft), // Cn   [2] <reserved-10EAE>..<reserved-10EAF>
    ('\u{10eb0}', '\u{10eb1}', BidiClass::RightToLeft), // Lo   [2] YEZIDI LETTER LAM WITH DOT ABOVE..YEZIDI LETTER YOT WITH CIRCUMFLEX ABOVE
    ('\u{10eb2}', '\u{10eff}', BidiClass::RightToLeft), // Cn  [78] <reserved-10EB2>..<reserved-10EFF>
    ('\u{10f00}', '\u{10f1c}', BidiClass::RightToLeft), // Lo  [29] OLD SOGDIAN LETTER ALEPH..OLD SOGDIAN LETTER FINAL TAW WITH VERTICAL TAIL
    ('\u{10f1d}', '\u{10f26}', BidiClass::RightToLeft), // No  [10] OLD SOGDIAN NUMBER ONE..OLD SOGDIAN FRACTION ONE HALF
    ('\u{10f27}', '\u{10f27}', BidiClass::RightToLeft), // Lo       OLD SOGDIAN LIGATURE AYIN-DALETH
    ('\u{10f28}', '\u{10f2f}', BidiClass::RightToLeft), // Cn   [8] <reserved-10F28>..<reserved-10F2F>
    ('\u{10f30}', '\u{10f45}', BidiClass::ArabicLetter), // Lo  [22] SOGDIAN LETTER ALEPH..SOGDIAN INDEPENDENT SHIN
    ('\u{10f46}', '\u{10f50}', BidiClass::NonspacingMark), // Mn  [11] SOGDIAN COMBINING DOT BELOW..SOGDIAN COMBINING STROKE BELOW
    ('\u{10f51}', '\u{10f54}', BidiClass::ArabicLetter), // No   [4] SOGDIAN NUMBER ONE..SOGDIAN NUMBER ONE HUNDRED
    ('\u{10f55}', '\u{10f59}', BidiClass::ArabicLetter), // Po   [5] SOGDIAN PUNCTUATION TWO VERTICAL BARS..SOGDIAN PUNCTUATION HALF CIRCLE WITH DOT
    ('\u{10f5a}', '\u{10f6f}', BidiClass::ArabicLetter), // Cn  [22] <reserved-10F5A>..<reserved-10F6F>
    ('\u{10f70}', '\u{10f81}', BidiClass::RightToLeft), // Lo  [18] OLD UYGHUR LETTER ALEPH..OLD UYGHUR LETTER LESH
    ('\u{10f82}', '\u{10f85}', BidiClass::NonspacingMark), // Mn   [4] OLD UYGHUR COMBINING DOT ABOVE..OLD UYGHUR COMBINING TWO DOTS BELOW
    ('\u{10f86}', '\u{10f89}', BidiClass::RightToLeft), // Po   [4] OLD UYGHUR PUNCTUATION BAR..OLD UYGHUR PUNCTUATION FOUR DOTS
    ('\u{10f8a}', '\u{10faf}', BidiClass::RightToLeft), // Cn  [38] <reserved-10F8A>..<reserved-10FAF>
    ('\u{10fb0}', '\u{10fc4}', BidiClass::RightToLeft), // Lo  [21] CHORASMIAN LETTER ALEPH..CHORASMIAN LETTER TAW
    ('\u{10fc5}', '\u{10fcb}', BidiClass::RightToLeft), // No   [7] CHORASMIAN NUMBER ONE..CHORASMIAN NUMBER ONE HUNDRED
    ('\u{10fcc}', '\u{10fdf}', BidiClass::RightToLeft), // Cn  [20] <reserved-10FCC>..<reserved-10FDF>
    ('\u{10fe0}', '\u{10ff6}', BidiClass::RightToLeft), // Lo  [23] ELYMAIC LETTER ALEPH..ELYMAIC LIGATURE ZAYIN-YODH
    ('\u{10ff7}', '\u{10fff}', BidiClass::RightToLeft), // Cn   [9] <reserved-10FF7>..<reserved-10FFF>
    ('\u{11000}', '\u{11000}', BidiClass::LeftToRight), // Mc       BRAHMI SIGN CANDRABINDU
    ('\u{11001}', '\u{11001}', BidiClass::NonspacingMark), // Mn       BRAHMI SIGN ANUSVARA
    ('\u{11002}', '\u{11002}', BidiClass::LeftToRight), // Mc       BRAHMI SIGN VISARGA
    ('\u{11003}', '\u{11037}', BidiClass::LeftToRight), // Lo  [53] BRAHMI SIGN JIHVAMULIYA..BRAHMI LETTER OLD TAMIL NNNA
    ('\u{11038}', '\u{11046}', BidiClass::NonspacingMark), // Mn  [15] BRAHMI VOWEL SIGN AA..BRAHMI VIRAMA
    ('\u{11047}', '\u{1104d}', BidiClass::LeftToRight), // Po   [7] BRAHMI DANDA..BRAHMI PUNCTUATION LOTUS
    ('\u{11052}', '\u{11065}', BidiClass::OtherNeutral), // No  [20] BRAHMI NUMBER ONE..BRAHMI NUMBER ONE THOUSAND
    ('\u{11066}', '\u{1106f}', BidiClass::LeftToRight), // Nd  [10] BRAHMI DIGIT ZERO..BRAHMI DIGIT NINE
    ('\u{11070}', '\u{11070}', BidiClass::NonspacingMark), // Mn       BRAHMI SIGN OLD TAMIL VIRAMA
    ('\u{11071}', '\u{11072}', BidiClass::LeftToRight), // Lo   [2] BRAHMI LETTER OLD TAMIL SHORT E..BRAHMI LETTER OLD TAMIL SHORT O
    ('\u{11073}', '\u{11074}', BidiClass::NonspacingMark), // Mn   [2] BRAHMI VOWEL SIGN OLD TAMIL SHORT E..BRAHMI VOWEL SIGN OLD TAMIL SHORT O
    ('\u{11075}', '\u{11075}', BidiClass::LeftToRight),    // Lo       BRAHMI LETTER OLD TAMIL LLA
    ('\u{1107f}', '\u{11081}', BidiClass::NonspacingMark), // Mn   [3] BRAHMI NUMBER JOINER..KAITHI SIGN ANUSVARA
    ('\u{11082}', '\u{11082}', BidiClass::LeftToRight),    // Mc       KAITHI SIGN VISARGA
    ('\u{11083}', '\u{110af}', BidiClass::LeftToRight), // Lo  [45] KAITHI LETTER A..KAITHI LETTER HA
    ('\u{110b0}', '\u{110b2}', BidiClass::LeftToRight), // Mc   [3] KAITHI VOWEL SIGN AA..KAITHI VOWEL SIGN II
    ('\u{110b3}', '\u{110b6}', BidiClass::NonspacingMark), // Mn   [4] KAITHI VOWEL SIGN U..KAITHI VOWEL SIGN AI
    ('\u{110b7}', '\u{110b8}', BidiClass::LeftToRight), // Mc   [2] KAITHI VOWEL SIGN O..KAITHI VOWEL SIGN AU
    ('\u{110b9}', '\u{110ba}', BidiClass::NonspacingMark), // Mn   [2] KAITHI SIGN VIRAMA..KAITHI SIGN NUKTA
    ('\u{110bb}', '\u{110bc}', BidiClass::LeftToRight), // Po   [2] KAITHI ABBREVIATION SIGN..KAITHI ENUMERATION SIGN
    ('\u{110bd}', '\u{110bd}', BidiClass::LeftToRight), // Cf       KAITHI NUMBER SIGN
    ('\u{110be}', '\u{110c1}', BidiClass::LeftToRight), // Po   [4] KAITHI SECTION MARK..KAITHI DOUBLE DANDA
    ('\u{110c2}', '\u{110c2}', BidiClass::NonspacingMark), // Mn       KAITHI VOWEL SIGN VOCALIC R
    ('\u{110cd}', '\u{110cd}', BidiClass::LeftToRight), // Cf       KAITHI NUMBER SIGN ABOVE
    ('\u{110d0}', '\u{110e8}', BidiClass::LeftToRight), // Lo  [25] SORA SOMPENG LETTER SAH..SORA SOMPENG LETTER MAE
    ('\u{110f0}', '\u{110f9}', BidiClass::LeftToRight), // Nd  [10] SORA SOMPENG DIGIT ZERO..SORA SOMPENG DIGIT NINE
    ('\u{11100}', '\u{11102}', BidiClass::NonspacingMark), // Mn   [3] CHAKMA SIGN CANDRABINDU..CHAKMA SIGN VISARGA
    ('\u{11103}', '\u{11126}', BidiClass::LeftToRight), // Lo  [36] CHAKMA LETTER AA..CHAKMA LETTER HAA
    ('\u{11127}', '\u{1112b}', BidiClass::NonspacingMark), // Mn   [5] CHAKMA VOWEL SIGN A..CHAKMA VOWEL SIGN UU
    ('\u{1112c}', '\u{1112c}', BidiClass::LeftToRight),    // Mc       CHAKMA VOWEL SIGN E
    ('\u{1112d}', '\u{11134}', BidiClass::NonspacingMark), // Mn   [8] CHAKMA VOWEL SIGN AI..CHAKMA MAAYYAA
    ('\u{11136}', '\u{1113f}', BidiClass::LeftToRight), // Nd  [10] CHAKMA DIGIT ZERO..CHAKMA DIGIT NINE
    ('\u{11140}', '\u{11143}', BidiClass::LeftToRight), // Po   [4] CHAKMA SECTION MARK..CHAKMA QUESTION MARK
    ('\u{11144}', '\u{11144}', BidiClass::LeftToRight), // Lo       CHAKMA LETTER LHAA
    ('\u{11145}', '\u{11146}', BidiClass::LeftToRight), // Mc   [2] CHAKMA VOWEL SIGN AA..CHAKMA VOWEL SIGN EI
    ('\u{11147}', '\u{11147}', BidiClass::LeftToRight), // Lo       CHAKMA LETTER VAA
    ('\u{11150}', '\u{11172}', BidiClass::LeftToRight), // Lo  [35] MAHAJANI LETTER A..MAHAJANI LETTER RRA
    ('\u{11173}', '\u{11173}', BidiClass::NonspacingMark), // Mn       MAHAJANI SIGN NUKTA
    ('\u{11174}', '\u{11175}', BidiClass::LeftToRight), // Po   [2] MAHAJANI ABBREVIATION SIGN..MAHAJANI SECTION MARK
    ('\u{11176}', '\u{11176}', BidiClass::LeftToRight), // Lo       MAHAJANI LIGATURE SHRI
    ('\u{11180}', '\u{11181}', BidiClass::NonspacingMark), // Mn   [2] SHARADA SIGN CANDRABINDU..SHARADA SIGN ANUSVARA
    ('\u{11182}', '\u{11182}', BidiClass::LeftToRight),    // Mc       SHARADA SIGN VISARGA
    ('\u{11183}', '\u{111b2}', BidiClass::LeftToRight), // Lo  [48] SHARADA LETTER A..SHARADA LETTER HA
    ('\u{111b3}', '\u{111b5}', BidiClass::LeftToRight), // Mc   [3] SHARADA VOWEL SIGN AA..SHARADA VOWEL SIGN II
    ('\u{111b6}', '\u{111be}', BidiClass::NonspacingMark), // Mn   [9] SHARADA VOWEL SIGN U..SHARADA VOWEL SIGN O
    ('\u{111bf}', '\u{111c0}', BidiClass::LeftToRight), // Mc   [2] SHARADA VOWEL SIGN AU..SHARADA SIGN VIRAMA
    ('\u{111c1}', '\u{111c4}', BidiClass::LeftToRight), // Lo   [4] SHARADA SIGN AVAGRAHA..SHARADA OM
    ('\u{111c5}', '\u{111c8}', BidiClass::LeftToRight), // Po   [4] SHARADA DANDA..SHARADA SEPARATOR
    ('\u{111c9}', '\u{111cc}', BidiClass::NonspacingMark), // Mn   [4] SHARADA SANDHI MARK..SHARADA EXTRA SHORT VOWEL MARK
    ('\u{111cd}', '\u{111cd}', BidiClass::LeftToRight),    // Po       SHARADA SUTRA MARK
    ('\u{111ce}', '\u{111ce}', BidiClass::LeftToRight), // Mc       SHARADA VOWEL SIGN PRISHTHAMATRA E
    ('\u{111cf}', '\u{111cf}', BidiClass::NonspacingMark), // Mn       SHARADA SIGN INVERTED CANDRABINDU
    ('\u{111d0}', '\u{111d9}', BidiClass::LeftToRight), // Nd  [10] SHARADA DIGIT ZERO..SHARADA DIGIT NINE
    ('\u{111da}', '\u{111da}', BidiClass::LeftToRight), // Lo       SHARADA EKAM
    ('\u{111db}', '\u{111db}', BidiClass::LeftToRight), // Po       SHARADA SIGN SIDDHAM
    ('\u{111dc}', '\u{111dc}', BidiClass::LeftToRight), // Lo       SHARADA HEADSTROKE
    ('\u{111dd}', '\u{111df}', BidiClass::LeftToRight), // Po   [3] SHARADA CONTINUATION SIGN..SHARADA SECTION MARK-2
    ('\u{111e1}', '\u{111f4}', BidiClass::LeftToRight), // No  [20] SINHALA ARCHAIC DIGIT ONE..SINHALA ARCHAIC NUMBER ONE THOUSAND
    ('\u{11200}', '\u{11211}', BidiClass::LeftToRight), // Lo  [18] KHOJKI LETTER A..KHOJKI LETTER JJA
    ('\u{11213}', '\u{1122b}', BidiClass::LeftToRight), // Lo  [25] KHOJKI LETTER NYA..KHOJKI LETTER LLA
    ('\u{1122c}', '\u{1122e}', BidiClass::LeftToRight), // Mc   [3] KHOJKI VOWEL SIGN AA..KHOJKI VOWEL SIGN II
    ('\u{1122f}', '\u{11231}', BidiClass::NonspacingMark), // Mn   [3] KHOJKI VOWEL SIGN U..KHOJKI VOWEL SIGN AI
    ('\u{11232}', '\u{11233}', BidiClass::LeftToRight), // Mc   [2] KHOJKI VOWEL SIGN O..KHOJKI VOWEL SIGN AU
    ('\u{11234}', '\u{11234}', BidiClass::NonspacingMark), // Mn       KHOJKI SIGN ANUSVARA
    ('\u{11235}', '\u{11235}', BidiClass::LeftToRight), // Mc       KHOJKI SIGN VIRAMA
    ('\u{11236}', '\u{11237}', BidiClass::NonspacingMark), // Mn   [2] KHOJKI SIGN NUKTA..KHOJKI SIGN SHADDA
    ('\u{11238}', '\u{1123d}', BidiClass::LeftToRight), // Po   [6] KHOJKI DANDA..KHOJKI ABBREVIATION SIGN
    ('\u{1123e}', '\u{1123e}', BidiClass::NonspacingMark), // Mn       KHOJKI SIGN SUKUN
    ('\u{11280}', '\u{11286}', BidiClass::LeftToRight), // Lo   [7] MULTANI LETTER A..MULTANI LETTER GA
    ('\u{11288}', '\u{11288}', BidiClass::LeftToRight), // Lo       MULTANI LETTER GHA
    ('\u{1128a}', '\u{1128d}', BidiClass::LeftToRight), // Lo   [4] MULTANI LETTER CA..MULTANI LETTER JJA
    ('\u{1128f}', '\u{1129d}', BidiClass::LeftToRight), // Lo  [15] MULTANI LETTER NYA..MULTANI LETTER BA
    ('\u{1129f}', '\u{112a8}', BidiClass::LeftToRight), // Lo  [10] MULTANI LETTER BHA..MULTANI LETTER RHA
    ('\u{112a9}', '\u{112a9}', BidiClass::LeftToRight), // Po       MULTANI SECTION MARK
    ('\u{112b0}', '\u{112de}', BidiClass::LeftToRight), // Lo  [47] KHUDAWADI LETTER A..KHUDAWADI LETTER HA
    ('\u{112df}', '\u{112df}', BidiClass::NonspacingMark), // Mn       KHUDAWADI SIGN ANUSVARA
    ('\u{112e0}', '\u{112e2}', BidiClass::LeftToRight), // Mc   [3] KHUDAWADI VOWEL SIGN AA..KHUDAWADI VOWEL SIGN II
    ('\u{112e3}', '\u{112ea}', BidiClass::NonspacingMark), // Mn   [8] KHUDAWADI VOWEL SIGN U..KHUDAWADI SIGN VIRAMA
    ('\u{112f0}', '\u{112f9}', BidiClass::LeftToRight), // Nd  [10] KHUDAWADI DIGIT ZERO..KHUDAWADI DIGIT NINE
    ('\u{11300}', '\u{11301}', BidiClass::NonspacingMark), // Mn   [2] GRANTHA SIGN COMBINING ANUSVARA ABOVE..GRANTHA SIGN CANDRABINDU
    ('\u{11302}', '\u{11303}', BidiClass::LeftToRight), // Mc   [2] GRANTHA SIGN ANUSVARA..GRANTHA SIGN VISARGA
    ('\u{11305}', '\u{1130c}', BidiClass::LeftToRight), // Lo   [8] GRANTHA LETTER A..GRANTHA LETTER VOCALIC L
    ('\u{1130f}', '\u{11310}', BidiClass::LeftToRight), // Lo   [2] GRANTHA LETTER EE..GRANTHA LETTER AI
    ('\u{11313}', '\u{11328}', BidiClass::LeftToRight), // Lo  [22] GRANTHA LETTER OO..GRANTHA LETTER NA
    ('\u{1132a}', '\u{11330}', BidiClass::LeftToRight), // Lo   [7] GRANTHA LETTER PA..GRANTHA LETTER RA
    ('\u{11332}', '\u{11333}', BidiClass::LeftToRight), // Lo   [2] GRANTHA LETTER LA..GRANTHA LETTER LLA
    ('\u{11335}', '\u{11339}', BidiClass::LeftToRight), // Lo   [5] GRANTHA LETTER VA..GRANTHA LETTER HA
    ('\u{1133b}', '\u{1133c}', BidiClass::NonspacingMark), // Mn   [2] COMBINING BINDU BELOW..GRANTHA SIGN NUKTA
    ('\u{1133d}', '\u{1133d}', BidiClass::LeftToRight),    // Lo       GRANTHA SIGN AVAGRAHA
    ('\u{1133e}', '\u{1133f}', BidiClass::LeftToRight), // Mc   [2] GRANTHA VOWEL SIGN AA..GRANTHA VOWEL SIGN I
    ('\u{11340}', '\u{11340}', BidiClass::NonspacingMark), // Mn       GRANTHA VOWEL SIGN II
    ('\u{11341}', '\u{11344}', BidiClass::LeftToRight), // Mc   [4] GRANTHA VOWEL SIGN U..GRANTHA VOWEL SIGN VOCALIC RR
    ('\u{11347}', '\u{11348}', BidiClass::LeftToRight), // Mc   [2] GRANTHA VOWEL SIGN EE..GRANTHA VOWEL SIGN AI
    ('\u{1134b}', '\u{1134d}', BidiClass::LeftToRight), // Mc   [3] GRANTHA VOWEL SIGN OO..GRANTHA SIGN VIRAMA
    ('\u{11350}', '\u{11350}', BidiClass::LeftToRight), // Lo       GRANTHA OM
    ('\u{11357}', '\u{11357}', BidiClass::LeftToRight), // Mc       GRANTHA AU LENGTH MARK
    ('\u{1135d}', '\u{11361}', BidiClass::LeftToRight), // Lo   [5] GRANTHA SIGN PLUTA..GRANTHA LETTER VOCALIC LL
    ('\u{11362}', '\u{11363}', BidiClass::LeftToRight), // Mc   [2] GRANTHA VOWEL SIGN VOCALIC L..GRANTHA VOWEL SIGN VOCALIC LL
    ('\u{11366}', '\u{1136c}', BidiClass::NonspacingMark), // Mn   [7] COMBINING GRANTHA DIGIT ZERO..COMBINING GRANTHA DIGIT SIX
    ('\u{11370}', '\u{11374}', BidiClass::NonspacingMark), // Mn   [5] COMBINING GRANTHA LETTER A..COMBINING GRANTHA LETTER PA
    ('\u{11400}', '\u{11434}', BidiClass::LeftToRight),    // Lo  [53] NEWA LETTER A..NEWA LETTER HA
    ('\u{11435}', '\u{11437}', BidiClass::LeftToRight), // Mc   [3] NEWA VOWEL SIGN AA..NEWA VOWEL SIGN II
    ('\u{11438}', '\u{1143f}', BidiClass::NonspacingMark), // Mn   [8] NEWA VOWEL SIGN U..NEWA VOWEL SIGN AI
    ('\u{11440}', '\u{11441}', BidiClass::LeftToRight), // Mc   [2] NEWA VOWEL SIGN O..NEWA VOWEL SIGN AU
    ('\u{11442}', '\u{11444}', BidiClass::NonspacingMark), // Mn   [3] NEWA SIGN VIRAMA..NEWA SIGN ANUSVARA
    ('\u{11445}', '\u{11445}', BidiClass::LeftToRight),    // Mc       NEWA SIGN VISARGA
    ('\u{11446}', '\u{11446}', BidiClass::NonspacingMark), // Mn       NEWA SIGN NUKTA
    ('\u{11447}', '\u{1144a}', BidiClass::LeftToRight), // Lo   [4] NEWA SIGN AVAGRAHA..NEWA SIDDHI
    ('\u{1144b}', '\u{1144f}', BidiClass::LeftToRight), // Po   [5] NEWA DANDA..NEWA ABBREVIATION SIGN
    ('\u{11450}', '\u{11459}', BidiClass::LeftToRight), // Nd  [10] NEWA DIGIT ZERO..NEWA DIGIT NINE
    ('\u{1145a}', '\u{1145b}', BidiClass::LeftToRight), // Po   [2] NEWA DOUBLE COMMA..NEWA PLACEHOLDER MARK
    ('\u{1145d}', '\u{1145d}', BidiClass::LeftToRight), // Po       NEWA INSERTION SIGN
    ('\u{1145e}', '\u{1145e}', BidiClass::NonspacingMark), // Mn       NEWA SANDHI MARK
    ('\u{1145f}', '\u{11461}', BidiClass::LeftToRight), // Lo   [3] NEWA LETTER VEDIC ANUSVARA..NEWA SIGN UPADHMANIYA
    ('\u{11480}', '\u{114af}', BidiClass::LeftToRight), // Lo  [48] TIRHUTA ANJI..TIRHUTA LETTER HA
    ('\u{114b0}', '\u{114b2}', BidiClass::LeftToRight), // Mc   [3] TIRHUTA VOWEL SIGN AA..TIRHUTA VOWEL SIGN II
    ('\u{114b3}', '\u{114b8}', BidiClass::NonspacingMark), // Mn   [6] TIRHUTA VOWEL SIGN U..TIRHUTA VOWEL SIGN VOCALIC LL
    ('\u{114b9}', '\u{114b9}', BidiClass::LeftToRight),    // Mc       TIRHUTA VOWEL SIGN E
    ('\u{114ba}', '\u{114ba}', BidiClass::NonspacingMark), // Mn       TIRHUTA VOWEL SIGN SHORT E
    ('\u{114bb}', '\u{114be}', BidiClass::LeftToRight), // Mc   [4] TIRHUTA VOWEL SIGN AI..TIRHUTA VOWEL SIGN AU
    ('\u{114bf}', '\u{114c0}', BidiClass::NonspacingMark), // Mn   [2] TIRHUTA SIGN CANDRABINDU..TIRHUTA SIGN ANUSVARA
    ('\u{114c1}', '\u{114c1}', BidiClass::LeftToRight),    // Mc       TIRHUTA SIGN VISARGA
    ('\u{114c2}', '\u{114c3}', BidiClass::NonspacingMark), // Mn   [2] TIRHUTA SIGN VIRAMA..TIRHUTA SIGN NUKTA
    ('\u{114c4}', '\u{114c5}', BidiClass::LeftToRight), // Lo   [2] TIRHUTA SIGN AVAGRAHA..TIRHUTA GVANG
    ('\u{114c6}', '\u{114c6}', BidiClass::LeftToRight), // Po       TIRHUTA ABBREVIATION SIGN
    ('\u{114c7}', '\u{114c7}', BidiClass::LeftToRight), // Lo       TIRHUTA OM
    ('\u{114d0}', '\u{114d9}', BidiClass::LeftToRight), // Nd  [10] TIRHUTA DIGIT ZERO..TIRHUTA DIGIT NINE
    ('\u{11580}', '\u{115ae}', BidiClass::LeftToRight), // Lo  [47] SIDDHAM LETTER A..SIDDHAM LETTER HA
    ('\u{115af}', '\u{115b1}', BidiClass::LeftToRight), // Mc   [3] SIDDHAM VOWEL SIGN AA..SIDDHAM VOWEL SIGN II
    ('\u{115b2}', '\u{115b5}', BidiClass::NonspacingMark), // Mn   [4] SIDDHAM VOWEL SIGN U..SIDDHAM VOWEL SIGN VOCALIC RR
    ('\u{115b8}', '\u{115bb}', BidiClass::LeftToRight), // Mc   [4] SIDDHAM VOWEL SIGN E..SIDDHAM VOWEL SIGN AU
    ('\u{115bc}', '\u{115bd}', BidiClass::NonspacingMark), // Mn   [2] SIDDHAM SIGN CANDRABINDU..SIDDHAM SIGN ANUSVARA
    ('\u{115be}', '\u{115be}', BidiClass::LeftToRight),    // Mc       SIDDHAM SIGN VISARGA
    ('\u{115bf}', '\u{115c0}', BidiClass::NonspacingMark), // Mn   [2] SIDDHAM SIGN VIRAMA..SIDDHAM SIGN NUKTA
    ('\u{115c1}', '\u{115d7}', BidiClass::LeftToRight), // Po  [23] SIDDHAM SIGN SIDDHAM..SIDDHAM SECTION MARK WITH CIRCLES AND FOUR ENCLOSURES
    ('\u{115d8}', '\u{115db}', BidiClass::LeftToRight), // Lo   [4] SIDDHAM LETTER THREE-CIRCLE ALTERNATE I..SIDDHAM LETTER ALTERNATE U
    ('\u{115dc}', '\u{115dd}', BidiClass::NonspacingMark), // Mn   [2] SIDDHAM VOWEL SIGN ALTERNATE U..SIDDHAM VOWEL SIGN ALTERNATE UU
    ('\u{11600}', '\u{1162f}', BidiClass::LeftToRight), // Lo  [48] MODI LETTER A..MODI LETTER LLA
    ('\u{11630}', '\u{11632}', BidiClass::LeftToRight), // Mc   [3] MODI VOWEL SIGN AA..MODI VOWEL SIGN II
    ('\u{11633}', '\u{1163a}', BidiClass::NonspacingMark), // Mn   [8] MODI VOWEL SIGN U..MODI VOWEL SIGN AI
    ('\u{1163b}', '\u{1163c}', BidiClass::LeftToRight), // Mc   [2] MODI VOWEL SIGN O..MODI VOWEL SIGN AU
    ('\u{1163d}', '\u{1163d}', BidiClass::NonspacingMark), // Mn       MODI SIGN ANUSVARA
    ('\u{1163e}', '\u{1163e}', BidiClass::LeftToRight), // Mc       MODI SIGN VISARGA
    ('\u{1163f}', '\u{11640}', BidiClass::NonspacingMark), // Mn   [2] MODI SIGN VIRAMA..MODI SIGN ARDHACANDRA
    ('\u{11641}', '\u{11643}', BidiClass::LeftToRight), // Po   [3] MODI DANDA..MODI ABBREVIATION SIGN
    ('\u{11644}', '\u{11644}', BidiClass::LeftToRight), // Lo       MODI SIGN HUVA
    ('\u{11650}', '\u{11659}', BidiClass::LeftToRight), // Nd  [10] MODI DIGIT ZERO..MODI DIGIT NINE
    ('\u{11660}', '\u{1166c}', BidiClass::OtherNeutral), // Po  [13] MONGOLIAN BIRGA WITH ORNAMENT..MONGOLIAN TURNED SWIRL BIRGA WITH DOUBLE ORNAMENT
    ('\u{11680}', '\u{116aa}', BidiClass::LeftToRight), // Lo  [43] TAKRI LETTER A..TAKRI LETTER RRA
    ('\u{116ab}', '\u{116ab}', BidiClass::NonspacingMark), // Mn       TAKRI SIGN ANUSVARA
    ('\u{116ac}', '\u{116ac}', BidiClass::LeftToRight), // Mc       TAKRI SIGN VISARGA
    ('\u{116ad}', '\u{116ad}', BidiClass::NonspacingMark), // Mn       TAKRI VOWEL SIGN AA
    ('\u{116ae}', '\u{116af}', BidiClass::LeftToRight), // Mc   [2] TAKRI VOWEL SIGN I..TAKRI VOWEL SIGN II
    ('\u{116b0}', '\u{116b5}', BidiClass::NonspacingMark), // Mn   [6] TAKRI VOWEL SIGN U..TAKRI VOWEL SIGN AU
    ('\u{116b6}', '\u{116b6}', BidiClass::LeftToRight),    // Mc       TAKRI SIGN VIRAMA
    ('\u{116b7}', '\u{116b7}', BidiClass::NonspacingMark), // Mn       TAKRI SIGN NUKTA
    ('\u{116b8}', '\u{116b8}', BidiClass::LeftToRight),    // Lo       TAKRI LETTER ARCHAIC KHA
    ('\u{116b9}', '\u{116b9}', BidiClass::LeftToRight),    // Po       TAKRI ABBREVIATION SIGN
    ('\u{116c0}', '\u{116c9}', BidiClass::LeftToRight), // Nd  [10] TAKRI DIGIT ZERO..TAKRI DIGIT NINE
    ('\u{11700}', '\u{1171a}', BidiClass::LeftToRight), // Lo  [27] AHOM LETTER KA..AHOM LETTER ALTERNATE BA
    ('\u{1171d}', '\u{1171f}', BidiClass::NonspacingMark), // Mn   [3] AHOM CONSONANT SIGN MEDIAL LA..AHOM CONSONANT SIGN MEDIAL LIGATING RA
    ('\u{11720}', '\u{11721}', BidiClass::LeftToRight), // Mc   [2] AHOM VOWEL SIGN A..AHOM VOWEL SIGN AA
    ('\u{11722}', '\u{11725}', BidiClass::NonspacingMark), // Mn   [4] AHOM VOWEL SIGN I..AHOM VOWEL SIGN UU
    ('\u{11726}', '\u{11726}', BidiClass::LeftToRight),    // Mc       AHOM VOWEL SIGN E
    ('\u{11727}', '\u{1172b}', BidiClass::NonspacingMark), // Mn   [5] AHOM VOWEL SIGN AW..AHOM SIGN KILLER
    ('\u{11730}', '\u{11739}', BidiClass::LeftToRight), // Nd  [10] AHOM DIGIT ZERO..AHOM DIGIT NINE
    ('\u{1173a}', '\u{1173b}', BidiClass::LeftToRight), // No   [2] AHOM NUMBER TEN..AHOM NUMBER TWENTY
    ('\u{1173c}', '\u{1173e}', BidiClass::LeftToRight), // Po   [3] AHOM SIGN SMALL SECTION..AHOM SIGN RULAI
    ('\u{1173f}', '\u{1173f}', BidiClass::LeftToRight), // So       AHOM SYMBOL VI
    ('\u{11740}', '\u{11746}', BidiClass::LeftToRight), // Lo   [7] AHOM LETTER CA..AHOM LETTER LLA
    ('\u{11800}', '\u{1182b}', BidiClass::LeftToRight), // Lo  [44] DOGRA LETTER A..DOGRA LETTER RRA
    ('\u{1182c}', '\u{1182e}', BidiClass::LeftToRight), // Mc   [3] DOGRA VOWEL SIGN AA..DOGRA VOWEL SIGN II
    ('\u{1182f}', '\u{11837}', BidiClass::NonspacingMark), // Mn   [9] DOGRA VOWEL SIGN U..DOGRA SIGN ANUSVARA
    ('\u{11838}', '\u{11838}', BidiClass::LeftToRight),    // Mc       DOGRA SIGN VISARGA
    ('\u{11839}', '\u{1183a}', BidiClass::NonspacingMark), // Mn   [2] DOGRA SIGN VIRAMA..DOGRA SIGN NUKTA
    ('\u{1183b}', '\u{1183b}', BidiClass::LeftToRight),    // Po       DOGRA ABBREVIATION SIGN
    ('\u{118a0}', '\u{118df}', BidiClass::LeftToRight), // L&  [64] WARANG CITI CAPITAL LETTER NGAA..WARANG CITI SMALL LETTER VIYO
    ('\u{118e0}', '\u{118e9}', BidiClass::LeftToRight), // Nd  [10] WARANG CITI DIGIT ZERO..WARANG CITI DIGIT NINE
    ('\u{118ea}', '\u{118f2}', BidiClass::LeftToRight), // No   [9] WARANG CITI NUMBER TEN..WARANG CITI NUMBER NINETY
    ('\u{118ff}', '\u{11906}', BidiClass::LeftToRight), // Lo   [8] WARANG CITI OM..DIVES AKURU LETTER E
    ('\u{11909}', '\u{11909}', BidiClass::LeftToRight), // Lo       DIVES AKURU LETTER O
    ('\u{1190c}', '\u{11913}', BidiClass::LeftToRight), // Lo   [8] DIVES AKURU LETTER KA..DIVES AKURU LETTER JA
    ('\u{11915}', '\u{11916}', BidiClass::LeftToRight), // Lo   [2] DIVES AKURU LETTER NYA..DIVES AKURU LETTER TTA
    ('\u{11918}', '\u{1192f}', BidiClass::LeftToRight), // Lo  [24] DIVES AKURU LETTER DDA..DIVES AKURU LETTER ZA
    ('\u{11930}', '\u{11935}', BidiClass::LeftToRight), // Mc   [6] DIVES AKURU VOWEL SIGN AA..DIVES AKURU VOWEL SIGN E
    ('\u{11937}', '\u{11938}', BidiClass::LeftToRight), // Mc   [2] DIVES AKURU VOWEL SIGN AI..DIVES AKURU VOWEL SIGN O
    ('\u{1193b}', '\u{1193c}', BidiClass::NonspacingMark), // Mn   [2] DIVES AKURU SIGN ANUSVARA..DIVES AKURU SIGN CANDRABINDU
    ('\u{1193d}', '\u{1193d}', BidiClass::LeftToRight),    // Mc       DIVES AKURU SIGN HALANTA
    ('\u{1193e}', '\u{1193e}', BidiClass::NonspacingMark), // Mn       DIVES AKURU VIRAMA
    ('\u{1193f}', '\u{1193f}', BidiClass::LeftToRight), // Lo       DIVES AKURU PREFIXED NASAL SIGN
    ('\u{11940}', '\u{11940}', BidiClass::LeftToRight), // Mc       DIVES AKURU MEDIAL YA
    ('\u{11941}', '\u{11941}', BidiClass::LeftToRight), // Lo       DIVES AKURU INITIAL RA
    ('\u{11942}', '\u{11942}', BidiClass::LeftToRight), // Mc       DIVES AKURU MEDIAL RA
    ('\u{11943}', '\u{11943}', BidiClass::NonspacingMark), // Mn       DIVES AKURU SIGN NUKTA
    ('\u{11944}', '\u{11946}', BidiClass::LeftToRight), // Po   [3] DIVES AKURU DOUBLE DANDA..DIVES AKURU END OF TEXT MARK
    ('\u{11950}', '\u{11959}', BidiClass::LeftToRight), // Nd  [10] DIVES AKURU DIGIT ZERO..DIVES AKURU DIGIT NINE
    ('\u{119a0}', '\u{119a7}', BidiClass::LeftToRight), // Lo   [8] NANDINAGARI LETTER A..NANDINAGARI LETTER VOCALIC RR
    ('\u{119aa}', '\u{119d0}', BidiClass::LeftToRight), // Lo  [39] NANDINAGARI LETTER E..NANDINAGARI LETTER RRA
    ('\u{119d1}', '\u{119d3}', BidiClass::LeftToRight), // Mc   [3] NANDINAGARI VOWEL SIGN AA..NANDINAGARI VOWEL SIGN II
    ('\u{119d4}', '\u{119d7}', BidiClass::NonspacingMark), // Mn   [4] NANDINAGARI VOWEL SIGN U..NANDINAGARI VOWEL SIGN VOCALIC RR
    ('\u{119da}', '\u{119db}', BidiClass::NonspacingMark), // Mn   [2] NANDINAGARI VOWEL SIGN E..NANDINAGARI VOWEL SIGN AI
    ('\u{119dc}', '\u{119df}', BidiClass::LeftToRight), // Mc   [4] NANDINAGARI VOWEL SIGN O..NANDINAGARI SIGN VISARGA
    ('\u{119e0}', '\u{119e0}', BidiClass::NonspacingMark), // Mn       NANDINAGARI SIGN VIRAMA
    ('\u{119e1}', '\u{119e1}', BidiClass::LeftToRight), // Lo       NANDINAGARI SIGN AVAGRAHA
    ('\u{119e2}', '\u{119e2}', BidiClass::LeftToRight), // Po       NANDINAGARI SIGN SIDDHAM
    ('\u{119e3}', '\u{119e3}', BidiClass::LeftToRight), // Lo       NANDINAGARI HEADSTROKE
    ('\u{119e4}', '\u{119e4}', BidiClass::LeftToRight), // Mc       NANDINAGARI VOWEL SIGN PRISHTHAMATRA E
    ('\u{11a00}', '\u{11a00}', BidiClass::LeftToRight), // Lo       ZANABAZAR SQUARE LETTER A
    ('\u{11a01}', '\u{11a06}', BidiClass::NonspacingMark), // Mn   [6] ZANABAZAR SQUARE VOWEL SIGN I..ZANABAZAR SQUARE VOWEL SIGN O
    ('\u{11a07}', '\u{11a08}', BidiClass::LeftToRight), // Mn   [2] ZANABAZAR SQUARE VOWEL SIGN AI..ZANABAZAR SQUARE VOWEL SIGN AU
    ('\u{11a09}', '\u{11a0a}', BidiClass::NonspacingMark), // Mn   [2] ZANABAZAR SQUARE VOWEL SIGN REVERSED I..ZANABAZAR SQUARE VOWEL LENGTH MARK
    ('\u{11a0b}', '\u{11a32}', BidiClass::LeftToRight), // Lo  [40] ZANABAZAR SQUARE LETTER KA..ZANABAZAR SQUARE LETTER KSSA
    ('\u{11a33}', '\u{11a38}', BidiClass::NonspacingMark), // Mn   [6] ZANABAZAR SQUARE FINAL CONSONANT MARK..ZANABAZAR SQUARE SIGN ANUSVARA
    ('\u{11a39}', '\u{11a39}', BidiClass::LeftToRight),    // Mc       ZANABAZAR SQUARE SIGN VISARGA
    ('\u{11a3a}', '\u{11a3a}', BidiClass::LeftToRight), // Lo       ZANABAZAR SQUARE CLUSTER-INITIAL LETTER RA
    ('\u{11a3b}', '\u{11a3e}', BidiClass::NonspacingMark), // Mn   [4] ZANABAZAR SQUARE CLUSTER-FINAL LETTER YA..ZANABAZAR SQUARE CLUSTER-FINAL LETTER VA
    ('\u{11a3f}', '\u{11a46}', BidiClass::LeftToRight), // Po   [8] ZANABAZAR SQUARE INITIAL HEAD MARK..ZANABAZAR SQUARE CLOSING DOUBLE-LINED HEAD MARK
    ('\u{11a47}', '\u{11a47}', BidiClass::NonspacingMark), // Mn       ZANABAZAR SQUARE SUBJOINER
    ('\u{11a50}', '\u{11a50}', BidiClass::LeftToRight), // Lo       SOYOMBO LETTER A
    ('\u{11a51}', '\u{11a56}', BidiClass::NonspacingMark), // Mn   [6] SOYOMBO VOWEL SIGN I..SOYOMBO VOWEL SIGN OE
    ('\u{11a57}', '\u{11a58}', BidiClass::LeftToRight), // Mc   [2] SOYOMBO VOWEL SIGN AI..SOYOMBO VOWEL SIGN AU
    ('\u{11a59}', '\u{11a5b}', BidiClass::NonspacingMark), // Mn   [3] SOYOMBO VOWEL SIGN VOCALIC R..SOYOMBO VOWEL LENGTH MARK
    ('\u{11a5c}', '\u{11a89}', BidiClass::LeftToRight), // Lo  [46] SOYOMBO LETTER KA..SOYOMBO CLUSTER-INITIAL LETTER SA
    ('\u{11a8a}', '\u{11a96}', BidiClass::NonspacingMark), // Mn  [13] SOYOMBO FINAL CONSONANT SIGN G..SOYOMBO SIGN ANUSVARA
    ('\u{11a97}', '\u{11a97}', BidiClass::LeftToRight),    // Mc       SOYOMBO SIGN VISARGA
    ('\u{11a98}', '\u{11a99}', BidiClass::NonspacingMark), // Mn   [2] SOYOMBO GEMINATION MARK..SOYOMBO SUBJOINER
    ('\u{11a9a}', '\u{11a9c}', BidiClass::LeftToRight), // Po   [3] SOYOMBO MARK TSHEG..SOYOMBO MARK DOUBLE SHAD
    ('\u{11a9d}', '\u{11a9d}', BidiClass::LeftToRight), // Lo       SOYOMBO MARK PLUTA
    ('\u{11a9e}', '\u{11aa2}', BidiClass::LeftToRight), // Po   [5] SOYOMBO HEAD MARK WITH MOON AND SUN AND TRIPLE FLAME..SOYOMBO TERMINAL MARK-2
    ('\u{11ab0}', '\u{11af8}', BidiClass::LeftToRight), // Lo  [73] CANADIAN SYLLABICS NATTILIK HI..PAU CIN HAU GLOTTAL STOP FINAL
    ('\u{11c00}', '\u{11c08}', BidiClass::LeftToRight), // Lo   [9] BHAIKSUKI LETTER A..BHAIKSUKI LETTER VOCALIC L
    ('\u{11c0a}', '\u{11c2e}', BidiClass::LeftToRight), // Lo  [37] BHAIKSUKI LETTER E..BHAIKSUKI LETTER HA
    ('\u{11c2f}', '\u{11c2f}', BidiClass::LeftToRight), // Mc       BHAIKSUKI VOWEL SIGN AA
    ('\u{11c30}', '\u{11c36}', BidiClass::NonspacingMark), // Mn   [7] BHAIKSUKI VOWEL SIGN I..BHAIKSUKI VOWEL SIGN VOCALIC L
    ('\u{11c38}', '\u{11c3d}', BidiClass::NonspacingMark), // Mn   [6] BHAIKSUKI VOWEL SIGN E..BHAIKSUKI SIGN ANUSVARA
    ('\u{11c3e}', '\u{11c3e}', BidiClass::LeftToRight),    // Mc       BHAIKSUKI SIGN VISARGA
    ('\u{11c3f}', '\u{11c3f}', BidiClass::LeftToRight),    // Mn       BHAIKSUKI SIGN VIRAMA
    ('\u{11c40}', '\u{11c40}', BidiClass::LeftToRight),    // Lo       BHAIKSUKI SIGN AVAGRAHA
    ('\u{11c41}', '\u{11c45}', BidiClass::LeftToRight), // Po   [5] BHAIKSUKI DANDA..BHAIKSUKI GAP FILLER-2
    ('\u{11c50}', '\u{11c59}', BidiClass::LeftToRight), // Nd  [10] BHAIKSUKI DIGIT ZERO..BHAIKSUKI DIGIT NINE
    ('\u{11c5a}', '\u{11c6c}', BidiClass::LeftToRight), // No  [19] BHAIKSUKI NUMBER ONE..BHAIKSUKI HUNDREDS UNIT MARK
    ('\u{11c70}', '\u{11c71}', BidiClass::LeftToRight), // Po   [2] MARCHEN HEAD MARK..MARCHEN MARK SHAD
    ('\u{11c72}', '\u{11c8f}', BidiClass::LeftToRight), // Lo  [30] MARCHEN LETTER KA..MARCHEN LETTER A
    ('\u{11c92}', '\u{11ca7}', BidiClass::NonspacingMark), // Mn  [22] MARCHEN SUBJOINED LETTER KA..MARCHEN SUBJOINED LETTER ZA
    ('\u{11ca9}', '\u{11ca9}', BidiClass::LeftToRight),    // Mc       MARCHEN SUBJOINED LETTER YA
    ('\u{11caa}', '\u{11cb0}', BidiClass::NonspacingMark), // Mn   [7] MARCHEN SUBJOINED LETTER RA..MARCHEN VOWEL SIGN AA
    ('\u{11cb1}', '\u{11cb1}', BidiClass::LeftToRight),    // Mc       MARCHEN VOWEL SIGN I
    ('\u{11cb2}', '\u{11cb3}', BidiClass::NonspacingMark), // Mn   [2] MARCHEN VOWEL SIGN U..MARCHEN VOWEL SIGN E
    ('\u{11cb4}', '\u{11cb4}', BidiClass::LeftToRight),    // Mc       MARCHEN VOWEL SIGN O
    ('\u{11cb5}', '\u{11cb6}', BidiClass::NonspacingMark), // Mn   [2] MARCHEN SIGN ANUSVARA..MARCHEN SIGN CANDRABINDU
    ('\u{11d00}', '\u{11d06}', BidiClass::LeftToRight), // Lo   [7] MASARAM GONDI LETTER A..MASARAM GONDI LETTER E
    ('\u{11d08}', '\u{11d09}', BidiClass::LeftToRight), // Lo   [2] MASARAM GONDI LETTER AI..MASARAM GONDI LETTER O
    ('\u{11d0b}', '\u{11d30}', BidiClass::LeftToRight), // Lo  [38] MASARAM GONDI LETTER AU..MASARAM GONDI LETTER TRA
    ('\u{11d31}', '\u{11d36}', BidiClass::NonspacingMark), // Mn   [6] MASARAM GONDI VOWEL SIGN AA..MASARAM GONDI VOWEL SIGN VOCALIC R
    ('\u{11d3a}', '\u{11d3a}', BidiClass::NonspacingMark), // Mn       MASARAM GONDI VOWEL SIGN E
    ('\u{11d3c}', '\u{11d3d}', BidiClass::NonspacingMark), // Mn   [2] MASARAM GONDI VOWEL SIGN AI..MASARAM GONDI VOWEL SIGN O
    ('\u{11d3f}', '\u{11d45}', BidiClass::NonspacingMark), // Mn   [7] MASARAM GONDI VOWEL SIGN AU..MASARAM GONDI VIRAMA
    ('\u{11d46}', '\u{11d46}', BidiClass::LeftToRight),    // Lo       MASARAM GONDI REPHA
    ('\u{11d47}', '\u{11d47}', BidiClass::NonspacingMark), // Mn       MASARAM GONDI RA-KARA
    ('\u{11d50}', '\u{11d59}', BidiClass::LeftToRight), // Nd  [10] MASARAM GONDI DIGIT ZERO..MASARAM GONDI DIGIT NINE
    ('\u{11d60}', '\u{11d65}', BidiClass::LeftToRight), // Lo   [6] GUNJALA GONDI LETTER A..GUNJALA GONDI LETTER UU
    ('\u{11d67}', '\u{11d68}', BidiClass::LeftToRight), // Lo   [2] GUNJALA GONDI LETTER EE..GUNJALA GONDI LETTER AI
    ('\u{11d6a}', '\u{11d89}', BidiClass::LeftToRight), // Lo  [32] GUNJALA GONDI LETTER OO..GUNJALA GONDI LETTER SA
    ('\u{11d8a}', '\u{11d8e}', BidiClass::LeftToRight), // Mc   [5] GUNJALA GONDI VOWEL SIGN AA..GUNJALA GONDI VOWEL SIGN UU
    ('\u{11d90}', '\u{11d91}', BidiClass::NonspacingMark), // Mn   [2] GUNJALA GONDI VOWEL SIGN EE..GUNJALA GONDI VOWEL SIGN AI
    ('\u{11d93}', '\u{11d94}', BidiClass::LeftToRight), // Mc   [2] GUNJALA GONDI VOWEL SIGN OO..GUNJALA GONDI VOWEL SIGN AU
    ('\u{11d95}', '\u{11d95}', BidiClass::NonspacingMark), // Mn       GUNJALA GONDI SIGN ANUSVARA
    ('\u{11d96}', '\u{11d96}', BidiClass::LeftToRight), // Mc       GUNJALA GONDI SIGN VISARGA
    ('\u{11d97}', '\u{11d97}', BidiClass::NonspacingMark), // Mn       GUNJALA GONDI VIRAMA
    ('\u{11d98}', '\u{11d98}', BidiClass::LeftToRight), // Lo       GUNJALA GONDI OM
    ('\u{11da0}', '\u{11da9}', BidiClass::LeftToRight), // Nd  [10] GUNJALA GONDI DIGIT ZERO..GUNJALA GONDI DIGIT NINE
    ('\u{11ee0}', '\u{11ef2}', BidiClass::LeftToRight), // Lo  [19] MAKASAR LETTER KA..MAKASAR ANGKA
    ('\u{11ef3}', '\u{11ef4}', BidiClass::NonspacingMark), // Mn   [2] MAKASAR VOWEL SIGN I..MAKASAR VOWEL SIGN U
    ('\u{11ef5}', '\u{11ef6}', BidiClass::LeftToRight), // Mc   [2] MAKASAR VOWEL SIGN E..MAKASAR VOWEL SIGN O
    ('\u{11ef7}', '\u{11ef8}', BidiClass::LeftToRight), // Po   [2] MAKASAR PASSIMBANG..MAKASAR END OF SECTION
    ('\u{11fb0}', '\u{11fb0}', BidiClass::LeftToRight), // Lo       LISU LETTER YHA
    ('\u{11fc0}', '\u{11fd4}', BidiClass::LeftToRight), // No  [21] TAMIL FRACTION ONE THREE-HUNDRED-AND-TWENTIETH..TAMIL FRACTION DOWNSCALING FACTOR KIIZH
    ('\u{11fd5}', '\u{11fdc}', BidiClass::OtherNeutral), // So   [8] TAMIL SIGN NEL..TAMIL SIGN MUKKURUNI
    ('\u{11fdd}', '\u{11fe0}', BidiClass::EuropeanTerminator), // Sc   [4] TAMIL SIGN KAACU..TAMIL SIGN VARAAKAN
    ('\u{11fe1}', '\u{11ff1}', BidiClass::OtherNeutral), // So  [17] TAMIL SIGN PAARAM..TAMIL SIGN VAKAIYARAA
    ('\u{11fff}', '\u{11fff}', BidiClass::LeftToRight),  // Po       TAMIL PUNCTUATION END OF TEXT
    ('\u{12000}', '\u{12399}', BidiClass::LeftToRight), // Lo [922] CUNEIFORM SIGN A..CUNEIFORM SIGN U U
    ('\u{12400}', '\u{1246e}', BidiClass::LeftToRight), // Nl [111] CUNEIFORM NUMERIC SIGN TWO ASH..CUNEIFORM NUMERIC SIGN NINE U VARIANT FORM
    ('\u{12470}', '\u{12474}', BidiClass::LeftToRight), // Po   [5] CUNEIFORM PUNCTUATION SIGN OLD ASSYRIAN WORD DIVIDER..CUNEIFORM PUNCTUATION SIGN DIAGONAL QUADCOLON
    ('\u{12480}', '\u{12543}', BidiClass::LeftToRight), // Lo [196] CUNEIFORM SIGN AB TIMES NUN TENU..CUNEIFORM SIGN ZU5 TIMES THREE DISH TENU
    ('\u{12f90}', '\u{12ff0}', BidiClass::LeftToRight), // Lo  [97] CYPRO-MINOAN SIGN CM001..CYPRO-MINOAN SIGN CM114
    ('\u{12ff1}', '\u{12ff2}', BidiClass::LeftToRight), // Po   [2] CYPRO-MINOAN SIGN CM301..CYPRO-MINOAN SIGN CM302
    ('\u{13000}', '\u{1342e}', BidiClass::LeftToRight), // Lo [1071] EGYPTIAN HIEROGLYPH A001..EGYPTIAN HIEROGLYPH AA032
    ('\u{13430}', '\u{13438}', BidiClass::LeftToRight), // Cf   [9] EGYPTIAN HIEROGLYPH VERTICAL JOINER..EGYPTIAN HIEROGLYPH END SEGMENT
    ('\u{14400}', '\u{14646}', BidiClass::LeftToRight), // Lo [583] ANATOLIAN HIEROGLYPH A001..ANATOLIAN HIEROGLYPH A530
    ('\u{16800}', '\u{16a38}', BidiClass::LeftToRight), // Lo [569] BAMUM LETTER PHASE-A NGKUE MFON..BAMUM LETTER PHASE-F VUEQ
    ('\u{16a40}', '\u{16a5e}', BidiClass::LeftToRight), // Lo  [31] MRO LETTER TA..MRO LETTER TEK
    ('\u{16a60}', '\u{16a69}', BidiClass::LeftToRight), // Nd  [10] MRO DIGIT ZERO..MRO DIGIT NINE
    ('\u{16a6e}', '\u{16a6f}', BidiClass::LeftToRight), // Po   [2] MRO DANDA..MRO DOUBLE DANDA
    ('\u{16a70}', '\u{16abe}', BidiClass::LeftToRight), // Lo  [79] TANGSA LETTER OZ..TANGSA LETTER ZA
    ('\u{16ac0}', '\u{16ac9}', BidiClass::LeftToRight), // Nd  [10] TANGSA DIGIT ZERO..TANGSA DIGIT NINE
    ('\u{16ad0}', '\u{16aed}', BidiClass::LeftToRight), // Lo  [30] BASSA VAH LETTER ENNI..BASSA VAH LETTER I
    ('\u{16af0}', '\u{16af4}', BidiClass::NonspacingMark), // Mn   [5] BASSA VAH COMBINING HIGH TONE..BASSA VAH COMBINING HIGH-LOW TONE
    ('\u{16af5}', '\u{16af5}', BidiClass::LeftToRight),    // Po       BASSA VAH FULL STOP
    ('\u{16b00}', '\u{16b2f}', BidiClass::LeftToRight), // Lo  [48] PAHAWH HMONG VOWEL KEEB..PAHAWH HMONG CONSONANT CAU
    ('\u{16b30}', '\u{16b36}', BidiClass::NonspacingMark), // Mn   [7] PAHAWH HMONG MARK CIM TUB..PAHAWH HMONG MARK CIM TAUM
    ('\u{16b37}', '\u{16b3b}', BidiClass::LeftToRight), // Po   [5] PAHAWH HMONG SIGN VOS THOM..PAHAWH HMONG SIGN VOS FEEM
    ('\u{16b3c}', '\u{16b3f}', BidiClass::LeftToRight), // So   [4] PAHAWH HMONG SIGN XYEEM NTXIV..PAHAWH HMONG SIGN XYEEM FAIB
    ('\u{16b40}', '\u{16b43}', BidiClass::LeftToRight), // Lm   [4] PAHAWH HMONG SIGN VOS SEEV..PAHAWH HMONG SIGN IB YAM
    ('\u{16b44}', '\u{16b44}', BidiClass::LeftToRight), // Po       PAHAWH HMONG SIGN XAUS
    ('\u{16b45}', '\u{16b45}', BidiClass::LeftToRight), // So       PAHAWH HMONG SIGN CIM TSOV ROG
    ('\u{16b50}', '\u{16b59}', BidiClass::LeftToRight), // Nd  [10] PAHAWH HMONG DIGIT ZERO..PAHAWH HMONG DIGIT NINE
    ('\u{16b5b}', '\u{16b61}', BidiClass::LeftToRight), // No   [7] PAHAWH HMONG NUMBER TENS..PAHAWH HMONG NUMBER TRILLIONS
    ('\u{16b63}', '\u{16b77}', BidiClass::LeftToRight), // Lo  [21] PAHAWH HMONG SIGN VOS LUB..PAHAWH HMONG SIGN CIM NRES TOS
    ('\u{16b7d}', '\u{16b8f}', BidiClass::LeftToRight), // Lo  [19] PAHAWH HMONG CLAN SIGN TSHEEJ..PAHAWH HMONG CLAN SIGN VWJ
    ('\u{16e40}', '\u{16e7f}', BidiClass::LeftToRight), // L&  [64] MEDEFAIDRIN CAPITAL LETTER M..MEDEFAIDRIN SMALL LETTER Y
    ('\u{16e80}', '\u{16e96}', BidiClass::LeftToRight), // No  [23] MEDEFAIDRIN DIGIT ZERO..MEDEFAIDRIN DIGIT THREE ALTERNATE FORM
    ('\u{16e97}', '\u{16e9a}', BidiClass::LeftToRight), // Po   [4] MEDEFAIDRIN COMMA..MEDEFAIDRIN EXCLAMATION OH
    ('\u{16f00}', '\u{16f4a}', BidiClass::LeftToRight), // Lo  [75] MIAO LETTER PA..MIAO LETTER RTE
    ('\u{16f4f}', '\u{16f4f}', BidiClass::NonspacingMark), // Mn       MIAO SIGN CONSONANT MODIFIER BAR
    ('\u{16f50}', '\u{16f50}', BidiClass::LeftToRight),    // Lo       MIAO LETTER NASALIZATION
    ('\u{16f51}', '\u{16f87}', BidiClass::LeftToRight), // Mc  [55] MIAO SIGN ASPIRATION..MIAO VOWEL SIGN UI
    ('\u{16f8f}', '\u{16f92}', BidiClass::NonspacingMark), // Mn   [4] MIAO TONE RIGHT..MIAO TONE BELOW
    ('\u{16f93}', '\u{16f9f}', BidiClass::LeftToRight), // Lm  [13] MIAO LETTER TONE-2..MIAO LETTER REFORMED TONE-8
    ('\u{16fe0}', '\u{16fe1}', BidiClass::LeftToRight), // Lm   [2] TANGUT ITERATION MARK..NUSHU ITERATION MARK
    ('\u{16fe2}', '\u{16fe2}', BidiClass::OtherNeutral), // Po       OLD CHINESE HOOK MARK
    ('\u{16fe3}', '\u{16fe3}', BidiClass::LeftToRight), // Lm       OLD CHINESE ITERATION MARK
    ('\u{16fe4}', '\u{16fe4}', BidiClass::NonspacingMark), // Mn       KHITAN SMALL SCRIPT FILLER
    ('\u{16ff0}', '\u{16ff1}', BidiClass::LeftToRight), // Mc   [2] VIETNAMESE ALTERNATE READING MARK CA..VIETNAMESE ALTERNATE READING MARK NHAY
    ('\u{17000}', '\u{187f7}', BidiClass::LeftToRight), // Lo [6136] TANGUT IDEOGRAPH-17000..TANGUT IDEOGRAPH-187F7
    ('\u{18800}', '\u{18cd5}', BidiClass::LeftToRight), // Lo [1238] TANGUT COMPONENT-001..KHITAN SMALL SCRIPT CHARACTER-18CD5
    ('\u{18d00}', '\u{18d08}', BidiClass::LeftToRight), // Lo   [9] TANGUT IDEOGRAPH-18D00..TANGUT IDEOGRAPH-18D08
    ('\u{1aff0}', '\u{1aff3}', BidiClass::LeftToRight), // Lm   [4] KATAKANA LETTER MINNAN TONE-2..KATAKANA LETTER MINNAN TONE-5
    ('\u{1aff5}', '\u{1affb}', BidiClass::LeftToRight), // Lm   [7] KATAKANA LETTER MINNAN TONE-7..KATAKANA LETTER MINNAN NASALIZED TONE-5
    ('\u{1affd}', '\u{1affe}', BidiClass::LeftToRight), // Lm   [2] KATAKANA LETTER MINNAN NASALIZED TONE-7..KATAKANA LETTER MINNAN NASALIZED TONE-8
    ('\u{1b000}', '\u{1b122}', BidiClass::LeftToRight), // Lo [291] KATAKANA LETTER ARCHAIC E..KATAKANA LETTER ARCHAIC WU
    ('\u{1b150}', '\u{1b152}', BidiClass::LeftToRight), // Lo   [3] HIRAGANA LETTER SMALL WI..HIRAGANA LETTER SMALL WO
    ('\u{1b164}', '\u{1b167}', BidiClass::LeftToRight), // Lo   [4] KATAKANA LETTER SMALL WI..KATAKANA LETTER SMALL N
    ('\u{1b170}', '\u{1b2fb}', BidiClass::LeftToRight), // Lo [396] NUSHU CHARACTER-1B170..NUSHU CHARACTER-1B2FB
    ('\u{1bc00}', '\u{1bc6a}', BidiClass::LeftToRight), // Lo [107] DUPLOYAN LETTER H..DUPLOYAN LETTER VOCALIC M
    ('\u{1bc70}', '\u{1bc7c}', BidiClass::LeftToRight), // Lo  [13] DUPLOYAN AFFIX LEFT HORIZONTAL SECANT..DUPLOYAN AFFIX ATTACHED TANGENT HOOK
    ('\u{1bc80}', '\u{1bc88}', BidiClass::LeftToRight), // Lo   [9] DUPLOYAN AFFIX HIGH ACUTE..DUPLOYAN AFFIX HIGH VERTICAL
    ('\u{1bc90}', '\u{1bc99}', BidiClass::LeftToRight), // Lo  [10] DUPLOYAN AFFIX LOW ACUTE..DUPLOYAN AFFIX LOW ARROW
    ('\u{1bc9c}', '\u{1bc9c}', BidiClass::LeftToRight), // So       DUPLOYAN SIGN O WITH CROSS
    ('\u{1bc9d}', '\u{1bc9e}', BidiClass::NonspacingMark), // Mn   [2] DUPLOYAN THICK LETTER SELECTOR..DUPLOYAN DOUBLE MARK
    ('\u{1bc9f}', '\u{1bc9f}', BidiClass::LeftToRight), // Po       DUPLOYAN PUNCTUATION CHINOOK FULL STOP
    ('\u{1bca0}', '\u{1bca3}', BidiClass::BoundaryNeutral), // Cf   [4] SHORTHAND FORMAT LETTER OVERLAP..SHORTHAND FORMAT UP STEP
    ('\u{1cf00}', '\u{1cf2d}', BidiClass::NonspacingMark), // Mn  [46] ZNAMENNY COMBINING MARK GORAZDO NIZKO S KRYZHEM ON LEFT..ZNAMENNY COMBINING MARK KRYZH ON LEFT
    ('\u{1cf30}', '\u{1cf46}', BidiClass::NonspacingMark), // Mn  [23] ZNAMENNY COMBINING TONAL RANGE MARK MRACHNO..ZNAMENNY PRIZNAK MODIFIER ROG
    ('\u{1cf50}', '\u{1cfc3}', BidiClass::LeftToRight), // So [116] ZNAMENNY NEUME KRYUK..ZNAMENNY NEUME PAUK
    ('\u{1d000}', '\u{1d0f5}', BidiClass::LeftToRight), // So [246] BYZANTINE MUSICAL SYMBOL PSILI..BYZANTINE MUSICAL SYMBOL GORGON NEO KATO
    ('\u{1d100}', '\u{1d126}', BidiClass::LeftToRight), // So  [39] MUSICAL SYMBOL SINGLE BARLINE..MUSICAL SYMBOL DRUM CLEF-2
    ('\u{1d129}', '\u{1d164}', BidiClass::LeftToRight), // So  [60] MUSICAL SYMBOL MULTIPLE MEASURE REST..MUSICAL SYMBOL ONE HUNDRED TWENTY-EIGHTH NOTE
    ('\u{1d165}', '\u{1d166}', BidiClass::LeftToRight), // Mc   [2] MUSICAL SYMBOL COMBINING STEM..MUSICAL SYMBOL COMBINING SPRECHGESANG STEM
    ('\u{1d167}', '\u{1d169}', BidiClass::NonspacingMark), // Mn   [3] MUSICAL SYMBOL COMBINING TREMOLO-1..MUSICAL SYMBOL COMBINING TREMOLO-3
    ('\u{1d16a}', '\u{1d16c}', BidiClass::LeftToRight), // So   [3] MUSICAL SYMBOL FINGERED TREMOLO-1..MUSICAL SYMBOL FINGERED TREMOLO-3
    ('\u{1d16d}', '\u{1d172}', BidiClass::LeftToRight), // Mc   [6] MUSICAL SYMBOL COMBINING AUGMENTATION DOT..MUSICAL SYMBOL COMBINING FLAG-5
    ('\u{1d173}', '\u{1d17a}', BidiClass::BoundaryNeutral), // Cf   [8] MUSICAL SYMBOL BEGIN BEAM..MUSICAL SYMBOL END PHRASE
    ('\u{1d17b}', '\u{1d182}', BidiClass::NonspacingMark), // Mn   [8] MUSICAL SYMBOL COMBINING ACCENT..MUSICAL SYMBOL COMBINING LOURE
    ('\u{1d183}', '\u{1d184}', BidiClass::LeftToRight), // So   [2] MUSICAL SYMBOL ARPEGGIATO UP..MUSICAL SYMBOL ARPEGGIATO DOWN
    ('\u{1d185}', '\u{1d18b}', BidiClass::NonspacingMark), // Mn   [7] MUSICAL SYMBOL COMBINING DOIT..MUSICAL SYMBOL COMBINING TRIPLE TONGUE
    ('\u{1d18c}', '\u{1d1a9}', BidiClass::LeftToRight), // So  [30] MUSICAL SYMBOL RINFORZANDO..MUSICAL SYMBOL DEGREE SLASH
    ('\u{1d1aa}', '\u{1d1ad}', BidiClass::NonspacingMark), // Mn   [4] MUSICAL SYMBOL COMBINING DOWN BOW..MUSICAL SYMBOL COMBINING SNAP PIZZICATO
    ('\u{1d1ae}', '\u{1d1e8}', BidiClass::LeftToRight), // So  [59] MUSICAL SYMBOL PEDAL MARK..MUSICAL SYMBOL KIEVAN FLAT SIGN
    ('\u{1d1e9}', '\u{1d1ea}', BidiClass::OtherNeutral), // So   [2] MUSICAL SYMBOL SORI..MUSICAL SYMBOL KORON
    ('\u{1d200}', '\u{1d241}', BidiClass::OtherNeutral), // So  [66] GREEK VOCAL NOTATION SYMBOL-1..GREEK INSTRUMENTAL NOTATION SYMBOL-54
    ('\u{1d242}', '\u{1d244}', BidiClass::NonspacingMark), // Mn   [3] COMBINING GREEK MUSICAL TRISEME..COMBINING GREEK MUSICAL PENTASEME
    ('\u{1d245}', '\u{1d245}', BidiClass::OtherNeutral),   // So       GREEK MUSICAL LEIMMA
    ('\u{1d2e0}', '\u{1d2f3}', BidiClass::LeftToRight), // No  [20] MAYAN NUMERAL ZERO..MAYAN NUMERAL NINETEEN
    ('\u{1d300}', '\u{1d356}', BidiClass::OtherNeutral), // So  [87] MONOGRAM FOR EARTH..TETRAGRAM FOR FOSTERING
    ('\u{1d360}', '\u{1d378}', BidiClass::LeftToRight), // No  [25] COUNTING ROD UNIT DIGIT ONE..TALLY MARK FIVE
    ('\u{1d400}', '\u{1d454}', BidiClass::LeftToRight), // L&  [85] MATHEMATICAL BOLD CAPITAL A..MATHEMATICAL ITALIC SMALL G
    ('\u{1d456}', '\u{1d49c}', BidiClass::LeftToRight), // L&  [71] MATHEMATICAL ITALIC SMALL I..MATHEMATICAL SCRIPT CAPITAL A
    ('\u{1d49e}', '\u{1d49f}', BidiClass::LeftToRight), // L&   [2] MATHEMATICAL SCRIPT CAPITAL C..MATHEMATICAL SCRIPT CAPITAL D
    ('\u{1d4a2}', '\u{1d4a2}', BidiClass::LeftToRight), // L&       MATHEMATICAL SCRIPT CAPITAL G
    ('\u{1d4a5}', '\u{1d4a6}', BidiClass::LeftToRight), // L&   [2] MATHEMATICAL SCRIPT CAPITAL J..MATHEMATICAL SCRIPT CAPITAL K
    ('\u{1d4a9}', '\u{1d4ac}', BidiClass::LeftToRight), // L&   [4] MATHEMATICAL SCRIPT CAPITAL N..MATHEMATICAL SCRIPT CAPITAL Q
    ('\u{1d4ae}', '\u{1d4b9}', BidiClass::LeftToRight), // L&  [12] MATHEMATICAL SCRIPT CAPITAL S..MATHEMATICAL SCRIPT SMALL D
    ('\u{1d4bb}', '\u{1d4bb}', BidiClass::LeftToRight), // L&       MATHEMATICAL SCRIPT SMALL F
    ('\u{1d4bd}', '\u{1d4c3}', BidiClass::LeftToRight), // L&   [7] MATHEMATICAL SCRIPT SMALL H..MATHEMATICAL SCRIPT SMALL N
    ('\u{1d4c5}', '\u{1d505}', BidiClass::LeftToRight), // L&  [65] MATHEMATICAL SCRIPT SMALL P..MATHEMATICAL FRAKTUR CAPITAL B
    ('\u{1d507}', '\u{1d50a}', BidiClass::LeftToRight), // L&   [4] MATHEMATICAL FRAKTUR CAPITAL D..MATHEMATICAL FRAKTUR CAPITAL G
    ('\u{1d50d}', '\u{1d514}', BidiClass::LeftToRight), // L&   [8] MATHEMATICAL FRAKTUR CAPITAL J..MATHEMATICAL FRAKTUR CAPITAL Q
    ('\u{1d516}', '\u{1d51c}', BidiClass::LeftToRight), // L&   [7] MATHEMATICAL FRAKTUR CAPITAL S..MATHEMATICAL FRAKTUR CAPITAL Y
    ('\u{1d51e}', '\u{1d539}', BidiClass::LeftToRight), // L&  [28] MATHEMATICAL FRAKTUR SMALL A..MATHEMATICAL DOUBLE-STRUCK CAPITAL B
    ('\u{1d53b}', '\u{1d53e}', BidiClass::LeftToRight), // L&   [4] MATHEMATICAL DOUBLE-STRUCK CAPITAL D..MATHEMATICAL DOUBLE-STRUCK CAPITAL G
    ('\u{1d540}', '\u{1d544}', BidiClass::LeftToRight), // L&   [5] MATHEMATICAL DOUBLE-STRUCK CAPITAL I..MATHEMATICAL DOUBLE-STRUCK CAPITAL M
    ('\u{1d546}', '\u{1d546}', BidiClass::LeftToRight), // L&       MATHEMATICAL DOUBLE-STRUCK CAPITAL O
    ('\u{1d54a}', '\u{1d550}', BidiClass::LeftToRight), // L&   [7] MATHEMATICAL DOUBLE-STRUCK CAPITAL S..MATHEMATICAL DOUBLE-STRUCK CAPITAL Y
    ('\u{1d552}', '\u{1d6a5}', BidiClass::LeftToRight), // L& [340] MATHEMATICAL DOUBLE-STRUCK SMALL A..MATHEMATICAL ITALIC SMALL DOTLESS J
    ('\u{1d6a8}', '\u{1d6c0}', BidiClass::LeftToRight), // L&  [25] MATHEMATICAL BOLD CAPITAL ALPHA..MATHEMATICAL BOLD CAPITAL OMEGA
    ('\u{1d6c1}', '\u{1d6c1}', BidiClass::LeftToRight), // Sm       MATHEMATICAL BOLD NABLA
    ('\u{1d6c2}', '\u{1d6da}', BidiClass::LeftToRight), // L&  [25] MATHEMATICAL BOLD SMALL ALPHA..MATHEMATICAL BOLD SMALL OMEGA
    ('\u{1d6db}', '\u{1d6db}', BidiClass::OtherNeutral), // Sm       MATHEMATICAL BOLD PARTIAL DIFFERENTIAL
    ('\u{1d6dc}', '\u{1d6fa}', BidiClass::LeftToRight), // L&  [31] MATHEMATICAL BOLD EPSILON SYMBOL..MATHEMATICAL ITALIC CAPITAL OMEGA
    ('\u{1d6fb}', '\u{1d6fb}', BidiClass::LeftToRight), // Sm       MATHEMATICAL ITALIC NABLA
    ('\u{1d6fc}', '\u{1d714}', BidiClass::LeftToRight), // L&  [25] MATHEMATICAL ITALIC SMALL ALPHA..MATHEMATICAL ITALIC SMALL OMEGA
    ('\u{1d715}', '\u{1d715}', BidiClass::OtherNeutral), // Sm       MATHEMATICAL ITALIC PARTIAL DIFFERENTIAL
    ('\u{1d716}', '\u{1d734}', BidiClass::LeftToRight), // L&  [31] MATHEMATICAL ITALIC EPSILON SYMBOL..MATHEMATICAL BOLD ITALIC CAPITAL OMEGA
    ('\u{1d735}', '\u{1d735}', BidiClass::LeftToRight), // Sm       MATHEMATICAL BOLD ITALIC NABLA
    ('\u{1d736}', '\u{1d74e}', BidiClass::LeftToRight), // L&  [25] MATHEMATICAL BOLD ITALIC SMALL ALPHA..MATHEMATICAL BOLD ITALIC SMALL OMEGA
    ('\u{1d74f}', '\u{1d74f}', BidiClass::OtherNeutral), // Sm       MATHEMATICAL BOLD ITALIC PARTIAL DIFFERENTIAL
    ('\u{1d750}', '\u{1d76e}', BidiClass::LeftToRight), // L&  [31] MATHEMATICAL BOLD ITALIC EPSILON SYMBOL..MATHEMATICAL SANS-SERIF BOLD CAPITAL OMEGA
    ('\u{1d76f}', '\u{1d76f}', BidiClass::LeftToRight), // Sm       MATHEMATICAL SANS-SERIF BOLD NABLA
    ('\u{1d770}', '\u{1d788}', BidiClass::LeftToRight), // L&  [25] MATHEMATICAL SANS-SERIF BOLD SMALL ALPHA..MATHEMATICAL SANS-SERIF BOLD SMALL OMEGA
    ('\u{1d789}', '\u{1d789}', BidiClass::OtherNeutral), // Sm       MATHEMATICAL SANS-SERIF BOLD PARTIAL DIFFERENTIAL
    ('\u{1d78a}', '\u{1d7a8}', BidiClass::LeftToRight), // L&  [31] MATHEMATICAL SANS-SERIF BOLD EPSILON SYMBOL..MATHEMATICAL SANS-SERIF BOLD ITALIC CAPITAL OMEGA
    ('\u{1d7a9}', '\u{1d7a9}', BidiClass::LeftToRight), // Sm       MATHEMATICAL SANS-SERIF BOLD ITALIC NABLA
    ('\u{1d7aa}', '\u{1d7c2}', BidiClass::LeftToRight), // L&  [25] MATHEMATICAL SANS-SERIF BOLD ITALIC SMALL ALPHA..MATHEMATICAL SANS-SERIF BOLD ITALIC SMALL OMEGA
    ('\u{1d7c3}', '\u{1d7c3}', BidiClass::OtherNeutral), // Sm       MATHEMATICAL SANS-SERIF BOLD ITALIC PARTIAL DIFFERENTIAL
    ('\u{1d7c4}', '\u{1d7cb}', BidiClass::LeftToRight), // L&   [8] MATHEMATICAL SANS-SERIF BOLD ITALIC EPSILON SYMBOL..MATHEMATICAL BOLD SMALL DIGAMMA
    ('\u{1d7ce}', '\u{1d7ff}', BidiClass::EuropeanNumber), // Nd  [50] MATHEMATICAL BOLD DIGIT ZERO..MATHEMATICAL MONOSPACE DIGIT NINE
    ('\u{1d800}', '\u{1d9ff}', BidiClass::LeftToRight), // So [512] SIGNWRITING HAND-FIST INDEX..SIGNWRITING HEAD
    ('\u{1da00}', '\u{1da36}', BidiClass::NonspacingMark), // Mn  [55] SIGNWRITING HEAD RIM..SIGNWRITING AIR SUCKING IN
    ('\u{1da37}', '\u{1da3a}', BidiClass::LeftToRight), // So   [4] SIGNWRITING AIR BLOW SMALL ROTATIONS..SIGNWRITING BREATH EXHALE
    ('\u{1da3b}', '\u{1da6c}', BidiClass::NonspacingMark), // Mn  [50] SIGNWRITING MOUTH CLOSED NEUTRAL..SIGNWRITING EXCITEMENT
    ('\u{1da6d}', '\u{1da74}', BidiClass::LeftToRight), // So   [8] SIGNWRITING SHOULDER HIP SPINE..SIGNWRITING TORSO-FLOORPLANE TWISTING
    ('\u{1da75}', '\u{1da75}', BidiClass::NonspacingMark), // Mn       SIGNWRITING UPPER BODY TILTING FROM HIP JOINTS
    ('\u{1da76}', '\u{1da83}', BidiClass::LeftToRight), // So  [14] SIGNWRITING LIMB COMBINATION..SIGNWRITING LOCATION DEPTH
    ('\u{1da84}', '\u{1da84}', BidiClass::NonspacingMark), // Mn       SIGNWRITING LOCATION HEAD NECK
    ('\u{1da85}', '\u{1da86}', BidiClass::LeftToRight), // So   [2] SIGNWRITING LOCATION TORSO..SIGNWRITING LOCATION LIMBS DIGITS
    ('\u{1da87}', '\u{1da8b}', BidiClass::LeftToRight), // Po   [5] SIGNWRITING COMMA..SIGNWRITING PARENTHESIS
    ('\u{1da9b}', '\u{1da9f}', BidiClass::NonspacingMark), // Mn   [5] SIGNWRITING FILL MODIFIER-2..SIGNWRITING FILL MODIFIER-6
    ('\u{1daa1}', '\u{1daaf}', BidiClass::NonspacingMark), // Mn  [15] SIGNWRITING ROTATION MODIFIER-2..SIGNWRITING ROTATION MODIFIER-16
    ('\u{1df00}', '\u{1df09}', BidiClass::LeftToRight), // L&  [10] LATIN SMALL LETTER FENG DIGRAPH WITH TRILL..LATIN SMALL LETTER T WITH HOOK AND RETROFLEX HOOK
    ('\u{1df0a}', '\u{1df0a}', BidiClass::LeftToRight), // Lo       LATIN LETTER RETROFLEX CLICK WITH RETROFLEX HOOK
    ('\u{1df0b}', '\u{1df1e}', BidiClass::LeftToRight), // L&  [20] LATIN SMALL LETTER ESH WITH DOUBLE BAR..LATIN SMALL LETTER S WITH CURL
    ('\u{1e000}', '\u{1e006}', BidiClass::NonspacingMark), // Mn   [7] COMBINING GLAGOLITIC LETTER AZU..COMBINING GLAGOLITIC LETTER ZHIVETE
    ('\u{1e008}', '\u{1e018}', BidiClass::NonspacingMark), // Mn  [17] COMBINING GLAGOLITIC LETTER ZEMLJA..COMBINING GLAGOLITIC LETTER HERU
    ('\u{1e01b}', '\u{1e021}', BidiClass::NonspacingMark), // Mn   [7] COMBINING GLAGOLITIC LETTER SHTA..COMBINING GLAGOLITIC LETTER YATI
    ('\u{1e023}', '\u{1e024}', BidiClass::NonspacingMark), // Mn   [2] COMBINING GLAGOLITIC LETTER YU..COMBINING GLAGOLITIC LETTER SMALL YUS
    ('\u{1e026}', '\u{1e02a}', BidiClass::NonspacingMark), // Mn   [5] COMBINING GLAGOLITIC LETTER YO..COMBINING GLAGOLITIC LETTER FITA
    ('\u{1e100}', '\u{1e12c}', BidiClass::LeftToRight), // Lo  [45] NYIAKENG PUACHUE HMONG LETTER MA..NYIAKENG PUACHUE HMONG LETTER W
    ('\u{1e130}', '\u{1e136}', BidiClass::NonspacingMark), // Mn   [7] NYIAKENG PUACHUE HMONG TONE-B..NYIAKENG PUACHUE HMONG TONE-D
    ('\u{1e137}', '\u{1e13d}', BidiClass::LeftToRight), // Lm   [7] NYIAKENG PUACHUE HMONG SIGN FOR PERSON..NYIAKENG PUACHUE HMONG SYLLABLE LENGTHENER
    ('\u{1e140}', '\u{1e149}', BidiClass::LeftToRight), // Nd  [10] NYIAKENG PUACHUE HMONG DIGIT ZERO..NYIAKENG PUACHUE HMONG DIGIT NINE
    ('\u{1e14e}', '\u{1e14e}', BidiClass::LeftToRight), // Lo       NYIAKENG PUACHUE HMONG LOGOGRAM NYAJ
    ('\u{1e14f}', '\u{1e14f}', BidiClass::LeftToRight), // So       NYIAKENG PUACHUE HMONG CIRCLED CA
    ('\u{1e290}', '\u{1e2ad}', BidiClass::LeftToRight), // Lo  [30] TOTO LETTER PA..TOTO LETTER A
    ('\u{1e2ae}', '\u{1e2ae}', BidiClass::NonspacingMark), // Mn       TOTO SIGN RISING TONE
    ('\u{1e2c0}', '\u{1e2eb}', BidiClass::LeftToRight), // Lo  [44] WANCHO LETTER AA..WANCHO LETTER YIH
    ('\u{1e2ec}', '\u{1e2ef}', BidiClass::NonspacingMark), // Mn   [4] WANCHO TONE TUP..WANCHO TONE KOINI
    ('\u{1e2f0}', '\u{1e2f9}', BidiClass::LeftToRight), // Nd  [10] WANCHO DIGIT ZERO..WANCHO DIGIT NINE
    ('\u{1e2ff}', '\u{1e2ff}', BidiClass::EuropeanTerminator), // Sc       WANCHO NGUN SIGN
    ('\u{1e7e0}', '\u{1e7e6}', BidiClass::LeftToRight), // Lo   [7] ETHIOPIC SYLLABLE HHYA..ETHIOPIC SYLLABLE HHYO
    ('\u{1e7e8}', '\u{1e7eb}', BidiClass::LeftToRight), // Lo   [4] ETHIOPIC SYLLABLE GURAGE HHWA..ETHIOPIC SYLLABLE HHWE
    ('\u{1e7ed}', '\u{1e7ee}', BidiClass::LeftToRight), // Lo   [2] ETHIOPIC SYLLABLE GURAGE MWI..ETHIOPIC SYLLABLE GURAGE MWEE
    ('\u{1e7f0}', '\u{1e7fe}', BidiClass::LeftToRight), // Lo  [15] ETHIOPIC SYLLABLE GURAGE QWI..ETHIOPIC SYLLABLE GURAGE PWEE
    ('\u{1e800}', '\u{1e8c4}', BidiClass::RightToLeft), // Lo [197] MENDE KIKAKUI SYLLABLE M001 KI..MENDE KIKAKUI SYLLABLE M060 NYON
    ('\u{1e8c5}', '\u{1e8c6}', BidiClass::RightToLeft), // Cn   [2] <reserved-1E8C5>..<reserved-1E8C6>
    ('\u{1e8c7}', '\u{1e8cf}', BidiClass::RightToLeft), // No   [9] MENDE KIKAKUI DIGIT ONE..MENDE KIKAKUI DIGIT NINE
    ('\u{1e8d0}', '\u{1e8d6}', BidiClass::NonspacingMark), // Mn   [7] MENDE KIKAKUI COMBINING NUMBER TEENS..MENDE KIKAKUI COMBINING NUMBER MILLIONS
    ('\u{1e8d7}', '\u{1e8ff}', BidiClass::RightToLeft), // Cn  [41] <reserved-1E8D7>..<reserved-1E8FF>
    ('\u{1e900}', '\u{1e943}', BidiClass::RightToLeft), // L&  [68] ADLAM CAPITAL LETTER ALIF..ADLAM SMALL LETTER SHA
    ('\u{1e944}', '\u{1e94a}', BidiClass::NonspacingMark), // Mn   [7] ADLAM ALIF LENGTHENER..ADLAM NUKTA
    ('\u{1e94b}', '\u{1e94b}', BidiClass::RightToLeft),    // Lm       ADLAM NASALIZATION MARK
    ('\u{1e94c}', '\u{1e94f}', BidiClass::RightToLeft), // Cn   [4] <reserved-1E94C>..<reserved-1E94F>
    ('\u{1e950}', '\u{1e959}', BidiClass::RightToLeft), // Nd  [10] ADLAM DIGIT ZERO..ADLAM DIGIT NINE
    ('\u{1e95a}', '\u{1e95d}', BidiClass::RightToLeft), // Cn   [4] <reserved-1E95A>..<reserved-1E95D>
    ('\u{1e95e}', '\u{1e95f}', BidiClass::RightToLeft), // Po   [2] ADLAM INITIAL EXCLAMATION MARK..ADLAM INITIAL QUESTION MARK
    ('\u{1e960}', '\u{1ec6f}', BidiClass::RightToLeft), // Cn [784] <reserved-1E960>..<reserved-1EC6F>
    ('\u{1ec70}', '\u{1ec70}', BidiClass::ArabicLetter), // Cn       <reserved-1EC70>
    ('\u{1ec71}', '\u{1ecab}', BidiClass::ArabicLetter), // No  [59] INDIC SIYAQ NUMBER ONE..INDIC SIYAQ NUMBER PREFIXED NINE
    ('\u{1ecac}', '\u{1ecac}', BidiClass::ArabicLetter), // So       INDIC SIYAQ PLACEHOLDER
    ('\u{1ecad}', '\u{1ecaf}', BidiClass::ArabicLetter), // No   [3] INDIC SIYAQ FRACTION ONE QUARTER..INDIC SIYAQ FRACTION THREE QUARTERS
    ('\u{1ecb0}', '\u{1ecb0}', BidiClass::ArabicLetter), // Sc       INDIC SIYAQ RUPEE MARK
    ('\u{1ecb1}', '\u{1ecb4}', BidiClass::ArabicLetter), // No   [4] INDIC SIYAQ NUMBER ALTERNATE ONE..INDIC SIYAQ ALTERNATE LAKH MARK
    ('\u{1ecb5}', '\u{1ecbf}', BidiClass::ArabicLetter), // Cn  [11] <reserved-1ECB5>..<reserved-1ECBF>
    ('\u{1ecc0}', '\u{1ecff}', BidiClass::RightToLeft), // Cn  [64] <reserved-1ECC0>..<reserved-1ECFF>
    ('\u{1ed00}', '\u{1ed00}', BidiClass::ArabicLetter), // Cn       <reserved-1ED00>
    ('\u{1ed01}', '\u{1ed2d}', BidiClass::ArabicLetter), // No  [45] OTTOMAN SIYAQ NUMBER ONE..OTTOMAN SIYAQ NUMBER NINETY THOUSAND
    ('\u{1ed2e}', '\u{1ed2e}', BidiClass::ArabicLetter), // So       OTTOMAN SIYAQ MARRATAN
    ('\u{1ed2f}', '\u{1ed3d}', BidiClass::ArabicLetter), // No  [15] OTTOMAN SIYAQ ALTERNATE NUMBER TWO..OTTOMAN SIYAQ FRACTION ONE SIXTH
    ('\u{1ed3e}', '\u{1ed4f}', BidiClass::ArabicLetter), // Cn  [18] <reserved-1ED3E>..<reserved-1ED4F>
    ('\u{1ed50}', '\u{1edff}', BidiClass::RightToLeft), // Cn [176] <reserved-1ED50>..<reserved-1EDFF>
    ('\u{1ee00}', '\u{1ee03}', BidiClass::ArabicLetter), // Lo   [4] ARABIC MATHEMATICAL ALEF..ARABIC MATHEMATICAL DAL
    ('\u{1ee04}', '\u{1ee04}', BidiClass::ArabicLetter), // Cn       <reserved-1EE04>
    ('\u{1ee05}', '\u{1ee1f}', BidiClass::ArabicLetter), // Lo  [27] ARABIC MATHEMATICAL WAW..ARABIC MATHEMATICAL DOTLESS QAF
    ('\u{1ee20}', '\u{1ee20}', BidiClass::ArabicLetter), // Cn       <reserved-1EE20>
    ('\u{1ee21}', '\u{1ee22}', BidiClass::ArabicLetter), // Lo   [2] ARABIC MATHEMATICAL INITIAL BEH..ARABIC MATHEMATICAL INITIAL JEEM
    ('\u{1ee23}', '\u{1ee23}', BidiClass::ArabicLetter), // Cn       <reserved-1EE23>
    ('\u{1ee24}', '\u{1ee24}', BidiClass::ArabicLetter), // Lo       ARABIC MATHEMATICAL INITIAL HEH
    ('\u{1ee25}', '\u{1ee26}', BidiClass::ArabicLetter), // Cn   [2] <reserved-1EE25>..<reserved-1EE26>
    ('\u{1ee27}', '\u{1ee27}', BidiClass::ArabicLetter), // Lo       ARABIC MATHEMATICAL INITIAL HAH
    ('\u{1ee28}', '\u{1ee28}', BidiClass::ArabicLetter), // Cn       <reserved-1EE28>
    ('\u{1ee29}', '\u{1ee32}', BidiClass::ArabicLetter), // Lo  [10] ARABIC MATHEMATICAL INITIAL YEH..ARABIC MATHEMATICAL INITIAL QAF
    ('\u{1ee33}', '\u{1ee33}', BidiClass::ArabicLetter), // Cn       <reserved-1EE33>
    ('\u{1ee34}', '\u{1ee37}', BidiClass::ArabicLetter), // Lo   [4] ARABIC MATHEMATICAL INITIAL SHEEN..ARABIC MATHEMATICAL INITIAL KHAH
    ('\u{1ee38}', '\u{1ee38}', BidiClass::ArabicLetter), // Cn       <reserved-1EE38>
    ('\u{1ee39}', '\u{1ee39}', BidiClass::ArabicLetter), // Lo       ARABIC MATHEMATICAL INITIAL DAD
    ('\u{1ee3a}', '\u{1ee3a}', BidiClass::ArabicLetter), // Cn       <reserved-1EE3A>
    ('\u{1ee3b}', '\u{1ee3b}', BidiClass::ArabicLetter), // Lo       ARABIC MATHEMATICAL INITIAL GHAIN
    ('\u{1ee3c}', '\u{1ee41}', BidiClass::ArabicLetter), // Cn   [6] <reserved-1EE3C>..<reserved-1EE41>
    ('\u{1ee42}', '\u{1ee42}', BidiClass::ArabicLetter), // Lo       ARABIC MATHEMATICAL TAILED JEEM
    ('\u{1ee43}', '\u{1ee46}', BidiClass::ArabicLetter), // Cn   [4] <reserved-1EE43>..<reserved-1EE46>
    ('\u{1ee47}', '\u{1ee47}', BidiClass::ArabicLetter), // Lo       ARABIC MATHEMATICAL TAILED HAH
    ('\u{1ee48}', '\u{1ee48}', BidiClass::ArabicLetter), // Cn       <reserved-1EE48>
    ('\u{1ee49}', '\u{1ee49}', BidiClass::ArabicLetter), // Lo       ARABIC MATHEMATICAL TAILED YEH
    ('\u{1ee4a}', '\u{1ee4a}', BidiClass::ArabicLetter), // Cn       <reserved-1EE4A>
    ('\u{1ee4b}', '\u{1ee4b}', BidiClass::ArabicLetter), // Lo       ARABIC MATHEMATICAL TAILED LAM
    ('\u{1ee4c}', '\u{1ee4c}', BidiClass::ArabicLetter), // Cn       <reserved-1EE4C>
    ('\u{1ee4d}', '\u{1ee4f}', BidiClass::ArabicLetter), // Lo   [3] ARABIC MATHEMATICAL TAILED NOON..ARABIC MATHEMATICAL TAILED AIN
    ('\u{1ee50}', '\u{1ee50}', BidiClass::ArabicLetter), // Cn       <reserved-1EE50>
    ('\u{1ee51}', '\u{1ee52}', BidiClass::ArabicLetter), // Lo   [2] ARABIC MATHEMATICAL TAILED SAD..ARABIC MATHEMATICAL TAILED QAF
    ('\u{1ee53}', '\u{1ee53}', BidiClass::ArabicLetter), // Cn       <reserved-1EE53>
    ('\u{1ee54}', '\u{1ee54}', BidiClass::ArabicLetter), // Lo       ARABIC MATHEMATICAL TAILED SHEEN
    ('\u{1ee55}', '\u{1ee56}', BidiClass::ArabicLetter), // Cn   [2] <reserved-1EE55>..<reserved-1EE56>
    ('\u{1ee57}', '\u{1ee57}', BidiClass::ArabicLetter), // Lo       ARABIC MATHEMATICAL TAILED KHAH
    ('\u{1ee58}', '\u{1ee58}', BidiClass::ArabicLetter), // Cn       <reserved-1EE58>
    ('\u{1ee59}', '\u{1ee59}', BidiClass::ArabicLetter), // Lo       ARABIC MATHEMATICAL TAILED DAD
    ('\u{1ee5a}', '\u{1ee5a}', BidiClass::ArabicLetter), // Cn       <reserved-1EE5A>
    ('\u{1ee5b}', '\u{1ee5b}', BidiClass::ArabicLetter), // Lo       ARABIC MATHEMATICAL TAILED GHAIN
    ('\u{1ee5c}', '\u{1ee5c}', BidiClass::ArabicLetter), // Cn       <reserved-1EE5C>
    ('\u{1ee5d}', '\u{1ee5d}', BidiClass::ArabicLetter), // Lo       ARABIC MATHEMATICAL TAILED DOTLESS NOON
    ('\u{1ee5e}', '\u{1ee5e}', BidiClass::ArabicLetter), // Cn       <reserved-1EE5E>
    ('\u{1ee5f}', '\u{1ee5f}', BidiClass::ArabicLetter), // Lo       ARABIC MATHEMATICAL TAILED DOTLESS QAF
    ('\u{1ee60}', '\u{1ee60}', BidiClass::ArabicLetter), // Cn       <reserved-1EE60>
    ('\u{1ee61}', '\u{1ee62}', BidiClass::ArabicLetter), // Lo   [2] ARABIC MATHEMATICAL STRETCHED BEH..ARABIC MATHEMATICAL STRETCHED JEEM
    ('\u{1ee63}', '\u{1ee63}', BidiClass::ArabicLetter), // Cn       <reserved-1EE63>
    ('\u{1ee64}', '\u{1ee64}', BidiClass::ArabicLetter), // Lo       ARABIC MATHEMATICAL STRETCHED HEH
    ('\u{1ee65}', '\u{1ee66}', BidiClass::ArabicLetter), // Cn   [2] <reserved-1EE65>..<reserved-1EE66>
    ('\u{1ee67}', '\u{1ee6a}', BidiClass::ArabicLetter), // Lo   [4] ARABIC MATHEMATICAL STRETCHED HAH..ARABIC MATHEMATICAL STRETCHED KAF
    ('\u{1ee6b}', '\u{1ee6b}', BidiClass::ArabicLetter), // Cn       <reserved-1EE6B>
    ('\u{1ee6c}', '\u{1ee72}', BidiClass::ArabicLetter), // Lo   [7] ARABIC MATHEMATICAL STRETCHED MEEM..ARABIC MATHEMATICAL STRETCHED QAF
    ('\u{1ee73}', '\u{1ee73}', BidiClass::ArabicLetter), // Cn       <reserved-1EE73>
    ('\u{1ee74}', '\u{1ee77}', BidiClass::ArabicLetter), // Lo   [4] ARABIC MATHEMATICAL STRETCHED SHEEN..ARABIC MATHEMATICAL STRETCHED KHAH
    ('\u{1ee78}', '\u{1ee78}', BidiClass::ArabicLetter), // Cn       <reserved-1EE78>
    ('\u{1ee79}', '\u{1ee7c}', BidiClass::ArabicLetter), // Lo   [4] ARABIC MATHEMATICAL STRETCHED DAD..ARABIC MATHEMATICAL STRETCHED DOTLESS BEH
    ('\u{1ee7d}', '\u{1ee7d}', BidiClass::ArabicLetter), // Cn       <reserved-1EE7D>
    ('\u{1ee7e}', '\u{1ee7e}', BidiClass::ArabicLetter), // Lo       ARABIC MATHEMATICAL STRETCHED DOTLESS FEH
    ('\u{1ee7f}', '\u{1ee7f}', BidiClass::ArabicLetter), // Cn       <reserved-1EE7F>
    ('\u{1ee80}', '\u{1ee89}', BidiClass::ArabicLetter), // Lo  [10] ARABIC MATHEMATICAL LOOPED ALEF..ARABIC MATHEMATICAL LOOPED YEH
    ('\u{1ee8a}', '\u{1ee8a}', BidiClass::ArabicLetter), // Cn       <reserved-1EE8A>
    ('\u{1ee8b}', '\u{1ee9b}', BidiClass::ArabicLetter), // Lo  [17] ARABIC MATHEMATICAL LOOPED LAM..ARABIC MATHEMATICAL LOOPED GHAIN
    ('\u{1ee9c}', '\u{1eea0}', BidiClass::ArabicLetter), // Cn   [5] <reserved-1EE9C>..<reserved-1EEA0>
    ('\u{1eea1}', '\u{1eea3}', BidiClass::ArabicLetter), // Lo   [3] ARABIC MATHEMATICAL DOUBLE-STRUCK BEH..ARABIC MATHEMATICAL DOUBLE-STRUCK DAL
    ('\u{1eea4}', '\u{1eea4}', BidiClass::ArabicLetter), // Cn       <reserved-1EEA4>
    ('\u{1eea5}', '\u{1eea9}', BidiClass::ArabicLetter), // Lo   [5] ARABIC MATHEMATICAL DOUBLE-STRUCK WAW..ARABIC MATHEMATICAL DOUBLE-STRUCK YEH
    ('\u{1eeaa}', '\u{1eeaa}', BidiClass::ArabicLetter), // Cn       <reserved-1EEAA>
    ('\u{1eeab}', '\u{1eebb}', BidiClass::ArabicLetter), // Lo  [17] ARABIC MATHEMATICAL DOUBLE-STRUCK LAM..ARABIC MATHEMATICAL DOUBLE-STRUCK GHAIN
    ('\u{1eebc}', '\u{1eeef}', BidiClass::ArabicLetter), // Cn  [52] <reserved-1EEBC>..<reserved-1EEEF>
    ('\u{1eef0}', '\u{1eef1}', BidiClass::OtherNeutral), // Sm   [2] ARABIC MATHEMATICAL OPERATOR MEEM WITH HAH WITH TATWEEL..ARABIC MATHEMATICAL OPERATOR HAH WITH DAL
    ('\u{1eef2}', '\u{1eeff}', BidiClass::ArabicLetter), // Cn  [14] <reserved-1EEF2>..<reserved-1EEFF>
    ('\u{1ef00}', '\u{1efff}', BidiClass::RightToLeft), // Cn [256] <reserved-1EF00>..<reserved-1EFFF>
    ('\u{1f000}', '\u{1f02b}', BidiClass::OtherNeutral), // So  [44] MAHJONG TILE EAST WIND..MAHJONG TILE BACK
    ('\u{1f030}', '\u{1f093}', BidiClass::OtherNeutral), // So [100] DOMINO TILE HORIZONTAL BACK..DOMINO TILE VERTICAL-06-06
    ('\u{1f0a0}', '\u{1f0ae}', BidiClass::OtherNeutral), // So  [15] PLAYING CARD BACK..PLAYING CARD KING OF SPADES
    ('\u{1f0b1}', '\u{1f0bf}', BidiClass::OtherNeutral), // So  [15] PLAYING CARD ACE OF HEARTS..PLAYING CARD RED JOKER
    ('\u{1f0c1}', '\u{1f0cf}', BidiClass::OtherNeutral), // So  [15] PLAYING CARD ACE OF DIAMONDS..PLAYING CARD BLACK JOKER
    ('\u{1f0d1}', '\u{1f0f5}', BidiClass::OtherNeutral), // So  [37] PLAYING CARD ACE OF CLUBS..PLAYING CARD TRUMP-21
    ('\u{1f100}', '\u{1f10a}', BidiClass::EuropeanNumber), // No  [11] DIGIT ZERO FULL STOP..DIGIT NINE COMMA
    ('\u{1f10b}', '\u{1f10c}', BidiClass::OtherNeutral), // No   [2] DINGBAT CIRCLED SANS-SERIF DIGIT ZERO..DINGBAT NEGATIVE CIRCLED SANS-SERIF DIGIT ZERO
    ('\u{1f10d}', '\u{1f10f}', BidiClass::OtherNeutral), // So   [3] CIRCLED ZERO WITH SLASH..CIRCLED DOLLAR SIGN WITH OVERLAID BACKSLASH
    ('\u{1f110}', '\u{1f12e}', BidiClass::LeftToRight), // So  [31] PARENTHESIZED LATIN CAPITAL LETTER A..CIRCLED WZ
    ('\u{1f12f}', '\u{1f12f}', BidiClass::OtherNeutral), // So       COPYLEFT SYMBOL
    ('\u{1f130}', '\u{1f169}', BidiClass::LeftToRight), // So  [58] SQUARED LATIN CAPITAL LETTER A..NEGATIVE CIRCLED LATIN CAPITAL LETTER Z
    ('\u{1f16a}', '\u{1f16f}', BidiClass::OtherNeutral), // So   [6] RAISED MC SIGN..CIRCLED HUMAN FIGURE
    ('\u{1f170}', '\u{1f1ac}', BidiClass::LeftToRight), // So  [61] NEGATIVE SQUARED LATIN CAPITAL LETTER A..SQUARED VOD
    ('\u{1f1ad}', '\u{1f1ad}', BidiClass::OtherNeutral), // So       MASK WORK SYMBOL
    ('\u{1f1e6}', '\u{1f202}', BidiClass::LeftToRight), // So  [29] REGIONAL INDICATOR SYMBOL LETTER A..SQUARED KATAKANA SA
    ('\u{1f210}', '\u{1f23b}', BidiClass::LeftToRight), // So  [44] SQUARED CJK UNIFIED IDEOGRAPH-624B..SQUARED CJK UNIFIED IDEOGRAPH-914D
    ('\u{1f240}', '\u{1f248}', BidiClass::LeftToRight), // So   [9] TORTOISE SHELL BRACKETED CJK UNIFIED IDEOGRAPH-672C..TORTOISE SHELL BRACKETED CJK UNIFIED IDEOGRAPH-6557
    ('\u{1f250}', '\u{1f251}', BidiClass::LeftToRight), // So   [2] CIRCLED IDEOGRAPH ADVANTAGE..CIRCLED IDEOGRAPH ACCEPT
    ('\u{1f260}', '\u{1f265}', BidiClass::OtherNeutral), // So   [6] ROUNDED SYMBOL FOR FU..ROUNDED SYMBOL FOR CAI
    ('\u{1f300}', '\u{1f3fa}', BidiClass::OtherNeutral), // So [251] CYCLONE..AMPHORA
    ('\u{1f3fb}', '\u{1f3ff}', BidiClass::OtherNeutral), // Sk   [5] EMOJI MODIFIER FITZPATRICK TYPE-1-2..EMOJI MODIFIER FITZPATRICK TYPE-6
    ('\u{1f400}', '\u{1f6d7}', BidiClass::OtherNeutral), // So [728] RAT..ELEVATOR
    ('\u{1f6dd}', '\u{1f6ec}', BidiClass::OtherNeutral), // So  [16] PLAYGROUND SLIDE..AIRPLANE ARRIVING
    ('\u{1f6f0}', '\u{1f6fc}', BidiClass::OtherNeutral), // So  [13] SATELLITE..ROLLER SKATE
    ('\u{1f700}', '\u{1f773}', BidiClass::OtherNeutral), // So [116] ALCHEMICAL SYMBOL FOR QUINTESSENCE..ALCHEMICAL SYMBOL FOR HALF OUNCE
    ('\u{1f780}', '\u{1f7d8}', BidiClass::OtherNeutral), // So  [89] BLACK LEFT-POINTING ISOSCELES RIGHT TRIANGLE..NEGATIVE CIRCLED SQUARE
    ('\u{1f7e0}', '\u{1f7eb}', BidiClass::OtherNeutral), // So  [12] LARGE ORANGE CIRCLE..LARGE BROWN SQUARE
    ('\u{1f7f0}', '\u{1f7f0}', BidiClass::OtherNeutral), // So       HEAVY EQUALS SIGN
    ('\u{1f800}', '\u{1f80b}', BidiClass::OtherNeutral), // So  [12] LEFTWARDS ARROW WITH SMALL TRIANGLE ARROWHEAD..DOWNWARDS ARROW WITH LARGE TRIANGLE ARROWHEAD
    ('\u{1f810}', '\u{1f847}', BidiClass::OtherNeutral), // So  [56] LEFTWARDS ARROW WITH SMALL EQUILATERAL ARROWHEAD..DOWNWARDS HEAVY ARROW
    ('\u{1f850}', '\u{1f859}', BidiClass::OtherNeutral), // So  [10] LEFTWARDS SANS-SERIF ARROW..UP DOWN SANS-SERIF ARROW
    ('\u{1f860}', '\u{1f887}', BidiClass::OtherNeutral), // So  [40] WIDE-HEADED LEFTWARDS LIGHT BARB ARROW..WIDE-HEADED SOUTH WEST VERY HEAVY BARB ARROW
    ('\u{1f890}', '\u{1f8ad}', BidiClass::OtherNeutral), // So  [30] LEFTWARDS TRIANGLE ARROWHEAD..WHITE ARROW SHAFT WIDTH TWO THIRDS
    ('\u{1f8b0}', '\u{1f8b1}', BidiClass::OtherNeutral), // So   [2] ARROW POINTING UPWARDS THEN NORTH WEST..ARROW POINTING RIGHTWARDS THEN CURVING SOUTH WEST
    ('\u{1f900}', '\u{1fa53}', BidiClass::OtherNeutral), // So [340] CIRCLED CROSS FORMEE WITH FOUR DOTS..BLACK CHESS KNIGHT-BISHOP
    ('\u{1fa60}', '\u{1fa6d}', BidiClass::OtherNeutral), // So  [14] XIANGQI RED GENERAL..XIANGQI BLACK SOLDIER
    ('\u{1fa70}', '\u{1fa74}', BidiClass::OtherNeutral), // So   [5] BALLET SHOES..THONG SANDAL
    ('\u{1fa78}', '\u{1fa7c}', BidiClass::OtherNeutral), // So   [5] DROP OF BLOOD..CRUTCH
    ('\u{1fa80}', '\u{1fa86}', BidiClass::OtherNeutral), // So   [7] YO-YO..NESTING DOLLS
    ('\u{1fa90}', '\u{1faac}', BidiClass::OtherNeutral), // So  [29] RINGED PLANET..HAMSA
    ('\u{1fab0}', '\u{1faba}', BidiClass::OtherNeutral), // So  [11] FLY..NEST WITH EGGS
    ('\u{1fac0}', '\u{1fac5}', BidiClass::OtherNeutral), // So   [6] ANATOMICAL HEART..PERSON WITH CROWN
    ('\u{1fad0}', '\u{1fad9}', BidiClass::OtherNeutral), // So  [10] BLUEBERRIES..JAR
    ('\u{1fae0}', '\u{1fae7}', BidiClass::OtherNeutral), // So   [8] MELTING FACE..BUBBLES
    ('\u{1faf0}', '\u{1faf6}', BidiClass::OtherNeutral), // So   [7] HAND WITH INDEX FINGER AND THUMB CROSSED..HEART HANDS
    ('\u{1fb00}', '\u{1fb92}', BidiClass::OtherNeutral), // So [147] BLOCK SEXTANT-1..UPPER HALF INVERSE MEDIUM SHADE AND LOWER HALF BLOCK
    ('\u{1fb94}', '\u{1fbca}', BidiClass::OtherNeutral), // So  [55] LEFT HALF INVERSE MEDIUM SHADE AND RIGHT HALF BLOCK..WHITE UP-POINTING CHEVRON
    ('\u{1fbf0}', '\u{1fbf9}', BidiClass::EuropeanNumber), // Nd  [10] SEGMENTED DIGIT ZERO..SEGMENTED DIGIT NINE
    ('\u{1fffe}', '\u{1ffff}', BidiClass::BoundaryNeutral), // Cn   [2] <noncharacter-1FFFE>..<noncharacter-1FFFF>
    ('\u{20000}', '\u{2a6df}', BidiClass::LeftToRight), // Lo [42720] CJK UNIFIED IDEOGRAPH-20000..CJK UNIFIED IDEOGRAPH-2A6DF
    ('\u{2a700}', '\u{2b738}', BidiClass::LeftToRight), // Lo [4153] CJK UNIFIED IDEOGRAPH-2A700..CJK UNIFIED IDEOGRAPH-2B738
    ('\u{2b740}', '\u{2b81d}', BidiClass::LeftToRight), // Lo [222] CJK UNIFIED IDEOGRAPH-2B740..CJK UNIFIED IDEOGRAPH-2B81D
    ('\u{2b820}', '\u{2cea1}', BidiClass::LeftToRight), // Lo [5762] CJK UNIFIED IDEOGRAPH-2B820..CJK UNIFIED IDEOGRAPH-2CEA1
    ('\u{2ceb0}', '\u{2ebe0}', BidiClass::LeftToRight), // Lo [7473] CJK UNIFIED IDEOGRAPH-2CEB0..CJK UNIFIED IDEOGRAPH-2EBE0
    ('\u{2f800}', '\u{2fa1d}', BidiClass::LeftToRight), // Lo [542] CJK COMPATIBILITY IDEOGRAPH-2F800..CJK COMPATIBILITY IDEOGRAPH-2FA1D
    ('\u{2fffe}', '\u{2ffff}', BidiClass::BoundaryNeutral), // Cn   [2] <noncharacter-2FFFE>..<noncharacter-2FFFF>
    ('\u{30000}', '\u{3134a}', BidiClass::LeftToRight), // Lo [4939] CJK UNIFIED IDEOGRAPH-30000..CJK UNIFIED IDEOGRAPH-3134A
    ('\u{3fffe}', '\u{3ffff}', BidiClass::BoundaryNeutral), // Cn   [2] <noncharacter-3FFFE>..<noncharacter-3FFFF>
    ('\u{4fffe}', '\u{4ffff}', BidiClass::BoundaryNeutral), // Cn   [2] <noncharacter-4FFFE>..<noncharacter-4FFFF>
    ('\u{5fffe}', '\u{5ffff}', BidiClass::BoundaryNeutral), // Cn   [2] <noncharacter-5FFFE>..<noncharacter-5FFFF>
    ('\u{6fffe}', '\u{6ffff}', BidiClass::BoundaryNeutral), // Cn   [2] <noncharacter-6FFFE>..<noncharacter-6FFFF>
    ('\u{7fffe}', '\u{7ffff}', BidiClass::BoundaryNeutral), // Cn   [2] <noncharacter-7FFFE>..<noncharacter-7FFFF>
    ('\u{8fffe}', '\u{8ffff}', BidiClass::BoundaryNeutral), // Cn   [2] <noncharacter-8FFFE>..<noncharacter-8FFFF>
    ('\u{9fffe}', '\u{9ffff}', BidiClass::BoundaryNeutral), // Cn   [2] <noncharacter-9FFFE>..<noncharacter-9FFFF>
    ('\u{afffe}', '\u{affff}', BidiClass::BoundaryNeutral), // Cn   [2] <noncharacter-AFFFE>..<noncharacter-AFFFF>
    ('\u{bfffe}', '\u{bffff}', BidiClass::BoundaryNeutral), // Cn   [2] <noncharacter-BFFFE>..<noncharacter-BFFFF>
    ('\u{cfffe}', '\u{cffff}', BidiClass::BoundaryNeutral), // Cn   [2] <noncharacter-CFFFE>..<noncharacter-CFFFF>
    ('\u{dfffe}', '\u{e0000}', BidiClass::BoundaryNeutral), // Cn   [3] <noncharacter-DFFFE>..<reserved-E0000>
    ('\u{e0001}', '\u{e0001}', BidiClass::BoundaryNeutral), // Cf       LANGUAGE TAG
    ('\u{e0002}', '\u{e001f}', BidiClass::BoundaryNeutral), // Cn  [30] <reserved-E0002>..<reserved-E001F>
    ('\u{e0020}', '\u{e007f}', BidiClass::BoundaryNeutral), // Cf  [96] TAG SPACE..CANCEL TAG
    ('\u{e0080}', '\u{e00ff}', BidiClass::BoundaryNeutral), // Cn [128] <reserved-E0080>..<reserved-E00FF>
    ('\u{e0100}', '\u{e01ef}', BidiClass::NonspacingMark), // Mn [240] VARIATION SELECTOR-17..VARIATION SELECTOR-256
    ('\u{e01f0}', '\u{e0fff}', BidiClass::BoundaryNeutral), // Cn [3600] <reserved-E01F0>..<reserved-E0FFF>
    ('\u{efffe}', '\u{effff}', BidiClass::BoundaryNeutral), // Cn   [2] <noncharacter-EFFFE>..<noncharacter-EFFFF>
    ('\u{f0000}', '\u{ffffd}', BidiClass::LeftToRight), // Co [65534] <private-use-F0000>..<private-use-FFFFD>
    ('\u{ffffe}', '\u{fffff}', BidiClass::BoundaryNeutral), // Cn   [2] <noncharacter-FFFFE>..<noncharacter-FFFFF>
    ('\u{100000}', '\u{10fffd}', BidiClass::LeftToRight), // Co [65534] <private-use-100000>..<private-use-10FFFD>
    ('\u{10fffe}', '\u{10ffff}', BidiClass::BoundaryNeutral), // Cn   [2] <noncharacter-10FFFE>..<noncharacter-10FFFF>
];
