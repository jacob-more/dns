use std::fmt::{Debug, Display};

use crate::types::ascii::{AsciiChar, constants::ASCII_ZERO};

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum EscapableChar {
    Ascii(AsciiChar),
    EscapedAscii(AsciiChar),
    EscapedOctal(AsciiChar)
}

impl EscapableChar {
    pub fn into_unescaped_character(self) -> AsciiChar {
        match self {
            EscapableChar::Ascii(character) => character,
            EscapableChar::EscapedAscii(character) => character,
            EscapableChar::EscapedOctal(character) => character,
        }
    }
}

impl Display for EscapableChar {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ascii(character) => write!(f, "{}", *character as char),
            Self::EscapedAscii(escaped_character) => write!(f, "\\{}", *escaped_character as char),
            Self::EscapedOctal(escaped_octal) => {
                let (char1, char2, char3) = ascii_to_octal(*escaped_octal);
                write!(f, "\\{}{}{}", char1 as char, char2 as char, char3 as char)
            },
        }
    }
}

impl Debug for EscapableChar {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ascii(character) => write!(f, "EscapableChar::Ascii({character} '{self}')"),
            Self::EscapedAscii(character) => write!(f, "EscapableChar::EscapedAscii({character} '{self}')"),
            Self::EscapedOctal(character) => write!(f, "EscapableChar::EscapedOctal({character} '{self}')"),
        }
    }
}

#[inline]
pub const fn ascii_to_octal(character: AsciiChar) -> (AsciiChar, AsciiChar, AsciiChar) {
    (
        ((character >> 6) & 0b00000011) + ASCII_ZERO,
        ((character >> 3) & 0b00000111) + ASCII_ZERO,
        ((character >> 0) & 0b00000111) + ASCII_ZERO
    )
}

#[inline]
pub const fn octal_to_ascii(char1: AsciiChar, char2: AsciiChar, char3: AsciiChar) -> AsciiChar {
    (((char1 - ASCII_ZERO) << 6) & 0b11000000 )
  + (((char2 - ASCII_ZERO) << 3) & 0b00111000 )
  + (((char3 - ASCII_ZERO) << 0) & 0b00000111 )
}

#[cfg(test)]
mod non_escaped_to_escaped_tests {
    use crate::serde::presentation::parse_chars::char_token::{ascii_to_octal, octal_to_ascii};

    use super::EscapableChar;

    fn test_escapable_octal_to_display_mapping(character: EscapableChar, display: &str, digits: (char, char, char)) {
        // Check for incorrect input.
        assert_eq!(display, format!("\\{}{}{}", digits.0, digits.1, digits.2).as_str(), "The expected display ({display}) did not match the provided digits {digits:?}");

        let byte_digits = (digits.0 as u8, digits.1 as u8, digits.2 as u8);

        // Test octal ascii -> octal
        let octal_characters = ascii_to_octal(character.into_unescaped_character());
        assert_eq!(octal_characters, byte_digits, "The conversion ascii_to_octal() did not result in the same octal code as the input");

        // Test octal octal -> ascii
        let ascii_character = octal_to_ascii(byte_digits.0, byte_digits.1, byte_digits.2);
        assert_eq!(ascii_character, character.into_unescaped_character(), "The conversion octal_to_ascii() did not result in the same ascii code as the input");

        // Test escapable character -> display
        let character_as_string = character.to_string();
        assert_eq!(character_as_string.as_str(), display, "The escaped character was expected to be displayed as \"{display}\" but was \"{character_as_string}\"");
    }

