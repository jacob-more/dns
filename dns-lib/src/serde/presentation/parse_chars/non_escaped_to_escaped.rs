use crate::types::ascii::{AsciiChar, constants::ASCII_BACKSLASH, is_control_char};

use super::char_token::EscapableChar;

/// Converts from an internal sequence of raw ascii characters, with no escape sequences, and
/// converts it into printable ascii, with escaped backslashes and octal sequences for non-printable
/// characters.
pub struct NonEscapedIntoEscapedIter<T> where T: Iterator<Item = AsciiChar> {
    chars: T
}

impl<T> NonEscapedIntoEscapedIter<T> where T: Iterator<Item = AsciiChar> {
    #[inline]
    pub fn new(iterator: T) -> Self {
        NonEscapedIntoEscapedIter { chars: iterator }
    }
}

impl<T> From<T> for NonEscapedIntoEscapedIter<T> where T: Iterator<Item = AsciiChar> {
    #[inline]
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T> Iterator for NonEscapedIntoEscapedIter<T> where T: Iterator<Item = AsciiChar> {
    type Item = EscapableChar;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.chars.next() {
            None => None,
            Some(ASCII_BACKSLASH) => Some(EscapableChar::EscapedAscii(ASCII_BACKSLASH)),
            Some(character) if is_control_char(character) => Some(EscapableChar::EscapedOctal(character)),
            Some(character) => Some(EscapableChar::Ascii(character)),
        }
    }
}


#[cfg(test)]
mod non_escaped_to_escaped_tests {
    use crate::{serde::presentation::parse_chars::char_token::EscapableChar, types::ascii::AsciiChar};

    use super::NonEscapedIntoEscapedIter;

    fn test_character_escape_mapping(input: AsciiChar, output: EscapableChar) {
        let character_iter = [input];
        let mut escapable_character_iter = NonEscapedIntoEscapedIter::new(character_iter.into_iter());

        let first_escapable_character = escapable_character_iter.next();
        assert!(first_escapable_character.is_some(), "At least one character was expected to be output by the NonEscapedIntoEscapedIter but none were");
        let first_escapable_character = first_escapable_character.unwrap();
        assert_eq!(first_escapable_character, output, "The escaped character was expected to be {output:?} but was {first_escapable_character:?}");
    }

