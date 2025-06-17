macro_rules! gen_ok_token_test {
    ($test_name:ident, $type:ty, $expected:expr, $tokens:expr) => {
        #[test]
        fn $test_name() {
            let expected =  $expected;
            let tokens = $tokens;
            let result = <$type as $crate::serde::presentation::from_presentation::FromPresentation>::from_token_format(tokens);

            assert!(result.is_ok());
            let (actual_record, remaining_tokens) = result.unwrap();
            assert_eq!(actual_record, expected);
            assert_eq!(0, remaining_tokens.len());
        }
    }
}

macro_rules! gen_fail_token_test {
    ($test_name:ident, $type:ty, $tokens:expr) => {
        #[test]
        fn $test_name() {
            let tokens = $tokens;
            let result = <$type as $crate::serde::presentation::from_presentation::FromPresentation>::from_token_format(tokens);

            assert!(result.is_err());
        }
    }
}

pub(crate) use gen_fail_token_test;
pub(crate) use gen_ok_token_test;