    #[test]
    fn test_all_escapable_octal_to_octal() {
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(000), r"\000", ('0','0','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(001), r"\001", ('0','0','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(002), r"\002", ('0','0','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(003), r"\003", ('0','0','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(004), r"\004", ('0','0','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(005), r"\005", ('0','0','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(006), r"\006", ('0','0','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(007), r"\007", ('0','0','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(008), r"\010", ('0','1','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(009), r"\011", ('0','1','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(010), r"\012", ('0','1','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(011), r"\013", ('0','1','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(012), r"\014", ('0','1','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(013), r"\015", ('0','1','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(014), r"\016", ('0','1','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(015), r"\017", ('0','1','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(016), r"\020", ('0','2','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(017), r"\021", ('0','2','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(018), r"\022", ('0','2','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(019), r"\023", ('0','2','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(020), r"\024", ('0','2','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(021), r"\025", ('0','2','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(022), r"\026", ('0','2','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(023), r"\027", ('0','2','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(024), r"\030", ('0','3','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(025), r"\031", ('0','3','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(026), r"\032", ('0','3','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(027), r"\033", ('0','3','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(028), r"\034", ('0','3','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(029), r"\035", ('0','3','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(030), r"\036", ('0','3','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(031), r"\037", ('0','3','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(032), r"\040", ('0','4','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(033), r"\041", ('0','4','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(034), r"\042", ('0','4','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(035), r"\043", ('0','4','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(036), r"\044", ('0','4','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(037), r"\045", ('0','4','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(038), r"\046", ('0','4','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(039), r"\047", ('0','4','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(040), r"\050", ('0','5','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(041), r"\051", ('0','5','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(042), r"\052", ('0','5','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(043), r"\053", ('0','5','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(044), r"\054", ('0','5','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(045), r"\055", ('0','5','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(046), r"\056", ('0','5','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(047), r"\057", ('0','5','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(048), r"\060", ('0','6','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(049), r"\061", ('0','6','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(050), r"\062", ('0','6','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(051), r"\063", ('0','6','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(052), r"\064", ('0','6','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(053), r"\065", ('0','6','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(054), r"\066", ('0','6','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(055), r"\067", ('0','6','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(056), r"\070", ('0','7','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(057), r"\071", ('0','7','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(058), r"\072", ('0','7','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(059), r"\073", ('0','7','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(060), r"\074", ('0','7','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(061), r"\075", ('0','7','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(062), r"\076", ('0','7','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(063), r"\077", ('0','7','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(064), r"\100", ('1','0','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(065), r"\101", ('1','0','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(066), r"\102", ('1','0','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(067), r"\103", ('1','0','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(068), r"\104", ('1','0','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(069), r"\105", ('1','0','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(070), r"\106", ('1','0','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(071), r"\107", ('1','0','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(072), r"\110", ('1','1','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(073), r"\111", ('1','1','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(074), r"\112", ('1','1','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(075), r"\113", ('1','1','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(076), r"\114", ('1','1','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(077), r"\115", ('1','1','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(078), r"\116", ('1','1','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(079), r"\117", ('1','1','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(080), r"\120", ('1','2','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(081), r"\121", ('1','2','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(082), r"\122", ('1','2','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(083), r"\123", ('1','2','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(084), r"\124", ('1','2','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(085), r"\125", ('1','2','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(086), r"\126", ('1','2','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(087), r"\127", ('1','2','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(088), r"\130", ('1','3','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(089), r"\131", ('1','3','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(090), r"\132", ('1','3','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(091), r"\133", ('1','3','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(092), r"\134", ('1','3','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(093), r"\135", ('1','3','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(094), r"\136", ('1','3','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(095), r"\137", ('1','3','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(096), r"\140", ('1','4','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(097), r"\141", ('1','4','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(098), r"\142", ('1','4','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(099), r"\143", ('1','4','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(100), r"\144", ('1','4','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(101), r"\145", ('1','4','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(102), r"\146", ('1','4','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(103), r"\147", ('1','4','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(104), r"\150", ('1','5','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(105), r"\151", ('1','5','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(106), r"\152", ('1','5','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(107), r"\153", ('1','5','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(108), r"\154", ('1','5','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(109), r"\155", ('1','5','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(110), r"\156", ('1','5','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(111), r"\157", ('1','5','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(112), r"\160", ('1','6','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(113), r"\161", ('1','6','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(114), r"\162", ('1','6','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(115), r"\163", ('1','6','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(116), r"\164", ('1','6','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(117), r"\165", ('1','6','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(118), r"\166", ('1','6','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(119), r"\167", ('1','6','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(120), r"\170", ('1','7','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(121), r"\171", ('1','7','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(122), r"\172", ('1','7','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(123), r"\173", ('1','7','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(124), r"\174", ('1','7','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(125), r"\175", ('1','7','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(126), r"\176", ('1','7','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(127), r"\177", ('1','7','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(128), r"\200", ('2','0','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(129), r"\201", ('2','0','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(130), r"\202", ('2','0','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(131), r"\203", ('2','0','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(132), r"\204", ('2','0','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(133), r"\205", ('2','0','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(134), r"\206", ('2','0','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(135), r"\207", ('2','0','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(136), r"\210", ('2','1','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(137), r"\211", ('2','1','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(138), r"\212", ('2','1','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(139), r"\213", ('2','1','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(140), r"\214", ('2','1','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(141), r"\215", ('2','1','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(142), r"\216", ('2','1','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(143), r"\217", ('2','1','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(144), r"\220", ('2','2','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(145), r"\221", ('2','2','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(146), r"\222", ('2','2','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(147), r"\223", ('2','2','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(148), r"\224", ('2','2','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(149), r"\225", ('2','2','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(150), r"\226", ('2','2','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(151), r"\227", ('2','2','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(152), r"\230", ('2','3','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(153), r"\231", ('2','3','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(154), r"\232", ('2','3','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(155), r"\233", ('2','3','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(156), r"\234", ('2','3','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(157), r"\235", ('2','3','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(158), r"\236", ('2','3','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(159), r"\237", ('2','3','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(160), r"\240", ('2','4','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(161), r"\241", ('2','4','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(162), r"\242", ('2','4','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(163), r"\243", ('2','4','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(164), r"\244", ('2','4','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(165), r"\245", ('2','4','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(166), r"\246", ('2','4','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(167), r"\247", ('2','4','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(168), r"\250", ('2','5','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(169), r"\251", ('2','5','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(170), r"\252", ('2','5','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(171), r"\253", ('2','5','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(172), r"\254", ('2','5','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(173), r"\255", ('2','5','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(174), r"\256", ('2','5','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(175), r"\257", ('2','5','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(176), r"\260", ('2','6','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(177), r"\261", ('2','6','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(178), r"\262", ('2','6','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(179), r"\263", ('2','6','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(180), r"\264", ('2','6','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(181), r"\265", ('2','6','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(182), r"\266", ('2','6','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(183), r"\267", ('2','6','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(184), r"\270", ('2','7','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(185), r"\271", ('2','7','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(186), r"\272", ('2','7','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(187), r"\273", ('2','7','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(188), r"\274", ('2','7','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(189), r"\275", ('2','7','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(190), r"\276", ('2','7','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(191), r"\277", ('2','7','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(192), r"\300", ('3','0','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(193), r"\301", ('3','0','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(194), r"\302", ('3','0','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(195), r"\303", ('3','0','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(196), r"\304", ('3','0','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(197), r"\305", ('3','0','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(198), r"\306", ('3','0','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(199), r"\307", ('3','0','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(200), r"\310", ('3','1','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(201), r"\311", ('3','1','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(202), r"\312", ('3','1','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(203), r"\313", ('3','1','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(204), r"\314", ('3','1','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(205), r"\315", ('3','1','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(206), r"\316", ('3','1','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(207), r"\317", ('3','1','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(208), r"\320", ('3','2','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(209), r"\321", ('3','2','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(210), r"\322", ('3','2','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(211), r"\323", ('3','2','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(212), r"\324", ('3','2','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(213), r"\325", ('3','2','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(214), r"\326", ('3','2','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(215), r"\327", ('3','2','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(216), r"\330", ('3','3','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(217), r"\331", ('3','3','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(218), r"\332", ('3','3','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(219), r"\333", ('3','3','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(220), r"\334", ('3','3','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(221), r"\335", ('3','3','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(222), r"\336", ('3','3','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(223), r"\337", ('3','3','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(224), r"\340", ('3','4','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(225), r"\341", ('3','4','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(226), r"\342", ('3','4','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(227), r"\343", ('3','4','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(228), r"\344", ('3','4','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(229), r"\345", ('3','4','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(230), r"\346", ('3','4','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(231), r"\347", ('3','4','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(232), r"\350", ('3','5','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(233), r"\351", ('3','5','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(234), r"\352", ('3','5','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(235), r"\353", ('3','5','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(236), r"\354", ('3','5','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(237), r"\355", ('3','5','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(238), r"\356", ('3','5','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(239), r"\357", ('3','5','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(240), r"\360", ('3','6','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(241), r"\361", ('3','6','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(242), r"\362", ('3','6','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(243), r"\363", ('3','6','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(244), r"\364", ('3','6','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(245), r"\365", ('3','6','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(246), r"\366", ('3','6','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(247), r"\367", ('3','6','7'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(248), r"\370", ('3','7','0'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(249), r"\371", ('3','7','1'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(250), r"\372", ('3','7','2'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(251), r"\373", ('3','7','3'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(252), r"\374", ('3','7','4'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(253), r"\375", ('3','7','5'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(254), r"\376", ('3','7','6'));
        test_escapable_octal_to_display_mapping(EscapableChar::EscapedOctal(255), r"\377", ('3','7','7'));
    }
}
