use std::{fmt::{Display, Debug}, slice::{Iter, IterMut}, iter::Rev, error::Error, ops::Add};

use self::constants::*;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum AsciiError {
    BadChar,
    Buffer,
    Overflow,
}

impl Error for AsciiError {}
impl Display for AsciiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadChar =>  write!(f, "character is not a valid ascii character"),
            Self::Buffer =>   write!(f, "Buffer size too small"),
            Self::Overflow => write!(f, "Overflow Unpacking Txt"),
        }
    }
}

pub type AsciiChar = u8;

// TODO: I am not sure whether it is better to do large match statements, like I have now that match
//       each letter to their uppercase/lowercase counterparts or use ranges A..=Z and add/subtract
//       32. Both solutions have merits but I don't know which is better.

#[inline]
pub const fn ascii_char_as_lower(character: AsciiChar) -> AsciiChar {
    match character {
        ASCII_UPPERCASE_A => ASCII_LOWERCASE_A,
        ASCII_UPPERCASE_B => ASCII_LOWERCASE_B,
        ASCII_UPPERCASE_C => ASCII_LOWERCASE_C,
        ASCII_UPPERCASE_D => ASCII_LOWERCASE_D,
        ASCII_UPPERCASE_E => ASCII_LOWERCASE_E,
        ASCII_UPPERCASE_F => ASCII_LOWERCASE_F,
        ASCII_UPPERCASE_G => ASCII_LOWERCASE_G,
        ASCII_UPPERCASE_H => ASCII_LOWERCASE_H,
        ASCII_UPPERCASE_I => ASCII_LOWERCASE_I,
        ASCII_UPPERCASE_J => ASCII_LOWERCASE_J,
        ASCII_UPPERCASE_K => ASCII_LOWERCASE_K,
        ASCII_UPPERCASE_L => ASCII_LOWERCASE_L,
        ASCII_UPPERCASE_M => ASCII_LOWERCASE_M,
        ASCII_UPPERCASE_N => ASCII_LOWERCASE_N,
        ASCII_UPPERCASE_O => ASCII_LOWERCASE_O,
        ASCII_UPPERCASE_P => ASCII_LOWERCASE_P,
        ASCII_UPPERCASE_Q => ASCII_LOWERCASE_Q,
        ASCII_UPPERCASE_R => ASCII_LOWERCASE_R,
        ASCII_UPPERCASE_S => ASCII_LOWERCASE_S,
        ASCII_UPPERCASE_T => ASCII_LOWERCASE_T,
        ASCII_UPPERCASE_U => ASCII_LOWERCASE_U,
        ASCII_UPPERCASE_V => ASCII_LOWERCASE_V,
        ASCII_UPPERCASE_W => ASCII_LOWERCASE_W,
        ASCII_UPPERCASE_X => ASCII_LOWERCASE_X,
        ASCII_UPPERCASE_Y => ASCII_LOWERCASE_Y,
        ASCII_UPPERCASE_Z => ASCII_LOWERCASE_Z,
        _ => character,
    }
}

#[inline]
pub const fn ascii_char_as_upper(character: AsciiChar) -> AsciiChar {
    match character {
        ASCII_LOWERCASE_A => ASCII_UPPERCASE_A,
        ASCII_LOWERCASE_B => ASCII_UPPERCASE_B,
        ASCII_LOWERCASE_C => ASCII_UPPERCASE_C,
        ASCII_LOWERCASE_D => ASCII_UPPERCASE_D,
        ASCII_LOWERCASE_E => ASCII_UPPERCASE_E,
        ASCII_LOWERCASE_F => ASCII_UPPERCASE_F,
        ASCII_LOWERCASE_G => ASCII_UPPERCASE_G,
        ASCII_LOWERCASE_H => ASCII_UPPERCASE_H,
        ASCII_LOWERCASE_I => ASCII_UPPERCASE_I,
        ASCII_LOWERCASE_J => ASCII_UPPERCASE_J,
        ASCII_LOWERCASE_K => ASCII_UPPERCASE_K,
        ASCII_LOWERCASE_L => ASCII_UPPERCASE_L,
        ASCII_LOWERCASE_M => ASCII_UPPERCASE_M,
        ASCII_LOWERCASE_N => ASCII_UPPERCASE_N,
        ASCII_LOWERCASE_O => ASCII_UPPERCASE_O,
        ASCII_LOWERCASE_P => ASCII_UPPERCASE_P,
        ASCII_LOWERCASE_Q => ASCII_UPPERCASE_Q,
        ASCII_LOWERCASE_R => ASCII_UPPERCASE_R,
        ASCII_LOWERCASE_S => ASCII_UPPERCASE_S,
        ASCII_LOWERCASE_T => ASCII_UPPERCASE_T,
        ASCII_LOWERCASE_U => ASCII_UPPERCASE_U,
        ASCII_LOWERCASE_V => ASCII_UPPERCASE_V,
        ASCII_LOWERCASE_W => ASCII_UPPERCASE_W,
        ASCII_LOWERCASE_X => ASCII_UPPERCASE_X,
        ASCII_LOWERCASE_Y => ASCII_UPPERCASE_Y,
        ASCII_LOWERCASE_Z => ASCII_UPPERCASE_Z,
        _ => character,
    }
}

pub mod constants {
    use super::{AsciiChar, AsciiString};

