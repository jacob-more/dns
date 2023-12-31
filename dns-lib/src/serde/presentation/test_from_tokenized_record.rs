macro_rules! gen_ok_record_test {
    ($test_name:ident, $rtype:ident, $expected:expr, $tokens:expr) => {
        #[test]
        fn $test_name() {
            let expected =  $expected;
            let rtype_mnemonic = <$rtype>::RTYPE.mnemonic();
            let rdata_tokens = crate::serde::presentation::tokenizer::tokenizer::ResourceRecord {
                domain_name: "www.example.org.",
                ttl: "86400",
                rclass: "IN",
                rtype: rtype_mnemonic.as_str(),
                rdata: $tokens.to_vec()
            };
    
            let actual = <$rtype as crate::serde::presentation::from_tokenized_record::FromTokenizedRecord>::from_tokenized_record(&rdata_tokens);
    
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
            let rtype_mnemonic = <$rtype>::RTYPE.mnemonic();
            let rdata_tokens = crate::serde::presentation::tokenizer::tokenizer::ResourceRecord {
                domain_name: "www.example.org.",
                ttl: "86400",
                rclass: "IN",
                rtype: rtype_mnemonic.as_str(),
                rdata: $tokens.to_vec()
            };
    
            let actual = <$rtype as crate::serde::presentation::from_tokenized_record::FromTokenizedRecord>::from_tokenized_record(&rdata_tokens);
    
            assert!(actual.is_err());
        }
    }
}

pub(crate) use gen_ok_record_test;
pub(crate) use gen_fail_record_test;