    #[test]
    fn characters_map_correctly() {
        test_character_escape_mapping(000, EscapableChar::EscapedOctal(000));
        test_character_escape_mapping(001, EscapableChar::EscapedOctal(001));
        test_character_escape_mapping(002, EscapableChar::EscapedOctal(002));
        test_character_escape_mapping(003, EscapableChar::EscapedOctal(003));
        test_character_escape_mapping(004, EscapableChar::EscapedOctal(004));
        test_character_escape_mapping(005, EscapableChar::EscapedOctal(005));
        test_character_escape_mapping(006, EscapableChar::EscapedOctal(006));
        test_character_escape_mapping(007, EscapableChar::EscapedOctal(007));
        test_character_escape_mapping(008, EscapableChar::EscapedOctal(008));
        test_character_escape_mapping(009, EscapableChar::EscapedOctal(009));
        test_character_escape_mapping(010, EscapableChar::EscapedOctal(010));
        test_character_escape_mapping(011, EscapableChar::EscapedOctal(011));
        test_character_escape_mapping(012, EscapableChar::EscapedOctal(012));
        test_character_escape_mapping(013, EscapableChar::EscapedOctal(013));
        test_character_escape_mapping(014, EscapableChar::EscapedOctal(014));
        test_character_escape_mapping(015, EscapableChar::EscapedOctal(015));
        test_character_escape_mapping(016, EscapableChar::EscapedOctal(016));
        test_character_escape_mapping(017, EscapableChar::EscapedOctal(017));
        test_character_escape_mapping(018, EscapableChar::EscapedOctal(018));
        test_character_escape_mapping(019, EscapableChar::EscapedOctal(019));
        test_character_escape_mapping(020, EscapableChar::EscapedOctal(020));
        test_character_escape_mapping(021, EscapableChar::EscapedOctal(021));
        test_character_escape_mapping(022, EscapableChar::EscapedOctal(022));
        test_character_escape_mapping(023, EscapableChar::EscapedOctal(023));
        test_character_escape_mapping(024, EscapableChar::EscapedOctal(024));
        test_character_escape_mapping(025, EscapableChar::EscapedOctal(025));
        test_character_escape_mapping(026, EscapableChar::EscapedOctal(026));
        test_character_escape_mapping(027, EscapableChar::EscapedOctal(027));
        test_character_escape_mapping(028, EscapableChar::EscapedOctal(028));
        test_character_escape_mapping(029, EscapableChar::EscapedOctal(029));
        test_character_escape_mapping(030, EscapableChar::EscapedOctal(030));
        test_character_escape_mapping(031, EscapableChar::EscapedOctal(031));
        test_character_escape_mapping(032, EscapableChar::Ascii(032));
        test_character_escape_mapping(033, EscapableChar::Ascii(033));
        test_character_escape_mapping(034, EscapableChar::Ascii(034));
        test_character_escape_mapping(035, EscapableChar::Ascii(035));
        test_character_escape_mapping(036, EscapableChar::Ascii(036));
        test_character_escape_mapping(037, EscapableChar::Ascii(037));
        test_character_escape_mapping(038, EscapableChar::Ascii(038));
        test_character_escape_mapping(039, EscapableChar::Ascii(039));
        test_character_escape_mapping(040, EscapableChar::Ascii(040));
        test_character_escape_mapping(041, EscapableChar::Ascii(041));
        test_character_escape_mapping(042, EscapableChar::Ascii(042));
        test_character_escape_mapping(043, EscapableChar::Ascii(043));
        test_character_escape_mapping(044, EscapableChar::Ascii(044));
        test_character_escape_mapping(045, EscapableChar::Ascii(045));
        test_character_escape_mapping(046, EscapableChar::Ascii(046));
        test_character_escape_mapping(047, EscapableChar::Ascii(047));
        test_character_escape_mapping(048, EscapableChar::Ascii(048));
        test_character_escape_mapping(049, EscapableChar::Ascii(049));
        test_character_escape_mapping(050, EscapableChar::Ascii(050));
        test_character_escape_mapping(051, EscapableChar::Ascii(051));
        test_character_escape_mapping(052, EscapableChar::Ascii(052));
        test_character_escape_mapping(053, EscapableChar::Ascii(053));
        test_character_escape_mapping(054, EscapableChar::Ascii(054));
        test_character_escape_mapping(055, EscapableChar::Ascii(055));
        test_character_escape_mapping(056, EscapableChar::Ascii(056));
        test_character_escape_mapping(057, EscapableChar::Ascii(057));
        test_character_escape_mapping(058, EscapableChar::Ascii(058));
        test_character_escape_mapping(059, EscapableChar::Ascii(059));
        test_character_escape_mapping(060, EscapableChar::Ascii(060));
        test_character_escape_mapping(061, EscapableChar::Ascii(061));
        test_character_escape_mapping(062, EscapableChar::Ascii(062));
        test_character_escape_mapping(063, EscapableChar::Ascii(063));
        test_character_escape_mapping(064, EscapableChar::Ascii(064));
        test_character_escape_mapping(065, EscapableChar::Ascii(065));
        test_character_escape_mapping(066, EscapableChar::Ascii(066));
        test_character_escape_mapping(067, EscapableChar::Ascii(067));
        test_character_escape_mapping(068, EscapableChar::Ascii(068));
        test_character_escape_mapping(069, EscapableChar::Ascii(069));
        test_character_escape_mapping(070, EscapableChar::Ascii(070));
        test_character_escape_mapping(071, EscapableChar::Ascii(071));
        test_character_escape_mapping(072, EscapableChar::Ascii(072));
        test_character_escape_mapping(073, EscapableChar::Ascii(073));
        test_character_escape_mapping(074, EscapableChar::Ascii(074));
        test_character_escape_mapping(075, EscapableChar::Ascii(075));
        test_character_escape_mapping(076, EscapableChar::Ascii(076));
        test_character_escape_mapping(077, EscapableChar::Ascii(077));
        test_character_escape_mapping(078, EscapableChar::Ascii(078));
        test_character_escape_mapping(079, EscapableChar::Ascii(079));
        test_character_escape_mapping(080, EscapableChar::Ascii(080));
        test_character_escape_mapping(081, EscapableChar::Ascii(081));
        test_character_escape_mapping(082, EscapableChar::Ascii(082));
        test_character_escape_mapping(083, EscapableChar::Ascii(083));
        test_character_escape_mapping(084, EscapableChar::Ascii(084));
        test_character_escape_mapping(085, EscapableChar::Ascii(085));
        test_character_escape_mapping(086, EscapableChar::Ascii(086));
        test_character_escape_mapping(087, EscapableChar::Ascii(087));
        test_character_escape_mapping(088, EscapableChar::Ascii(088));
        test_character_escape_mapping(089, EscapableChar::Ascii(089));
        test_character_escape_mapping(090, EscapableChar::Ascii(090));
        test_character_escape_mapping(091, EscapableChar::Ascii(091));
        test_character_escape_mapping(092, EscapableChar::EscapedAscii(092));
        test_character_escape_mapping(093, EscapableChar::Ascii(093));
        test_character_escape_mapping(094, EscapableChar::Ascii(094));
        test_character_escape_mapping(095, EscapableChar::Ascii(095));
        test_character_escape_mapping(096, EscapableChar::Ascii(096));
        test_character_escape_mapping(097, EscapableChar::Ascii(097));
        test_character_escape_mapping(098, EscapableChar::Ascii(098));
        test_character_escape_mapping(099, EscapableChar::Ascii(099));
        test_character_escape_mapping(100, EscapableChar::Ascii(100));
        test_character_escape_mapping(101, EscapableChar::Ascii(101));
        test_character_escape_mapping(102, EscapableChar::Ascii(102));
        test_character_escape_mapping(103, EscapableChar::Ascii(103));
        test_character_escape_mapping(104, EscapableChar::Ascii(104));
        test_character_escape_mapping(105, EscapableChar::Ascii(105));
        test_character_escape_mapping(106, EscapableChar::Ascii(106));
        test_character_escape_mapping(107, EscapableChar::Ascii(107));
        test_character_escape_mapping(108, EscapableChar::Ascii(108));
        test_character_escape_mapping(109, EscapableChar::Ascii(109));
        test_character_escape_mapping(110, EscapableChar::Ascii(110));
        test_character_escape_mapping(111, EscapableChar::Ascii(111));
        test_character_escape_mapping(112, EscapableChar::Ascii(112));
        test_character_escape_mapping(113, EscapableChar::Ascii(113));
        test_character_escape_mapping(114, EscapableChar::Ascii(114));
        test_character_escape_mapping(115, EscapableChar::Ascii(115));
        test_character_escape_mapping(116, EscapableChar::Ascii(116));
        test_character_escape_mapping(117, EscapableChar::Ascii(117));
        test_character_escape_mapping(118, EscapableChar::Ascii(118));
        test_character_escape_mapping(119, EscapableChar::Ascii(119));
        test_character_escape_mapping(120, EscapableChar::Ascii(120));
        test_character_escape_mapping(121, EscapableChar::Ascii(121));
        test_character_escape_mapping(122, EscapableChar::Ascii(122));
        test_character_escape_mapping(123, EscapableChar::Ascii(123));
        test_character_escape_mapping(124, EscapableChar::Ascii(124));
        test_character_escape_mapping(125, EscapableChar::Ascii(125));
        test_character_escape_mapping(126, EscapableChar::Ascii(126));
        test_character_escape_mapping(127, EscapableChar::EscapedOctal(127));
        test_character_escape_mapping(128, EscapableChar::EscapedOctal(128));
        test_character_escape_mapping(129, EscapableChar::EscapedOctal(129));
        test_character_escape_mapping(130, EscapableChar::EscapedOctal(130));
        test_character_escape_mapping(131, EscapableChar::EscapedOctal(131));
        test_character_escape_mapping(132, EscapableChar::EscapedOctal(132));
        test_character_escape_mapping(133, EscapableChar::EscapedOctal(133));
        test_character_escape_mapping(134, EscapableChar::EscapedOctal(134));
        test_character_escape_mapping(135, EscapableChar::EscapedOctal(135));
        test_character_escape_mapping(136, EscapableChar::EscapedOctal(136));
        test_character_escape_mapping(137, EscapableChar::EscapedOctal(137));
        test_character_escape_mapping(138, EscapableChar::EscapedOctal(138));
        test_character_escape_mapping(139, EscapableChar::EscapedOctal(139));
        test_character_escape_mapping(140, EscapableChar::EscapedOctal(140));
        test_character_escape_mapping(141, EscapableChar::EscapedOctal(141));
        test_character_escape_mapping(142, EscapableChar::EscapedOctal(142));
        test_character_escape_mapping(143, EscapableChar::EscapedOctal(143));
        test_character_escape_mapping(144, EscapableChar::EscapedOctal(144));
        test_character_escape_mapping(145, EscapableChar::EscapedOctal(145));
        test_character_escape_mapping(146, EscapableChar::EscapedOctal(146));
        test_character_escape_mapping(147, EscapableChar::EscapedOctal(147));
        test_character_escape_mapping(148, EscapableChar::EscapedOctal(148));
        test_character_escape_mapping(149, EscapableChar::EscapedOctal(149));
        test_character_escape_mapping(150, EscapableChar::EscapedOctal(150));
        test_character_escape_mapping(151, EscapableChar::EscapedOctal(151));
        test_character_escape_mapping(152, EscapableChar::EscapedOctal(152));
        test_character_escape_mapping(153, EscapableChar::EscapedOctal(153));
        test_character_escape_mapping(154, EscapableChar::EscapedOctal(154));
        test_character_escape_mapping(155, EscapableChar::EscapedOctal(155));
        test_character_escape_mapping(156, EscapableChar::EscapedOctal(156));
        test_character_escape_mapping(157, EscapableChar::EscapedOctal(157));
        test_character_escape_mapping(158, EscapableChar::EscapedOctal(158));
        test_character_escape_mapping(159, EscapableChar::EscapedOctal(159));
        test_character_escape_mapping(160, EscapableChar::EscapedOctal(160));
        test_character_escape_mapping(161, EscapableChar::EscapedOctal(161));
        test_character_escape_mapping(162, EscapableChar::EscapedOctal(162));
        test_character_escape_mapping(163, EscapableChar::EscapedOctal(163));
        test_character_escape_mapping(164, EscapableChar::EscapedOctal(164));
        test_character_escape_mapping(165, EscapableChar::EscapedOctal(165));
        test_character_escape_mapping(166, EscapableChar::EscapedOctal(166));
        test_character_escape_mapping(167, EscapableChar::EscapedOctal(167));
        test_character_escape_mapping(168, EscapableChar::EscapedOctal(168));
        test_character_escape_mapping(169, EscapableChar::EscapedOctal(169));
        test_character_escape_mapping(170, EscapableChar::EscapedOctal(170));
        test_character_escape_mapping(171, EscapableChar::EscapedOctal(171));
        test_character_escape_mapping(172, EscapableChar::EscapedOctal(172));
        test_character_escape_mapping(173, EscapableChar::EscapedOctal(173));
        test_character_escape_mapping(174, EscapableChar::EscapedOctal(174));
        test_character_escape_mapping(175, EscapableChar::EscapedOctal(175));
        test_character_escape_mapping(176, EscapableChar::EscapedOctal(176));
        test_character_escape_mapping(177, EscapableChar::EscapedOctal(177));
        test_character_escape_mapping(178, EscapableChar::EscapedOctal(178));
        test_character_escape_mapping(179, EscapableChar::EscapedOctal(179));
        test_character_escape_mapping(180, EscapableChar::EscapedOctal(180));
        test_character_escape_mapping(181, EscapableChar::EscapedOctal(181));
        test_character_escape_mapping(182, EscapableChar::EscapedOctal(182));
        test_character_escape_mapping(183, EscapableChar::EscapedOctal(183));
        test_character_escape_mapping(184, EscapableChar::EscapedOctal(184));
        test_character_escape_mapping(185, EscapableChar::EscapedOctal(185));
        test_character_escape_mapping(186, EscapableChar::EscapedOctal(186));
        test_character_escape_mapping(187, EscapableChar::EscapedOctal(187));
        test_character_escape_mapping(188, EscapableChar::EscapedOctal(188));
        test_character_escape_mapping(189, EscapableChar::EscapedOctal(189));
        test_character_escape_mapping(190, EscapableChar::EscapedOctal(190));
        test_character_escape_mapping(191, EscapableChar::EscapedOctal(191));
        test_character_escape_mapping(192, EscapableChar::EscapedOctal(192));
        test_character_escape_mapping(193, EscapableChar::EscapedOctal(193));
        test_character_escape_mapping(194, EscapableChar::EscapedOctal(194));
        test_character_escape_mapping(195, EscapableChar::EscapedOctal(195));
        test_character_escape_mapping(196, EscapableChar::EscapedOctal(196));
        test_character_escape_mapping(197, EscapableChar::EscapedOctal(197));
        test_character_escape_mapping(198, EscapableChar::EscapedOctal(198));
        test_character_escape_mapping(199, EscapableChar::EscapedOctal(199));
        test_character_escape_mapping(200, EscapableChar::EscapedOctal(200));
        test_character_escape_mapping(201, EscapableChar::EscapedOctal(201));
        test_character_escape_mapping(202, EscapableChar::EscapedOctal(202));
        test_character_escape_mapping(203, EscapableChar::EscapedOctal(203));
        test_character_escape_mapping(204, EscapableChar::EscapedOctal(204));
        test_character_escape_mapping(205, EscapableChar::EscapedOctal(205));
        test_character_escape_mapping(206, EscapableChar::EscapedOctal(206));
        test_character_escape_mapping(207, EscapableChar::EscapedOctal(207));
        test_character_escape_mapping(208, EscapableChar::EscapedOctal(208));
        test_character_escape_mapping(209, EscapableChar::EscapedOctal(209));
        test_character_escape_mapping(210, EscapableChar::EscapedOctal(210));
        test_character_escape_mapping(211, EscapableChar::EscapedOctal(211));
        test_character_escape_mapping(212, EscapableChar::EscapedOctal(212));
        test_character_escape_mapping(213, EscapableChar::EscapedOctal(213));
        test_character_escape_mapping(214, EscapableChar::EscapedOctal(214));
        test_character_escape_mapping(215, EscapableChar::EscapedOctal(215));
        test_character_escape_mapping(216, EscapableChar::EscapedOctal(216));
        test_character_escape_mapping(217, EscapableChar::EscapedOctal(217));
        test_character_escape_mapping(218, EscapableChar::EscapedOctal(218));
        test_character_escape_mapping(219, EscapableChar::EscapedOctal(219));
        test_character_escape_mapping(220, EscapableChar::EscapedOctal(220));
        test_character_escape_mapping(221, EscapableChar::EscapedOctal(221));
        test_character_escape_mapping(222, EscapableChar::EscapedOctal(222));
        test_character_escape_mapping(223, EscapableChar::EscapedOctal(223));
        test_character_escape_mapping(224, EscapableChar::EscapedOctal(224));
        test_character_escape_mapping(225, EscapableChar::EscapedOctal(225));
        test_character_escape_mapping(226, EscapableChar::EscapedOctal(226));
        test_character_escape_mapping(227, EscapableChar::EscapedOctal(227));
        test_character_escape_mapping(228, EscapableChar::EscapedOctal(228));
        test_character_escape_mapping(229, EscapableChar::EscapedOctal(229));
        test_character_escape_mapping(230, EscapableChar::EscapedOctal(230));
        test_character_escape_mapping(231, EscapableChar::EscapedOctal(231));
        test_character_escape_mapping(232, EscapableChar::EscapedOctal(232));
        test_character_escape_mapping(233, EscapableChar::EscapedOctal(233));
        test_character_escape_mapping(234, EscapableChar::EscapedOctal(234));
        test_character_escape_mapping(235, EscapableChar::EscapedOctal(235));
        test_character_escape_mapping(236, EscapableChar::EscapedOctal(236));
        test_character_escape_mapping(237, EscapableChar::EscapedOctal(237));
        test_character_escape_mapping(238, EscapableChar::EscapedOctal(238));
        test_character_escape_mapping(239, EscapableChar::EscapedOctal(239));
        test_character_escape_mapping(240, EscapableChar::EscapedOctal(240));
        test_character_escape_mapping(241, EscapableChar::EscapedOctal(241));
        test_character_escape_mapping(242, EscapableChar::EscapedOctal(242));
        test_character_escape_mapping(243, EscapableChar::EscapedOctal(243));
        test_character_escape_mapping(244, EscapableChar::EscapedOctal(244));
        test_character_escape_mapping(245, EscapableChar::EscapedOctal(245));
        test_character_escape_mapping(246, EscapableChar::EscapedOctal(246));
        test_character_escape_mapping(247, EscapableChar::EscapedOctal(247));
        test_character_escape_mapping(248, EscapableChar::EscapedOctal(248));
        test_character_escape_mapping(249, EscapableChar::EscapedOctal(249));
        test_character_escape_mapping(250, EscapableChar::EscapedOctal(250));
        test_character_escape_mapping(251, EscapableChar::EscapedOctal(251));
        test_character_escape_mapping(252, EscapableChar::EscapedOctal(252));
        test_character_escape_mapping(253, EscapableChar::EscapedOctal(253));
        test_character_escape_mapping(254, EscapableChar::EscapedOctal(254));
        test_character_escape_mapping(255, EscapableChar::EscapedOctal(255));
    }
}