    pub const ASCII_NUL: AsciiChar                       = 000;
    pub const ASCII_START_OF_HEADING: AsciiChar          = 001;
    pub const ASCII_START_OF_TEXT: AsciiChar             = 002;
    pub const ASCII_END_OF_TEXT: AsciiChar               = 003;
    pub const ASCII_END_OF_TRANSMISSION: AsciiChar       = 004;
    pub const ASCII_ENQUIRY: AsciiChar                   = 005;
    pub const ASCII_ACKNOWLEDGE: AsciiChar               = 006;
    pub const ASCII_BELL: AsciiChar                      = 007;
    pub const ASCII_BACKSPACE: AsciiChar                 = 008;
    pub const ASCII_HORIZONTAL_TAB: AsciiChar            = 009;
    pub const ASCII_LINE_FEED: AsciiChar                 = 010;
    pub const ASCII_VERTICAL_TABULATION: AsciiChar       = 011;
    pub const ASCII_FORM_FEED: AsciiChar                 = 012;
    pub const ASCII_CARRIAGE_RETURN: AsciiChar           = 013;
    pub const ASCII_SHIFT_OUT: AsciiChar                 = 014;
    pub const ASCII_SHIFT_IN: AsciiChar                  = 015;
    pub const ASCII_DATA_LINK_ESCAPE: AsciiChar          = 016;
    pub const ASCII_DEVICE_CONTROL_ONE: AsciiChar        = 017;
    pub const ASCII_DEVICE_CONTROL_TWO: AsciiChar        = 018;
    pub const ASCII_DEVICE_CONTROL_THREE: AsciiChar      = 019;
    pub const ASCII_DEVICE_CONTROL_FOUR: AsciiChar       = 020;
    pub const ASCII_NEGATIVE_ACKNOWLEDGE: AsciiChar      = 021;
    pub const ASCII_SYNCHRONOUS_IDLE: AsciiChar          = 022;
    pub const ASCII_END_OF_TRANSMISSION_BLOCK: AsciiChar = 023;
    pub const ASCII_CANCEL: AsciiChar                    = 024;
    pub const ASCII_END_OF_MEDIUM: AsciiChar             = 025;
    pub const ASCII_SUBSTITUTE: AsciiChar                = 026;
    pub const ASCII_ESCAPE: AsciiChar                    = 027;
    pub const ASCII_FILE_SEPARATOR: AsciiChar            = 028;
    pub const ASCII_GROUP_SEPARATOR: AsciiChar           = 029;
    pub const ASCII_RECORD_SEPARATOR: AsciiChar          = 030;
    pub const ASCII_UNIT_SEPARATOR: AsciiChar            = 031;
    pub const ASCII_SPACE: AsciiChar                     = 032;
    pub const ASCII_EXCLAMATION_MARK: AsciiChar          = 033;
    pub const ASCII_DOUBLE_QUOTES: AsciiChar             = 034;
    pub const ASCII_NUMBER_SIGN: AsciiChar               = 035;
    pub const ASCII_DOLLAR_SIGN: AsciiChar               = 036;
    pub const ASCII_PERCENT_SIGN: AsciiChar              = 037;
    pub const ASCII_AMPERSAND: AsciiChar                 = 038;
    pub const ASCII_SINGLE_QUOTE: AsciiChar              = 039;
    pub const ASCII_OPEN_PARENTHESIS: AsciiChar          = 040;
    pub const ASCII_CLOSE_PARENTHESIS: AsciiChar         = 041;
    pub const ASCII_ASTERISK: AsciiChar                  = 042;
    pub const ASCII_PLUS: AsciiChar                      = 043;
    pub const ASCII_COMMA: AsciiChar                     = 044;
    pub const ASCII_HYPHEN_MINUS: AsciiChar              = 045;
    pub const ASCII_PERIOD: AsciiChar                    = 046;
    pub const ASCII_SLASH: AsciiChar                     = 047;
    pub const ASCII_ZERO: AsciiChar                      = 048;
    pub const ASCII_ONE: AsciiChar                       = 049;
    pub const ASCII_TWO: AsciiChar                       = 050;
    pub const ASCII_THREE: AsciiChar                     = 051;
    pub const ASCII_FOUR: AsciiChar                      = 052;
    pub const ASCII_FIVE: AsciiChar                      = 053;
    pub const ASCII_SIX: AsciiChar                       = 054;
    pub const ASCII_SEVEN: AsciiChar                     = 055;
    pub const ASCII_EIGHT: AsciiChar                     = 056;
    pub const ASCII_NINE: AsciiChar                      = 057;
    pub const ASCII_COLON: AsciiChar                     = 058;
    pub const ASCII_SEMICOLON: AsciiChar                 = 059;
    pub const ASCII_OPEN_ANGLED_BRACKET: AsciiChar       = 060;
    pub const ASCII_EQUALS: AsciiChar                    = 061;
    pub const ASCII_CLOSE_ANGLED_BRACKET: AsciiChar      = 062;
    pub const ASCII_QUESTION_MARK: AsciiChar             = 063;
    pub const ASCII_AT_SIGN: AsciiChar                   = 064;
    pub const ASCII_UPPERCASE_A: AsciiChar               = 065;
    pub const ASCII_UPPERCASE_B: AsciiChar               = 066;
    pub const ASCII_UPPERCASE_C: AsciiChar               = 067;
    pub const ASCII_UPPERCASE_D: AsciiChar               = 068;
    pub const ASCII_UPPERCASE_E: AsciiChar               = 069;
    pub const ASCII_UPPERCASE_F: AsciiChar               = 070;
    pub const ASCII_UPPERCASE_G: AsciiChar               = 071;
    pub const ASCII_UPPERCASE_H: AsciiChar               = 072;
    pub const ASCII_UPPERCASE_I: AsciiChar               = 073;
    pub const ASCII_UPPERCASE_J: AsciiChar               = 074;
    pub const ASCII_UPPERCASE_K: AsciiChar               = 075;
    pub const ASCII_UPPERCASE_L: AsciiChar               = 076;
    pub const ASCII_UPPERCASE_M: AsciiChar               = 077;
    pub const ASCII_UPPERCASE_N: AsciiChar               = 078;
    pub const ASCII_UPPERCASE_O: AsciiChar               = 079;
    pub const ASCII_UPPERCASE_P: AsciiChar               = 080;
    pub const ASCII_UPPERCASE_Q: AsciiChar               = 081;
    pub const ASCII_UPPERCASE_R: AsciiChar               = 082;
    pub const ASCII_UPPERCASE_S: AsciiChar               = 083;
    pub const ASCII_UPPERCASE_T: AsciiChar               = 084;
    pub const ASCII_UPPERCASE_U: AsciiChar               = 085;
    pub const ASCII_UPPERCASE_V: AsciiChar               = 086;
    pub const ASCII_UPPERCASE_W: AsciiChar               = 087;
    pub const ASCII_UPPERCASE_X: AsciiChar               = 088;
    pub const ASCII_UPPERCASE_Y: AsciiChar               = 089;
    pub const ASCII_UPPERCASE_Z: AsciiChar               = 090;
    pub const ASCII_OPENING_BRACKET: AsciiChar           = 091;
    pub const ASCII_BACKSLASH: AsciiChar                 = 092;
    pub const ASCII_CLOSING_BRACKET: AsciiChar           = 093;
    pub const ASCII_CARET: AsciiChar                     = 094;
    pub const ASCII_UNDERSCORE: AsciiChar                = 095;
    pub const ASCII_GRAVE_ACCENT: AsciiChar              = 096;
    pub const ASCII_LOWERCASE_A: AsciiChar               = 097;
    pub const ASCII_LOWERCASE_B: AsciiChar               = 098;
    pub const ASCII_LOWERCASE_C: AsciiChar               = 099;
    pub const ASCII_LOWERCASE_D: AsciiChar               = 100;
    pub const ASCII_LOWERCASE_E: AsciiChar               = 101;
    pub const ASCII_LOWERCASE_F: AsciiChar               = 102;
    pub const ASCII_LOWERCASE_G: AsciiChar               = 103;
    pub const ASCII_LOWERCASE_H: AsciiChar               = 104;
    pub const ASCII_LOWERCASE_I: AsciiChar               = 105;
    pub const ASCII_LOWERCASE_J: AsciiChar               = 106;
    pub const ASCII_LOWERCASE_K: AsciiChar               = 107;
    pub const ASCII_LOWERCASE_L: AsciiChar               = 108;
    pub const ASCII_LOWERCASE_M: AsciiChar               = 109;
    pub const ASCII_LOWERCASE_N: AsciiChar               = 110;
    pub const ASCII_LOWERCASE_O: AsciiChar               = 111;
    pub const ASCII_LOWERCASE_P: AsciiChar               = 112;
    pub const ASCII_LOWERCASE_Q: AsciiChar               = 113;
    pub const ASCII_LOWERCASE_R: AsciiChar               = 114;
    pub const ASCII_LOWERCASE_S: AsciiChar               = 115;
    pub const ASCII_LOWERCASE_T: AsciiChar               = 116;
    pub const ASCII_LOWERCASE_U: AsciiChar               = 117;
    pub const ASCII_LOWERCASE_V: AsciiChar               = 118;
    pub const ASCII_LOWERCASE_W: AsciiChar               = 119;
    pub const ASCII_LOWERCASE_X: AsciiChar               = 120;
    pub const ASCII_LOWERCASE_Y: AsciiChar               = 121;
    pub const ASCII_LOWERCASE_Z: AsciiChar               = 122;
    pub const ASCII_OPENING_BRACE: AsciiChar             = 123;
    pub const ASCII_VERTICAL_BAR: AsciiChar              = 124;
    pub const ASCII_CLOSING_BRACE: AsciiChar             = 125;
    pub const ASCII_TILDE: AsciiChar                     = 126;
    pub const ASCII_DELETE: AsciiChar                    = 127;
    pub const ASCII_EURO_SIGN: AsciiChar                 = 128;
    // unused: AsciiChar { 129 };
    pub const ASCII_SINGLE_LOW_9_QUOTATION_MARK: AsciiChar          = 130;
    pub const ASCII_LATIN_SMALL_LETTER_F_WITH_HOOK: AsciiChar       = 131;
    pub const ASCII_DOUBLE_LOW_9_QUOTATION_MARK: AsciiChar          = 132;
    pub const ASCII_HORIZONTAL_ELLIPSE: AsciiChar                   = 133;
    pub const ASCII_DAGGER: AsciiChar                               = 134;
    pub const ASCII_DOUBLE_DAGGER: AsciiChar                        = 135;
    pub const ASCII_MODIFIER_LETTER_CIRCUMFLEX_ACCENT: AsciiChar    = 136;
    pub const ASCII_PER_MILE_SIGN: AsciiChar                        = 137;
    pub const ASCII_LATIN_CAPITAL_S_WITH_CARON: AsciiChar           = 138;
    pub const ASCII_SINGLE_LEFT_POINTING_ANGLE_QUOTATION: AsciiChar = 139;
    pub const ASCII_LATIN_CAPITAL_LIGATURE_OE: AsciiChar            = 140;
    // unused: AsciiChar { 141 };
    pub const ASCII_LATIN_CAPITAL_LETTER_Z_WITH_CARON: AsciiChar    = 142;
    // unused: AsciiChar { 143 };
    // unused: AsciiChar { 144 };
    pub const ASCII_LEFT_SINGLE_QUOTATION_MARK: AsciiChar                   = 145;
    pub const ASCII_RIGHT_SINGLE_QUOTATION_MARK: AsciiChar                  = 146;
    pub const ASCII_LEFT_DOUBLE_QUOTATION_MARK: AsciiChar                   = 147;
    pub const ASCII_RIGHT_DOUBLE_QUOTATION_MARK: AsciiChar                  = 148;
    pub const ASCII_BULLET: AsciiChar                                       = 149;
    pub const ASCII_EN_DASH: AsciiChar                                      = 150;
    pub const ASCII_EM_DASH: AsciiChar                                      = 151;
    pub const ASCII_SMALL_TILDE: AsciiChar                                  = 152;
    pub const ASCII_TRADE_MARK_SIGN: AsciiChar                              = 153;
    pub const ASCII_LATIN_SMALL_LETTER_S_WITH_CARON: AsciiChar              = 154;
    pub const ASCII_SINGLE_RIGHT_POINTING_ANGLE_QUOTATION_MARK: AsciiChar   = 155;
    pub const ASCII_LATIN_SMALL_LIGATURE_OE: AsciiChar                      = 156;
    // unused: AsciiChar { 157 };
    pub const ASCII_LATIN_SMALL_LETTER_Z_WITH_CARON: AsciiChar          = 158;
    pub const ASCII_LATIN_CAPITAL_LETTER_Y_WITH_DIAERESIS: AsciiChar    = 159;
    pub const ASCII_NON_BREAKING_SPACE: AsciiChar                       = 160;
    pub const ASCII_INVERTED_EXCLAMATION_MARK: AsciiChar                = 161;
    pub const ASCII_CENT_SIGN: AsciiChar                                = 162;
    pub const ASCII_POUND_SIGN: AsciiChar                               = 163;
    pub const ASCII_CURRENCY_SIGN: AsciiChar                            = 164;
    pub const ASCII_YEN_SIGN: AsciiChar                                 = 165;
    pub const ASCII_PIPE_BROKEN_VERTICAL: AsciiChar                     = 166;
    pub const ASCII_SECTION_SIGN: AsciiChar                             = 167;
    pub const ASCII_SPACING_DIAERESIS_UMLAUT: AsciiChar                 = 168;
    pub const ASCII_COPYRIGHT_SIGN: AsciiChar                           = 169;
    pub const ASCII_FEMININE_ORDINAL_INDICATOR: AsciiChar               = 170;
    pub const ASCII_LEFT_DOUBLE_ANGLE_BRACKET: AsciiChar                = 171;
    pub const ASCII_NEGATION: AsciiChar                                 = 172;
    pub const ASCII_SOFT_HYPHEN: AsciiChar                              = 173;
    pub const ASCII_REGISTERED_TRADE_MARK_SIGN: AsciiChar               = 174;
    pub const ASCII_SPACING_MACRON_OVERLINE: AsciiChar                  = 175;
    pub const ASCII_DEGREE_SIGN: AsciiChar                              = 176;
    pub const ASCII_PLUS_OR_MINUS_SIGN: AsciiChar                       = 177;
    pub const ASCII_SUPERSCRIPT_TWO: AsciiChar                          = 178;
    pub const ASCII_SUPERSCRIPT_THREE: AsciiChar                        = 179;
    pub const ASCII_ACUTE_ACCENT: AsciiChar                             = 180;
    pub const ASCII_MICRO_SIGN: AsciiChar                               = 181;
    pub const ASCII_PILCROW_SIGN: AsciiChar                             = 182;
    pub const ASCII_MIDDLE_DOT: AsciiChar                               = 183;
    pub const ASCII_SPACING_CEDILLA: AsciiChar                          = 184;
    pub const ASCII_SUPERSCRIPT_1: AsciiChar                            = 185;
    pub const ASCII_MASCULINE_ORDINAL_INDICATOR: AsciiChar              = 186;
    pub const ASCII_RIGHT_DOUBLE_ANGLE_QUOTES: AsciiChar                = 187;
    pub const ASCII_FRACTION_ONE_QUARTER: AsciiChar                     = 188;
    pub const ASCII_FRACTION_ONE_HALF: AsciiChar                        = 189;
    pub const ASCII_FRACTION_THREE_QUARTERS: AsciiChar                  = 190;
    pub const ASCII_INVERTED_QUESTION_MARK: AsciiChar                   = 191;
    pub const ASCII_LATIN_CAPITAL_LETTER_A_WITH_GRAVE: AsciiChar        = 192;
    pub const ASCII_LATIN_CAPITAL_LETTER_A_WITH_ACUTE: AsciiChar        = 193;
    pub const ASCII_LATIN_CAPITAL_LETTER_A_WITH_CIRCUMFLEX: AsciiChar   = 194;
    pub const ASCII_LATIN_CAPITAL_LETTER_A_WITH_TILDE: AsciiChar        = 195;
    pub const ASCII_LATIN_CAPITAL_LETTER_A_WITH_DIAERESIS: AsciiChar    = 196;
    pub const ASCII_LATIN_CAPITAL_LETTER_A_WITH_RING_ABOVE: AsciiChar   = 197;
    pub const ASCII_LATIN_CAPITAL_LETTER_AE: AsciiChar                  = 198;
    pub const ASCII_LATIN_CAPITAL_LETTER_C_WITH_CEDILLA: AsciiChar      = 199;
    pub const ASCII_LATIN_CAPITAL_LETTER_E_WITH_GRAVE: AsciiChar        = 200;
    pub const ASCII_LATIN_CAPITAL_LETTER_E_WITH_ACUTE: AsciiChar        = 201;
    pub const ASCII_LATIN_CAPITAL_LETTER_E_WITH_CIRCUMFLEX: AsciiChar   = 202;
    pub const ASCII_LATIN_CAPITAL_LETTER_E_WITH_DIAERESIS: AsciiChar    = 203;
    pub const ASCII_LATIN_CAPITAL_LETTER_I_WITH_GRAVE: AsciiChar        = 204;
    pub const ASCII_LATIN_CAPITAL_LETTER_I_WITH_ACUTE: AsciiChar        = 205;
    pub const ASCII_LATIN_CAPITAL_LETTER_I_WITH_CIRCUMFLEX: AsciiChar   = 206;
    pub const ASCII_LATIN_CAPITAL_LETTER_I_WITH_DIAERESIS: AsciiChar    = 207;
    pub const ASCII_LATIN_CAPITAL_LETTER_ETH: AsciiChar                 = 208;
    pub const ASCII_LATIN_CAPITAL_LETTER_N_WITH_TILDE: AsciiChar        = 209;
    pub const ASCII_LATIN_CAPITAL_LETTER_O_WITH_GRAVE: AsciiChar        = 210;
    pub const ASCII_LATIN_CAPITAL_LETTER_O_WITH_ACUTE: AsciiChar        = 211;
    pub const ASCII_LATIN_CAPITAL_LETTER_O_WITH_CIRCUMFLEX: AsciiChar   = 212;
    pub const ASCII_LATIN_CAPITAL_LETTER_O_WITH_TILDE: AsciiChar        = 213;
    pub const ASCII_LATIN_CAPITAL_LETTER_O_WITH_DIAERESIS: AsciiChar    = 214;
    pub const ASCII_MULTIPLICATION_SIGN: AsciiChar                      = 215;
    pub const ASCII_LATIN_CAPITAL_LETTER_O_WITH_SLASH: AsciiChar        = 216;
    pub const ASCII_LATIN_CAPITAL_LETTER_U_WITH_GRAVE: AsciiChar        = 217;
    pub const ASCII_LATIN_CAPITAL_LETTER_ACUTE: AsciiChar               = 218;
    pub const ASCII_LATIN_CAPITAL_LETTER_CIRCUMFLEX: AsciiChar          = 219;
    pub const ASCII_LATIN_CAPITAL_LETTER_DIAERESIS: AsciiChar           = 220;
    pub const ASCII_LATIN_CAPITAL_LETTER_Y_WITH_ACUTE: AsciiChar        = 221;
    pub const ASCII_LATIN_CAPITAL_LETTER_THORN: AsciiChar               = 222;
    pub const ASCII_LATIN_SMALL_LETTER_SHARP_S: AsciiChar               = 223;
    pub const ASCII_LATIN_SMALL_LETTER_A_WITH_GRAVE: AsciiChar          = 224;
    pub const ASCII_LATIN_SMALL_LETTER_A_WITH_ACUTE: AsciiChar          = 225;
    pub const ASCII_LATIN_SMALL_LETTER_A_WITH_CIRCUMFLEX: AsciiChar     = 226;
    pub const ASCII_LATIN_SMALL_LETTER_A_WITH_TILDE: AsciiChar          = 227;
    pub const ASCII_LATIN_SMALL_LETTER_A_WITH_DIAERESIS: AsciiChar      = 228;
    pub const ASCII_LATIN_SMALL_LETTER_A_WITH_RING_ABOVE: AsciiChar     = 229;
    pub const ASCII_LATIN_SMALL_LETTER_AE: AsciiChar                    = 230;
    pub const ASCII_LATIN_SMALL_LETTER_C_WITH_CEDILLA: AsciiChar        = 231;
    pub const ASCII_LATIN_SMALL_LETTER_E_WITH_GRAVE: AsciiChar          = 232;
    pub const ASCII_LATIN_SMALL_LETTER_E_WITH_ACUTE: AsciiChar          = 233;
    pub const ASCII_LATIN_SMALL_LETTER_E_WITH_CIRCUMFLEX: AsciiChar     = 234;
    pub const ASCII_LATIN_SMALL_LETTER_E_WITH_DIAERESIS: AsciiChar      = 235;
    pub const ASCII_LATIN_SMALL_LETTER_I_WITH_GRAVE: AsciiChar          = 236;
    pub const ASCII_LATIN_SMALL_LETTER_I_WITH_ACUTE: AsciiChar          = 237;
    pub const ASCII_LATIN_SMALL_LETTER_I_WITH_CIRCUMFLEX: AsciiChar     = 238;
    pub const ASCII_LATIN_SMALL_LETTER_I_WITH_DIAERESIS: AsciiChar      = 239;
    pub const ASCII_LATIN_SMALL_LETTER_ETH: AsciiChar                   = 240;
    pub const ASCII_LATIN_SMALL_LETTER_N_WITH_TILDE: AsciiChar          = 241;
    pub const ASCII_LATIN_SMALL_LETTER_O_WITH_GRAVE: AsciiChar          = 242;
    pub const ASCII_LATIN_SMALL_LETTER_O_WITH_ACUTE: AsciiChar          = 243;
    pub const ASCII_LATIN_SMALL_LETTER_O_WITH_CIRCUMFLEX: AsciiChar     = 244;
    pub const ASCII_LATIN_SMALL_LETTER_O_WITH_TILDE: AsciiChar          = 245;
    pub const ASCII_LATIN_SMALL_LETTER_O_WITH_DIAERESIS: AsciiChar      = 246;
    pub const ASCII_DIVISION_SIGN: AsciiChar                            = 247;
    pub const ASCII_LATIN_SMALL_LETTER_O_WITH_SLASH: AsciiChar          = 248;
    pub const ASCII_LATIN_SMALL_LETTER_U_WITH_GRAVE: AsciiChar          = 249;
    pub const ASCII_LATIN_SMALL_LETTER_U_WITH_ACUTE: AsciiChar          = 250;
    pub const ASCII_LATIN_SMALL_LETTER_U_WITH_CIRCUMFLEX: AsciiChar     = 251;
    pub const ASCII_LATIN_SMALL_LETTER_U_WITH_DIAERESIS: AsciiChar      = 252;
    pub const ASCII_LATIN_SMALL_LETTER_Y_WITH_ACUTE: AsciiChar          = 253;
    pub const ASCII_LATIN_SMALL_LETTER_THORN: AsciiChar                 = 254;
    pub const ASCII_LATIN_SMALL_LETTER_Y_WITH_DIAERESIS: AsciiChar      = 255;

