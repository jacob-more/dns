macro_rules! gen_ok_record_test {
    ($test_name:ident, $rtype:ident, $expected:expr, $tokens:expr) => {
        #[test]
        fn $test_name() {
            let expected =  $expected;
            let rdata = $tokens.to_vec();
            let actual = <$rtype as $crate::serde::presentation::from_tokenized_rdata::FromTokenizedRData>::from_tokenized_rdata(&rdata);
    
            assert!(actual.is_ok());
            let actual_record = actual.unwrap();
            assert_eq!(actual_record, expected);
        }
    }
}

macro_rules! gen_fail_record_test {
    ($test_name:ident, $rtype:ident, $tokens:expr) => {
        #[test]
        fn $test_name() {
            let rdata = $tokens.to_vec();
            let actual = <$rtype as $crate::serde::presentation::from_tokenized_rdata::FromTokenizedRData>::from_tokenized_rdata(&rdata);
    
            assert!(actual.is_err());
        }
    }
}

pub(crate) use gen_ok_record_test;
pub(crate) use gen_fail_record_test;
