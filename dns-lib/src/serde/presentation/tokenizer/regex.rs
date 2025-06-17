use lazy_static::lazy_static;
use regex::Regex;

// Opening or closing parenthesis on their own are a text literal. However, if they are a part of a
// larger non-quoted text literal, they should be pulled out as their own text literal.
const CHARACTER_STR_UNQUOTED: &str =
    "\\A((([[:ascii:]&&[^ \\t;\\r\\n\"\\(\\)\\\\]]|(\\\\[^0-9])|(\\\\[0-7]{3}))+)|(\\()|(\\)))";
const CHARACTER_STR_QUOTED: &str =
    "\\A(\"(([[:ascii:]&&[^\"\\\\]]|(\\\\[^0-9])|(\\\\[0-7]{3}))*)\")";

lazy_static! {
    pub static ref REGEX_CHARACTER_STR_UNQUOTED: Regex =
        Regex::new(CHARACTER_STR_UNQUOTED).unwrap();
    pub static ref REGEX_CHARACTER_STR_QUOTED: Regex = Regex::new(CHARACTER_STR_QUOTED).unwrap();
    pub static ref REGEX_SEPARATOR: Regex = Regex::new("\\A([ \t]+)").unwrap();
    pub static ref REGEX_NEW_LINE: Regex = Regex::new("\\A(\r?\n)").unwrap();
    pub static ref REGEX_COMMENT: Regex = Regex::new("\\A((;)([^\n]*))").unwrap();
}

const RCLASS_STR: &str = r"\A((IN)|(CS)|(CH)|(HS)|(NONE)|(ANY)|(CLASS[[:digit:]]+))\z";
const RTYPE_STR: &str = r"\A(([A-Z]+)|(TYPE[[:digit:]]+))\z";
const TTL_STR: &str = r"\A([[:digit:]]+)\z";

lazy_static! {
    pub static ref REGEX_RCLASS: Regex = Regex::new(RCLASS_STR).unwrap();
    pub static ref REGEX_RTYPE: Regex = Regex::new(RTYPE_STR).unwrap();
    pub static ref REGEX_TTL: Regex = Regex::new(TTL_STR).unwrap();
}

#[cfg(test)]
mod character_str_regex {
    use regex::Regex;

    use super::{CHARACTER_STR_QUOTED, CHARACTER_STR_UNQUOTED};

    const QUOTED_TESTS: [[&str; 2]; 3] = [
        ["\"This\" is a character string", "\"This\""],
        [
            "\"This is a\n character\" string",
            "\"This is a\n character\"",
        ],
        ["\"\" string", "\"\""],
    ];
    const UNQUOTED_TESTS: [[&str; 2]; 4] = [
        ["This is a character string", "This"],
        ["This; is a character string", "This"],
        ["This\n is a character string", "This"],
        ["This\" is a character string", "This"],
    ];
    const FAIL_TESTS: [&str; 4] = ["    ", " \n ", "  \r\n", "\""];

    #[test]
    fn test_read_quoted_character_string_found() {
        let tests = QUOTED_TESTS;

        let quoted_char_str_regex = Regex::new(CHARACTER_STR_QUOTED).unwrap();
        for [input, expected_output] in tests {
            let actual_output = quoted_char_str_regex.find(input);
            assert!(actual_output.is_some());
            let actual_output = actual_output.unwrap();
            assert_eq!(
                actual_output.as_str(),
                expected_output,
                "Index start: {0}  Index end: {1}",
                actual_output.start(),
                actual_output.end()
            );
        }
    }

    #[test]
    fn test_read_unquoted_character_string_found() {
        let tests = UNQUOTED_TESTS;

        let unquoted_char_str_regex = Regex::new(CHARACTER_STR_UNQUOTED).unwrap();
        for [input, expected_output] in tests {
            let actual_output = unquoted_char_str_regex.find(input);
            assert!(
                actual_output.is_some(),
                "Expected Some but got None  Input: '{input}'  Expected Output: '{expected_output}'"
            );
            let actual_output = actual_output.unwrap();
            assert_eq!(
                actual_output.as_str(),
                expected_output,
                "Index start: {0}  Index end: {1}",
                actual_output.start(),
                actual_output.end()
            );
        }
    }

    #[test]
    fn test_read_quoted_and_unquoted_character_string_found() {
        let tests = QUOTED_TESTS.iter().chain(UNQUOTED_TESTS.iter());

        let char_str_regex = Regex::new(&format!(
            "(({CHARACTER_STR_UNQUOTED})|({CHARACTER_STR_QUOTED}))"
        ))
        .unwrap();
        for [input, expected_output] in tests {
            let actual_output = char_str_regex.find(input);
            assert!(
                actual_output.is_some(),
                "Expected Some but got None  Input: '{input}'  Expected Output: '{expected_output}'"
            );
            let actual_output = actual_output.unwrap();
            assert_eq!(
                actual_output.as_str(),
                *expected_output,
                "Index start: {0}  Index end: {1}",
                actual_output.start(),
                actual_output.end()
            );
        }
    }

    #[test]
    fn test_read_quoted_and_unquoted_character_string_not_found() {
        let tests = FAIL_TESTS;

        let char_str_regex = Regex::new(&format!(
            "(({CHARACTER_STR_UNQUOTED})|({CHARACTER_STR_QUOTED}))"
        ))
        .unwrap();
        for input in tests {
            let actual_output = char_str_regex.find(input);
            assert!(
                actual_output.is_none(),
                "Found: {0}",
                actual_output.unwrap().as_str()
            );
        }
    }
}