    pub const EMPTY_ASCII_STRING: AsciiString = AsciiString { string: vec![] };
}

#[inline]
pub fn is_numeric(character: AsciiChar) -> bool {
    match character {
        ASCII_ZERO..=ASCII_NINE => true,
        _ => false,
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct AsciiString {
    string: Vec<AsciiChar>,
}

impl Display for AsciiString {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for &character in &self.string {
            write!(f, "{}", character as char)?;
        }
        return Ok(());
    }
}

impl Debug for AsciiString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AsciiString(\"{}\")", self)
    }
}

impl AsciiString {
    #[inline]
    pub fn from(string: &[u8]) -> Self {
        Self { string: string.to_vec() }
    }

    #[inline]
    pub fn from_utf8(string: &str) -> Result<Self, AsciiError> {
        let mut ascii_characters = Vec::with_capacity(string.len());
        for character in string.chars() {
            let u32_character = character as u32;
            if ((u8::MIN as u32) > u32_character) || ((u8::MAX as u32) < u32_character) {
                return Err(AsciiError::BadChar);
            } else {
                ascii_characters.push(character as u8);
            }
        }
        return Ok(Self {string: ascii_characters});
    }

    #[inline]
    pub fn from_range(&self, start: usize, end: usize) -> Self {
        Self { string: self.string[start..end].to_vec() }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.string.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.string.is_empty()
    }

