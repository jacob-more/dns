macro_rules! gen_ok_token_test {
    ($test_name:ident, $type:ty, $expected:expr, $token:expr) => {
        #[test]
        fn $test_name() {
            let expected =  $expected;
            let token = $token;
            let actual = <$type as crate::serde::presentation::from_presentation::FromPresentation>::from_token_format(&token);
    
            assert!(actual.is_ok());
            let actual_record = actual.unwrap();
            assert_eq!(actual_record, expected);
        }
    }
}

macro_rules! gen_fail_token_test {
    ($test_name:ident, $type:ty, $token:expr) => {
        #[test]
        fn $test_name() {
            let token = $token;
            let actual = <$type as crate::serde::presentation::from_presentation::FromPresentation>::from_token_format(&token);
    
            assert!(actual.is_err());
        }
    }
}

pub(crate) use gen_ok_token_test;
pub(crate) use gen_fail_token_test;
