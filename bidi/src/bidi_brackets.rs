//! Generated from bidi/data/BidiBrackets.txt by bidi/generate/src/main.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BracketType {
    Open,
    Close,
}
pub const BIDI_BRACKETS: &'static [(char, char, BracketType)] = &[
    ('\u{28}', '\u{29}', BracketType::Open),   // LEFT PARENTHESIS
    ('\u{29}', '\u{28}', BracketType::Close),  // RIGHT PARENTHESIS
    ('\u{5b}', '\u{5d}', BracketType::Open),   // LEFT SQUARE BRACKET
    ('\u{5d}', '\u{5b}', BracketType::Close),  // RIGHT SQUARE BRACKET
    ('\u{7b}', '\u{7d}', BracketType::Open),   // LEFT CURLY BRACKET
    ('\u{7d}', '\u{7b}', BracketType::Close),  // RIGHT CURLY BRACKET
    ('\u{f3a}', '\u{f3b}', BracketType::Open), // TIBETAN MARK GUG RTAGS GYON
    ('\u{f3b}', '\u{f3a}', BracketType::Close), // TIBETAN MARK GUG RTAGS GYAS
    ('\u{f3c}', '\u{f3d}', BracketType::Open), // TIBETAN MARK ANG KHANG GYON
    ('\u{f3d}', '\u{f3c}', BracketType::Close), // TIBETAN MARK ANG KHANG GYAS
    ('\u{169b}', '\u{169c}', BracketType::Open), // OGHAM FEATHER MARK
    ('\u{169c}', '\u{169b}', BracketType::Close), // OGHAM REVERSED FEATHER MARK
    ('\u{2045}', '\u{2046}', BracketType::Open), // LEFT SQUARE BRACKET WITH QUILL
    ('\u{2046}', '\u{2045}', BracketType::Close), // RIGHT SQUARE BRACKET WITH QUILL
    ('\u{207d}', '\u{207e}', BracketType::Open), // SUPERSCRIPT LEFT PARENTHESIS
    ('\u{207e}', '\u{207d}', BracketType::Close), // SUPERSCRIPT RIGHT PARENTHESIS
    ('\u{208d}', '\u{208e}', BracketType::Open), // SUBSCRIPT LEFT PARENTHESIS
    ('\u{208e}', '\u{208d}', BracketType::Close), // SUBSCRIPT RIGHT PARENTHESIS
    ('\u{2308}', '\u{2309}', BracketType::Open), // LEFT CEILING
    ('\u{2309}', '\u{2308}', BracketType::Close), // RIGHT CEILING
    ('\u{230a}', '\u{230b}', BracketType::Open), // LEFT FLOOR
    ('\u{230b}', '\u{230a}', BracketType::Close), // RIGHT FLOOR
    ('\u{2329}', '\u{232a}', BracketType::Open), // LEFT-POINTING ANGLE BRACKET
    ('\u{232a}', '\u{2329}', BracketType::Close), // RIGHT-POINTING ANGLE BRACKET
    ('\u{2768}', '\u{2769}', BracketType::Open), // MEDIUM LEFT PARENTHESIS ORNAMENT
    ('\u{2769}', '\u{2768}', BracketType::Close), // MEDIUM RIGHT PARENTHESIS ORNAMENT
    ('\u{276a}', '\u{276b}', BracketType::Open), // MEDIUM FLATTENED LEFT PARENTHESIS ORNAMENT
    ('\u{276b}', '\u{276a}', BracketType::Close), // MEDIUM FLATTENED RIGHT PARENTHESIS ORNAMENT
    ('\u{276c}', '\u{276d}', BracketType::Open), // MEDIUM LEFT-POINTING ANGLE BRACKET ORNAMENT
    ('\u{276d}', '\u{276c}', BracketType::Close), // MEDIUM RIGHT-POINTING ANGLE BRACKET ORNAMENT
    ('\u{276e}', '\u{276f}', BracketType::Open), // HEAVY LEFT-POINTING ANGLE QUOTATION MARK ORNAMENT
    ('\u{276f}', '\u{276e}', BracketType::Close), // HEAVY RIGHT-POINTING ANGLE QUOTATION MARK ORNAMENT
    ('\u{2770}', '\u{2771}', BracketType::Open),  // HEAVY LEFT-POINTING ANGLE BRACKET ORNAMENT
    ('\u{2771}', '\u{2770}', BracketType::Close), // HEAVY RIGHT-POINTING ANGLE BRACKET ORNAMENT
    ('\u{2772}', '\u{2773}', BracketType::Open),  // LIGHT LEFT TORTOISE SHELL BRACKET ORNAMENT
    ('\u{2773}', '\u{2772}', BracketType::Close), // LIGHT RIGHT TORTOISE SHELL BRACKET ORNAMENT
    ('\u{2774}', '\u{2775}', BracketType::Open),  // MEDIUM LEFT CURLY BRACKET ORNAMENT
    ('\u{2775}', '\u{2774}', BracketType::Close), // MEDIUM RIGHT CURLY BRACKET ORNAMENT
    ('\u{27c5}', '\u{27c6}', BracketType::Open),  // LEFT S-SHAPED BAG DELIMITER
    ('\u{27c6}', '\u{27c5}', BracketType::Close), // RIGHT S-SHAPED BAG DELIMITER
    ('\u{27e6}', '\u{27e7}', BracketType::Open),  // MATHEMATICAL LEFT WHITE SQUARE BRACKET
    ('\u{27e7}', '\u{27e6}', BracketType::Close), // MATHEMATICAL RIGHT WHITE SQUARE BRACKET
    ('\u{27e8}', '\u{27e9}', BracketType::Open),  // MATHEMATICAL LEFT ANGLE BRACKET
    ('\u{27e9}', '\u{27e8}', BracketType::Close), // MATHEMATICAL RIGHT ANGLE BRACKET
    ('\u{27ea}', '\u{27eb}', BracketType::Open),  // MATHEMATICAL LEFT DOUBLE ANGLE BRACKET
    ('\u{27eb}', '\u{27ea}', BracketType::Close), // MATHEMATICAL RIGHT DOUBLE ANGLE BRACKET
    ('\u{27ec}', '\u{27ed}', BracketType::Open),  // MATHEMATICAL LEFT WHITE TORTOISE SHELL BRACKET
    ('\u{27ed}', '\u{27ec}', BracketType::Close), // MATHEMATICAL RIGHT WHITE TORTOISE SHELL BRACKET
    ('\u{27ee}', '\u{27ef}', BracketType::Open),  // MATHEMATICAL LEFT FLATTENED PARENTHESIS
    ('\u{27ef}', '\u{27ee}', BracketType::Close), // MATHEMATICAL RIGHT FLATTENED PARENTHESIS
    ('\u{2983}', '\u{2984}', BracketType::Open),  // LEFT WHITE CURLY BRACKET
    ('\u{2984}', '\u{2983}', BracketType::Close), // RIGHT WHITE CURLY BRACKET
    ('\u{2985}', '\u{2986}', BracketType::Open),  // LEFT WHITE PARENTHESIS
    ('\u{2986}', '\u{2985}', BracketType::Close), // RIGHT WHITE PARENTHESIS
    ('\u{2987}', '\u{2988}', BracketType::Open),  // Z NOTATION LEFT IMAGE BRACKET
    ('\u{2988}', '\u{2987}', BracketType::Close), // Z NOTATION RIGHT IMAGE BRACKET
    ('\u{2989}', '\u{298a}', BracketType::Open),  // Z NOTATION LEFT BINDING BRACKET
    ('\u{298a}', '\u{2989}', BracketType::Close), // Z NOTATION RIGHT BINDING BRACKET
    ('\u{298b}', '\u{298c}', BracketType::Open),  // LEFT SQUARE BRACKET WITH UNDERBAR
    ('\u{298c}', '\u{298b}', BracketType::Close), // RIGHT SQUARE BRACKET WITH UNDERBAR
    ('\u{298d}', '\u{2990}', BracketType::Open),  // LEFT SQUARE BRACKET WITH TICK IN TOP CORNER
    ('\u{298e}', '\u{298f}', BracketType::Close), // RIGHT SQUARE BRACKET WITH TICK IN BOTTOM CORNER
    ('\u{298f}', '\u{298e}', BracketType::Open),  // LEFT SQUARE BRACKET WITH TICK IN BOTTOM CORNER
    ('\u{2990}', '\u{298d}', BracketType::Close), // RIGHT SQUARE BRACKET WITH TICK IN TOP CORNER
    ('\u{2991}', '\u{2992}', BracketType::Open),  // LEFT ANGLE BRACKET WITH DOT
    ('\u{2992}', '\u{2991}', BracketType::Close), // RIGHT ANGLE BRACKET WITH DOT
    ('\u{2993}', '\u{2994}', BracketType::Open),  // LEFT ARC LESS-THAN BRACKET
    ('\u{2994}', '\u{2993}', BracketType::Close), // RIGHT ARC GREATER-THAN BRACKET
    ('\u{2995}', '\u{2996}', BracketType::Open),  // DOUBLE LEFT ARC GREATER-THAN BRACKET
    ('\u{2996}', '\u{2995}', BracketType::Close), // DOUBLE RIGHT ARC LESS-THAN BRACKET
    ('\u{2997}', '\u{2998}', BracketType::Open),  // LEFT BLACK TORTOISE SHELL BRACKET
    ('\u{2998}', '\u{2997}', BracketType::Close), // RIGHT BLACK TORTOISE SHELL BRACKET
    ('\u{29d8}', '\u{29d9}', BracketType::Open),  // LEFT WIGGLY FENCE
    ('\u{29d9}', '\u{29d8}', BracketType::Close), // RIGHT WIGGLY FENCE
    ('\u{29da}', '\u{29db}', BracketType::Open),  // LEFT DOUBLE WIGGLY FENCE
    ('\u{29db}', '\u{29da}', BracketType::Close), // RIGHT DOUBLE WIGGLY FENCE
    ('\u{29fc}', '\u{29fd}', BracketType::Open),  // LEFT-POINTING CURVED ANGLE BRACKET
    ('\u{29fd}', '\u{29fc}', BracketType::Close), // RIGHT-POINTING CURVED ANGLE BRACKET
    ('\u{2e22}', '\u{2e23}', BracketType::Open),  // TOP LEFT HALF BRACKET
    ('\u{2e23}', '\u{2e22}', BracketType::Close), // TOP RIGHT HALF BRACKET
    ('\u{2e24}', '\u{2e25}', BracketType::Open),  // BOTTOM LEFT HALF BRACKET
    ('\u{2e25}', '\u{2e24}', BracketType::Close), // BOTTOM RIGHT HALF BRACKET
    ('\u{2e26}', '\u{2e27}', BracketType::Open),  // LEFT SIDEWAYS U BRACKET
    ('\u{2e27}', '\u{2e26}', BracketType::Close), // RIGHT SIDEWAYS U BRACKET
    ('\u{2e28}', '\u{2e29}', BracketType::Open),  // LEFT DOUBLE PARENTHESIS
    ('\u{2e29}', '\u{2e28}', BracketType::Close), // RIGHT DOUBLE PARENTHESIS
    ('\u{2e55}', '\u{2e56}', BracketType::Open),  // LEFT SQUARE BRACKET WITH STROKE
    ('\u{2e56}', '\u{2e55}', BracketType::Close), // RIGHT SQUARE BRACKET WITH STROKE
    ('\u{2e57}', '\u{2e58}', BracketType::Open),  // LEFT SQUARE BRACKET WITH DOUBLE STROKE
    ('\u{2e58}', '\u{2e57}', BracketType::Close), // RIGHT SQUARE BRACKET WITH DOUBLE STROKE
    ('\u{2e59}', '\u{2e5a}', BracketType::Open),  // TOP HALF LEFT PARENTHESIS
    ('\u{2e5a}', '\u{2e59}', BracketType::Close), // TOP HALF RIGHT PARENTHESIS
    ('\u{2e5b}', '\u{2e5c}', BracketType::Open),  // BOTTOM HALF LEFT PARENTHESIS
    ('\u{2e5c}', '\u{2e5b}', BracketType::Close), // BOTTOM HALF RIGHT PARENTHESIS
    ('\u{3008}', '\u{3009}', BracketType::Open),  // LEFT ANGLE BRACKET
    ('\u{3009}', '\u{3008}', BracketType::Close), // RIGHT ANGLE BRACKET
    ('\u{300a}', '\u{300b}', BracketType::Open),  // LEFT DOUBLE ANGLE BRACKET
    ('\u{300b}', '\u{300a}', BracketType::Close), // RIGHT DOUBLE ANGLE BRACKET
    ('\u{300c}', '\u{300d}', BracketType::Open),  // LEFT CORNER BRACKET
    ('\u{300d}', '\u{300c}', BracketType::Close), // RIGHT CORNER BRACKET
    ('\u{300e}', '\u{300f}', BracketType::Open),  // LEFT WHITE CORNER BRACKET
    ('\u{300f}', '\u{300e}', BracketType::Close), // RIGHT WHITE CORNER BRACKET
    ('\u{3010}', '\u{3011}', BracketType::Open),  // LEFT BLACK LENTICULAR BRACKET
    ('\u{3011}', '\u{3010}', BracketType::Close), // RIGHT BLACK LENTICULAR BRACKET
    ('\u{3014}', '\u{3015}', BracketType::Open),  // LEFT TORTOISE SHELL BRACKET
    ('\u{3015}', '\u{3014}', BracketType::Close), // RIGHT TORTOISE SHELL BRACKET
    ('\u{3016}', '\u{3017}', BracketType::Open),  // LEFT WHITE LENTICULAR BRACKET
    ('\u{3017}', '\u{3016}', BracketType::Close), // RIGHT WHITE LENTICULAR BRACKET
    ('\u{3018}', '\u{3019}', BracketType::Open),  // LEFT WHITE TORTOISE SHELL BRACKET
    ('\u{3019}', '\u{3018}', BracketType::Close), // RIGHT WHITE TORTOISE SHELL BRACKET
    ('\u{301a}', '\u{301b}', BracketType::Open),  // LEFT WHITE SQUARE BRACKET
    ('\u{301b}', '\u{301a}', BracketType::Close), // RIGHT WHITE SQUARE BRACKET
    ('\u{fe59}', '\u{fe5a}', BracketType::Open),  // SMALL LEFT PARENTHESIS
    ('\u{fe5a}', '\u{fe59}', BracketType::Close), // SMALL RIGHT PARENTHESIS
    ('\u{fe5b}', '\u{fe5c}', BracketType::Open),  // SMALL LEFT CURLY BRACKET
    ('\u{fe5c}', '\u{fe5b}', BracketType::Close), // SMALL RIGHT CURLY BRACKET
    ('\u{fe5d}', '\u{fe5e}', BracketType::Open),  // SMALL LEFT TORTOISE SHELL BRACKET
    ('\u{fe5e}', '\u{fe5d}', BracketType::Close), // SMALL RIGHT TORTOISE SHELL BRACKET
    ('\u{ff08}', '\u{ff09}', BracketType::Open),  // FULLWIDTH LEFT PARENTHESIS
    ('\u{ff09}', '\u{ff08}', BracketType::Close), // FULLWIDTH RIGHT PARENTHESIS
    ('\u{ff3b}', '\u{ff3d}', BracketType::Open),  // FULLWIDTH LEFT SQUARE BRACKET
    ('\u{ff3d}', '\u{ff3b}', BracketType::Close), // FULLWIDTH RIGHT SQUARE BRACKET
    ('\u{ff5b}', '\u{ff5d}', BracketType::Open),  // FULLWIDTH LEFT CURLY BRACKET
    ('\u{ff5d}', '\u{ff5b}', BracketType::Close), // FULLWIDTH RIGHT CURLY BRACKET
    ('\u{ff5f}', '\u{ff60}', BracketType::Open),  // FULLWIDTH LEFT WHITE PARENTHESIS
    ('\u{ff60}', '\u{ff5f}', BracketType::Close), // FULLWIDTH RIGHT WHITE PARENTHESIS
    ('\u{ff62}', '\u{ff63}', BracketType::Open),  // HALFWIDTH LEFT CORNER BRACKET
    ('\u{ff63}', '\u{ff62}', BracketType::Close), // HALFWIDTH RIGHT CORNER BRACKET
];