    #[inline]
    pub fn get(&self, index: usize) -> Option<&AsciiChar> {
        self.string.get(index)
    }

    #[inline]
    pub fn last(&self) -> Option<&AsciiChar> {
        self.string.last()
    }

    #[inline]
    pub fn last_mut(&mut self) -> Option<&mut AsciiChar> {
        self.string.last_mut()
    }

    #[inline]
    pub fn first(&self) -> Option<&AsciiChar> {
        self.string.first()
    }

    #[inline]
    pub fn first_mut(&mut self) -> Option<&mut AsciiChar> {
        self.string.first_mut()
    }

    #[inline]
    pub fn push(&mut self, character: AsciiChar) {
        self.string.push(character);
    }

    #[inline]
    pub fn pop(&mut self) -> Option<AsciiChar> {
        self.string.pop()
    }

    #[inline]
    pub fn insert(&mut self, index: usize, character: AsciiChar) {
        self.string.insert(index, character);
    }

    #[inline]
    pub fn remove(&mut self, index: usize) -> AsciiChar {
        self.string.remove(index)
    }

    #[inline]
    pub fn clear(&mut self) {
        self.string.clear();
    }

    #[inline]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut AsciiChar> {
        self.string.get_mut(index)
    }

    #[inline]
    pub fn swap(&mut self, index1: usize, index2: usize) {
        self.string.swap(index1, index2)
    }

    #[inline]
    pub fn reverse(&mut self) {
        self.string.reverse();
    }

    #[inline]
    pub fn as_reversed(&self) -> Rev<Iter<'_, AsciiChar>> {
        self.iter().rev()
    }

    #[inline]
    pub fn iter(&self) -> Iter<'_, AsciiChar> {
        self.string.iter()
    }

    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_, AsciiChar> {
        self.string.iter_mut()
    }

    #[inline]
    pub fn strip_suffix(&self, suffix: &Self) -> Option<Self> {
        let suffix = suffix.string.as_slice();
        let (len, n) = (self.len(), suffix.len());
        if n <= len {
            let (head, tail) = self.string.split_at(len - n);
            if tail == suffix {
                return Some(Self { string: head.to_vec() });
            }
        }
        None
    }

    #[inline]
    pub fn strip_prefix(&self, prefix: &Self) -> Option<Self> {
        let prefix = prefix.string.as_slice();
        let n = prefix.len();
        if n <= self.len() {
            let (head, tail) = self.string.split_at(n);
            if head == prefix {
                return Some(Self { string: tail.to_vec() });
            }
        }
        None
    }

    #[inline]
    pub fn contains(&self, character: &AsciiChar) -> bool {
        self.string.contains(character)
    }

    #[inline]
    pub fn as_slice(&self) -> &[AsciiChar] {
        self.string.as_slice()
    }

    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [AsciiChar] {
        self.string.as_mut_slice()
    }

    #[inline]
    pub fn as_vec(&self) -> &Vec<AsciiChar> {
        &self.string
    }

    #[inline]
    pub fn as_lower(&self) -> Self {
        Self {
            string: self.string.iter()
                .map(|character| ascii_char_as_lower(*character))
                .collect()
        }
    }

    #[inline]
    pub fn lower(&mut self) {
        for character in self.string.iter_mut() {
            *character = ascii_char_as_lower(*character)
        }
    }

    #[inline]
    pub fn as_upper(&self) -> Self {
        Self {
            string: self.string.iter()
                .map(|character| ascii_char_as_upper(*character))
                .collect()
        }
    }

    #[inline]
    pub fn upper(&mut self) {
        for character in self.string.iter_mut() {
            *character = ascii_char_as_upper(*character)
        }
    }

    #[inline]
    pub fn is_numeric(&self) -> bool {
        self.string.len() != 0
        && self.string.iter().all(|character| match character {
            ASCII_ZERO..=ASCII_NINE => true,
            _ => false,
        })
    }

    #[inline]
    pub fn is_alphanumeric(&self) -> bool {
        self.string.len() != 0 
        && self.string.iter().all(|character| match character {
            ASCII_ZERO..=ASCII_NINE => true,
            ASCII_UPPERCASE_A..=ASCII_UPPERCASE_Z => true,
            ASCII_LOWERCASE_A..=ASCII_LOWERCASE_Z => true,
            _ => false,
        })
    }

    #[inline]
    pub fn is_lower_alphanumeric(&self) -> bool {
        self.string.len() != 0
        && self.string.iter().all(|character| match character {
            ASCII_ZERO..=ASCII_NINE => true,
            ASCII_LOWERCASE_A..=ASCII_LOWERCASE_Z => true,
            _ => false,
        })
    }

    #[inline]
    pub fn is_upper_alphanumeric(&self) -> bool {
        self.string.len() != 0
        && self.string.iter().all(|character| match character {
            ASCII_ZERO..=ASCII_NINE => true,
            ASCII_UPPERCASE_A..=ASCII_UPPERCASE_Z => true,
            _ => false,
        })
    }
}

impl Add for AsciiString {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        let mut string = self.string.clone();
        string.extend(rhs.string);

        Self { string: string, }
    }
}

#[cfg(test)]
mod upper_lower_case_tests {
    use super::AsciiString;

    const MIXED_CASE_STRING: &str = "This Is A Test aAbBcCdDeEfFgGhHiIjJkKlLmMnNoOpPqQrRsStTuUvVwWxXyYzZ 0123456789 !@#$%^&*()";
    const UPPER_STRING: &str = "THIS IS A TEST AABBCCDDEEFFGGHHIIJJKKLLMMNNOOPPQQRRSSTTUUVVWWXXYYZZ 0123456789 !@#$%^&*()";
    const LOWER_STRING: &str = "this is a test aabbccddeeffgghhiijjkkllmmnnooppqqrrssttuuvvwwxxyyzz 0123456789 !@#$%^&*()";

    #[test]
    fn test_as_upper_success() {
        // Setup
        let ascii_mixed_case = AsciiString::from_utf8(MIXED_CASE_STRING).expect("The mixed case string could not convert from utf8 to ascii");
        let ascii_upper_case = AsciiString::from_utf8(UPPER_STRING).expect("The upper case string could not convert from utf8 to ascii");
        let ascii_lower_case = AsciiString::from_utf8(LOWER_STRING).expect("The lower case string could not convert from utf8 to ascii");

        let expected_string = AsciiString::from_utf8(UPPER_STRING).expect("The upper case string could not convert from utf8 to ascii");

        // Test & Check
        assert_eq!(expected_string, ascii_mixed_case.as_upper());
        assert_eq!(expected_string, ascii_upper_case.as_upper());
        assert_eq!(expected_string, ascii_lower_case.as_upper());
    }

    #[test]
    fn test_upper_success() {
        // Setup
        let mut ascii_mixed_case = AsciiString::from_utf8(MIXED_CASE_STRING).expect("The mixed case string could not convert from utf8 to ascii");
        let mut ascii_upper_case = AsciiString::from_utf8(UPPER_STRING).expect("The upper case string could not convert from utf8 to ascii");
        let mut ascii_lower_case = AsciiString::from_utf8(LOWER_STRING).expect("The lower case string could not convert from utf8 to ascii");

        let expected_string = AsciiString::from_utf8(UPPER_STRING).expect("The upper case string could not convert from utf8 to ascii");

        // Test & Check
        ascii_mixed_case.upper();
        ascii_upper_case.upper();
        ascii_lower_case.upper();

        assert_eq!(expected_string, ascii_mixed_case);
        assert_eq!(expected_string, ascii_upper_case);
        assert_eq!(expected_string, ascii_lower_case);
    }

    #[test]
    fn test_as_lower_success() {
        // Setup
        let ascii_mixed_case = AsciiString::from_utf8(MIXED_CASE_STRING).expect("The mixed case string could not convert from utf8 to ascii");
        let ascii_upper_case = AsciiString::from_utf8(UPPER_STRING).expect("The upper case string could not convert from utf8 to ascii");
        let ascii_lower_case = AsciiString::from_utf8(LOWER_STRING).expect("The lower case string could not convert from utf8 to ascii");

        let expected_string = AsciiString::from_utf8(LOWER_STRING).expect("The upper case string could not convert from utf8 to ascii");

        // Test & Check
        assert_eq!(expected_string, ascii_mixed_case.as_lower());
        assert_eq!(expected_string, ascii_upper_case.as_lower());
        assert_eq!(expected_string, ascii_lower_case.as_lower());
    }

    #[test]
    fn test_lower_success() {
        // Setup
        let mut ascii_mixed_case = AsciiString::from_utf8(MIXED_CASE_STRING).expect("The mixed case string could not convert from utf8 to ascii");
        let mut ascii_upper_case = AsciiString::from_utf8(UPPER_STRING).expect("The upper case string could not convert from utf8 to ascii");
        let mut ascii_lower_case = AsciiString::from_utf8(LOWER_STRING).expect("The lower case string could not convert from utf8 to ascii");

        let expected_string = AsciiString::from_utf8(LOWER_STRING).expect("The upper case string could not convert from utf8 to ascii");

        // Test & Check
        ascii_mixed_case.lower();
        ascii_upper_case.lower();
        ascii_lower_case.lower();

        assert_eq!(expected_string, ascii_mixed_case);
        assert_eq!(expected_string, ascii_upper_case);
        assert_eq!(expected_string, ascii_lower_case);
    }

    const MIXED_CASE_ALPHA: &str = "aAbBcCdDeEfFgGhHiIjJkKlLmMnNoOpPqQrRsStTuUvVwWxXyYzZ";
    const UPPER_STRING_ALPHA: &str = "AABBCCDDEEFFGGHHIIJJKKLLMMNNOOPPQQRRSSTTUUVVWWXXYYZZ";
    const LOWER_STRING_ALPHA: &str = "aabbccddeeffgghhiijjkkllmmnnooppqqrrssttuuvvwwxxyyzz";

    const NUMERIC: &str = "0123456789";

    const MIXED_CASE_ALPHA_NUMERIC: &str = "aAbBcCdDeEfFgGhHiIjJkKlLmMnNoOpPqQrRsStTuUvVwWxXyYzZ0123456789";
    const UPPER_STRING_ALPHA_NUMERIC: &str = "AABBCCDDEEFFGGHHIIJJKKLLMMNNOOPPQQRRSSTTUUVVWWXXYYZZ0123456789";
    const LOWER_STRING_ALPHA_NUMERIC: &str = "aabbccddeeffgghhiijjkkllmmnnooppqqrrssttuuvvwwxxyyzz0123456789";

    const MIXED_CASE_NON_ALPHA_NUMERIC: &str = "aAbBcCdDeEfFgGhHiIjJkKlLmMnNoOpPqQrRsStTuUvVwWxXyYzZ0123456789!@#$%^&*()";
    const UPPER_STRING_NON_ALPHA_NUMERIC: &str = "AABBCCDDEEFFGGHHIIJJKKLLMMNNOOPPQQRRSSTTUUVVWWXXYYZZ0123456789!@#$%^&*()";
    const LOWER_STRING_NON_ALPHA_NUMERIC: &str = "aabbccddeeffgghhiijjkkllmmnnooppqqrrssttuuvvwwxxyyzz0123456789!@#$%^&*()";

    #[test]
    fn test_is_alpha_numeric_lower() {
        // Setup
        let ascii_mixed_case_alpha = AsciiString::from_utf8(MIXED_CASE_ALPHA).expect("The mixed case string could not convert from utf8 to ascii");
        let ascii_upper_case_alpha = AsciiString::from_utf8(UPPER_STRING_ALPHA).expect("The upper case string could not convert from utf8 to ascii");
        let ascii_lower_case_alpha = AsciiString::from_utf8(LOWER_STRING_ALPHA).expect("The lower case string could not convert from utf8 to ascii");

        let ascii_numeric = AsciiString::from_utf8(NUMERIC).expect("The numeric string could not convert from utf8 to ascii");

        let ascii_mixed_case_alphanumeric = AsciiString::from_utf8(MIXED_CASE_ALPHA_NUMERIC).expect("The mixed case string could not convert from utf8 to ascii");
        let ascii_upper_case_alphanumeric = AsciiString::from_utf8(UPPER_STRING_ALPHA_NUMERIC).expect("The upper case string could not convert from utf8 to ascii");
        let ascii_lower_case_alphanumeric = AsciiString::from_utf8(LOWER_STRING_ALPHA_NUMERIC).expect("The lower case string could not convert from utf8 to ascii");

        let ascii_mixed_case_non_alphanumeric = AsciiString::from_utf8(MIXED_CASE_NON_ALPHA_NUMERIC).expect("The mixed case string could not convert from utf8 to ascii");
        let ascii_upper_case_non_alphanumeric = AsciiString::from_utf8(UPPER_STRING_NON_ALPHA_NUMERIC).expect("The upper case string could not convert from utf8 to ascii");
        let ascii_lower_case_non_alphanumeric = AsciiString::from_utf8(LOWER_STRING_NON_ALPHA_NUMERIC).expect("The lower case string could not convert from utf8 to ascii");

        // Test & Check
        assert!(!ascii_mixed_case_alpha.is_lower_alphanumeric());
        assert!(!ascii_upper_case_alpha.is_lower_alphanumeric());
        assert!(ascii_lower_case_alpha.is_lower_alphanumeric());

        assert!(ascii_numeric.is_lower_alphanumeric());

        assert!(!ascii_mixed_case_alphanumeric.is_lower_alphanumeric());
        assert!(!ascii_upper_case_alphanumeric.is_lower_alphanumeric());
        assert!(ascii_lower_case_alphanumeric.is_lower_alphanumeric());

        assert!(!ascii_mixed_case_non_alphanumeric.is_lower_alphanumeric());
        assert!(!ascii_upper_case_non_alphanumeric.is_lower_alphanumeric());
        assert!(!ascii_lower_case_non_alphanumeric.is_lower_alphanumeric());
    }

    #[test]
    fn test_is_alpha_numeric_upper() {
        // Setup
        let ascii_mixed_case_alpha = AsciiString::from_utf8(MIXED_CASE_ALPHA).expect("The mixed case string could not convert from utf8 to ascii");
        let ascii_upper_case_alpha = AsciiString::from_utf8(UPPER_STRING_ALPHA).expect("The upper case string could not convert from utf8 to ascii");
        let ascii_lower_case_alpha = AsciiString::from_utf8(LOWER_STRING_ALPHA).expect("The lower case string could not convert from utf8 to ascii");

        let ascii_numeric = AsciiString::from_utf8(NUMERIC).expect("The numeric string could not convert from utf8 to ascii");

        let ascii_mixed_case_alphanumeric = AsciiString::from_utf8(MIXED_CASE_ALPHA_NUMERIC).expect("The mixed case string could not convert from utf8 to ascii");
        let ascii_upper_case_alphanumeric = AsciiString::from_utf8(UPPER_STRING_ALPHA_NUMERIC).expect("The upper case string could not convert from utf8 to ascii");
        let ascii_lower_case_alphanumeric = AsciiString::from_utf8(LOWER_STRING_ALPHA_NUMERIC).expect("The lower case string could not convert from utf8 to ascii");

        let ascii_mixed_case_non_alphanumeric = AsciiString::from_utf8(MIXED_CASE_NON_ALPHA_NUMERIC).expect("The mixed case string could not convert from utf8 to ascii");
        let ascii_upper_case_non_alphanumeric = AsciiString::from_utf8(UPPER_STRING_NON_ALPHA_NUMERIC).expect("The upper case string could not convert from utf8 to ascii");
        let ascii_lower_case_non_alphanumeric = AsciiString::from_utf8(LOWER_STRING_NON_ALPHA_NUMERIC).expect("The lower case string could not convert from utf8 to ascii");

        // Test & Check
        assert!(!ascii_mixed_case_alpha.is_upper_alphanumeric());
        assert!(ascii_upper_case_alpha.is_upper_alphanumeric());
        assert!(!ascii_lower_case_alpha.is_upper_alphanumeric());

        assert!(ascii_numeric.is_upper_alphanumeric());

        assert!(!ascii_mixed_case_alphanumeric.is_upper_alphanumeric());
        assert!(ascii_upper_case_alphanumeric.is_upper_alphanumeric());
        assert!(!ascii_lower_case_alphanumeric.is_upper_alphanumeric());

        assert!(!ascii_mixed_case_non_alphanumeric.is_upper_alphanumeric());
        assert!(!ascii_upper_case_non_alphanumeric.is_upper_alphanumeric());
        assert!(!ascii_lower_case_non_alphanumeric.is_upper_alphanumeric());
    }

    #[test]
    fn test_is_alpha_numeric() {
        // Setup
        let ascii_mixed_case_alpha = AsciiString::from_utf8(MIXED_CASE_ALPHA).expect("The mixed case string could not convert from utf8 to ascii");
        let ascii_upper_case_alpha = AsciiString::from_utf8(UPPER_STRING_ALPHA).expect("The upper case string could not convert from utf8 to ascii");
        let ascii_lower_case_alpha = AsciiString::from_utf8(LOWER_STRING_ALPHA).expect("The lower case string could not convert from utf8 to ascii");

        let ascii_numeric = AsciiString::from_utf8(NUMERIC).expect("The numeric string could not convert from utf8 to ascii");

        let ascii_mixed_case_alphanumeric = AsciiString::from_utf8(MIXED_CASE_ALPHA_NUMERIC).expect("The mixed case string could not convert from utf8 to ascii");
        let ascii_upper_case_alphanumeric = AsciiString::from_utf8(UPPER_STRING_ALPHA_NUMERIC).expect("The upper case string could not convert from utf8 to ascii");
        let ascii_lower_case_alphanumeric = AsciiString::from_utf8(LOWER_STRING_ALPHA_NUMERIC).expect("The lower case string could not convert from utf8 to ascii");

        let ascii_mixed_case_non_alphanumeric = AsciiString::from_utf8(MIXED_CASE_NON_ALPHA_NUMERIC).expect("The mixed case string could not convert from utf8 to ascii");
        let ascii_upper_case_non_alphanumeric = AsciiString::from_utf8(UPPER_STRING_NON_ALPHA_NUMERIC).expect("The upper case string could not convert from utf8 to ascii");
        let ascii_lower_case_non_alphanumeric = AsciiString::from_utf8(LOWER_STRING_NON_ALPHA_NUMERIC).expect("The lower case string could not convert from utf8 to ascii");

        // Test & Check
        assert!(ascii_mixed_case_alpha.is_alphanumeric());
        assert!(ascii_upper_case_alpha.is_alphanumeric());
        assert!(ascii_lower_case_alpha.is_alphanumeric());

        assert!(ascii_numeric.is_alphanumeric());

        assert!(ascii_mixed_case_alphanumeric.is_alphanumeric());
        assert!(ascii_upper_case_alphanumeric.is_alphanumeric());
        assert!(ascii_lower_case_alphanumeric.is_alphanumeric());

        assert!(!ascii_mixed_case_non_alphanumeric.is_alphanumeric());
        assert!(!ascii_upper_case_non_alphanumeric.is_alphanumeric());
        assert!(!ascii_lower_case_non_alphanumeric.is_alphanumeric());
    }

    #[test]
    fn test_is_numeric() {
        // Setup
        let ascii_mixed_case_alpha = AsciiString::from_utf8(MIXED_CASE_ALPHA).expect("The mixed case string could not convert from utf8 to ascii");
        let ascii_upper_case_alpha = AsciiString::from_utf8(UPPER_STRING_ALPHA).expect("The upper case string could not convert from utf8 to ascii");
        let ascii_lower_case_alpha = AsciiString::from_utf8(LOWER_STRING_ALPHA).expect("The lower case string could not convert from utf8 to ascii");

        let ascii_numeric = AsciiString::from_utf8(NUMERIC).expect("The numeric string could not convert from utf8 to ascii");

        let ascii_mixed_case_alphanumeric = AsciiString::from_utf8(MIXED_CASE_ALPHA_NUMERIC).expect("The mixed case string could not convert from utf8 to ascii");
        let ascii_upper_case_alphanumeric = AsciiString::from_utf8(UPPER_STRING_ALPHA_NUMERIC).expect("The upper case string could not convert from utf8 to ascii");
        let ascii_lower_case_alphanumeric = AsciiString::from_utf8(LOWER_STRING_ALPHA_NUMERIC).expect("The lower case string could not convert from utf8 to ascii");

        let ascii_mixed_case_non_alphanumeric = AsciiString::from_utf8(MIXED_CASE_NON_ALPHA_NUMERIC).expect("The mixed case string could not convert from utf8 to ascii");
        let ascii_upper_case_non_alphanumeric = AsciiString::from_utf8(UPPER_STRING_NON_ALPHA_NUMERIC).expect("The upper case string could not convert from utf8 to ascii");
        let ascii_lower_case_non_alphanumeric = AsciiString::from_utf8(LOWER_STRING_NON_ALPHA_NUMERIC).expect("The lower case string could not convert from utf8 to ascii");

        // Test & Check
        assert!(!ascii_mixed_case_alpha.is_numeric());
        assert!(!ascii_upper_case_alpha.is_numeric());
        assert!(!ascii_lower_case_alpha.is_numeric());

        assert!(ascii_numeric.is_numeric());

        assert!(!ascii_mixed_case_alphanumeric.is_numeric());
        assert!(!ascii_upper_case_alphanumeric.is_numeric());
        assert!(!ascii_lower_case_alphanumeric.is_numeric());

        assert!(!ascii_mixed_case_non_alphanumeric.is_numeric());
        assert!(!ascii_upper_case_non_alphanumeric.is_numeric());
        assert!(!ascii_lower_case_non_alphanumeric.is_numeric());
    }
}
