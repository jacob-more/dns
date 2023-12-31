use dns_macros::{ToWire, FromWire, FromTokenizedRecord, RTypeCode};

use crate::types::c_domain_name::CDomainName;

/// (Original) https://datatracker.ietf.org/doc/html/rfc1035#section-3.3.13
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, FromTokenizedRecord, RTypeCode)]
pub struct SOA {
    mname: CDomainName,
    rname: CDomainName,
    serial: u32,
    refresh: i32,   // TODO: make DNSTime once that is defined
    retry: i32,     // TODO: make DNSTime once that is defined
    expire: i32,    // TODO: make DNSTime once that is defined
    minimum: u32,
}

impl SOA {
    #[inline]
    pub fn main_domain_name(&self) -> &CDomainName {
        &self.mname
    }

    #[inline]
    pub fn responsible_mailbox_domain_name(&self) -> &CDomainName {
        &self.rname
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use crate::{serde::wire::circular_test::gen_test_circular_serde_sanity_test, types::c_domain_name::CDomainName};
    use super::SOA;

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test,
        SOA {
            mname: CDomainName::from_utf8("name_server.example.com.").unwrap(),
            rname: CDomainName::from_utf8("responsible_person.example.com.").unwrap(),
            serial: 12,
            refresh: 60,
            retry: 15,
            expire: 86400,
            minimum: 0,
        }
    );
}

#[cfg(test)]
mod tokenizer_tests {
    use crate::{serde::presentation::test_from_tokenized_record::{gen_ok_record_test, gen_fail_record_test}, types::c_domain_name::CDomainName};
    use super::SOA;

    const GOOD_DOMAIN_NAME: &str = "www.example.com.";
    const GOOD_INTEGER: &str = "10";
    const BAD_INTEGER: &str = "bad_integer";
    const NEGATIVE_INTEGER: &str = "-1";

    // Good SOA record
    gen_ok_record_test!(
        test_ok,
        SOA,
        SOA {
            mname: CDomainName::from_utf8(GOOD_DOMAIN_NAME).unwrap(),
            rname: CDomainName::from_utf8(GOOD_DOMAIN_NAME).unwrap(),
            serial: 10,
            refresh: 10,
            retry: 10,
            expire: 10,
            minimum: 10
        },
        [GOOD_DOMAIN_NAME, GOOD_DOMAIN_NAME, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER]
    );

    // Good SOA record with negative DNSTime
    // TODO: this will probably not be allowed in the future once the DNSTime has been defined
    gen_ok_record_test!(
        test_ok_negative_refresh,
        SOA,
        SOA {
            mname: CDomainName::from_utf8(GOOD_DOMAIN_NAME).unwrap(),
            rname: CDomainName::from_utf8(GOOD_DOMAIN_NAME).unwrap(),
            serial: 10,
            refresh: -1,
            retry: 10,
            expire: 10,
            minimum: 10
        },
        [GOOD_DOMAIN_NAME, GOOD_DOMAIN_NAME, GOOD_INTEGER, NEGATIVE_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER]
    );
    gen_ok_record_test!(
        test_ok_negative_retry,
        SOA,
        SOA {
            mname: CDomainName::from_utf8(GOOD_DOMAIN_NAME).unwrap(),
            rname: CDomainName::from_utf8(GOOD_DOMAIN_NAME).unwrap(),
            serial: 10,
            refresh: 10,
            retry: -1,
            expire: 10,
            minimum: 10
        },
        [GOOD_DOMAIN_NAME, GOOD_DOMAIN_NAME, GOOD_INTEGER, GOOD_INTEGER, NEGATIVE_INTEGER, GOOD_INTEGER, GOOD_INTEGER]
    );
    gen_ok_record_test!(
        test_ok_negative_expire,
        SOA,
        SOA {
            mname: CDomainName::from_utf8(GOOD_DOMAIN_NAME).unwrap(),
            rname: CDomainName::from_utf8(GOOD_DOMAIN_NAME).unwrap(),
            serial: 10,
            refresh: 10,
            retry: 10,
            expire: -1,
            minimum: 10
        },
        [GOOD_DOMAIN_NAME, GOOD_DOMAIN_NAME, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, NEGATIVE_INTEGER, GOOD_INTEGER]
    );

    // Test bad tokens
    gen_fail_record_test!(test_fail_bad_serial, SOA, [GOOD_DOMAIN_NAME, GOOD_DOMAIN_NAME, BAD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER]);
    gen_fail_record_test!(test_fail_bad_refresh, SOA, [GOOD_DOMAIN_NAME, GOOD_DOMAIN_NAME, GOOD_INTEGER, BAD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER]);
    gen_fail_record_test!(test_fail_bad_retry, SOA, [GOOD_DOMAIN_NAME, GOOD_DOMAIN_NAME, GOOD_INTEGER, GOOD_INTEGER, BAD_INTEGER, GOOD_INTEGER, GOOD_INTEGER]);
    gen_fail_record_test!(test_fail_bad_expire, SOA, [GOOD_DOMAIN_NAME, GOOD_DOMAIN_NAME, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, BAD_INTEGER, GOOD_INTEGER]);
    gen_fail_record_test!(test_fail_bad_minimum, SOA, [GOOD_DOMAIN_NAME, GOOD_DOMAIN_NAME, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, BAD_INTEGER]);

    // Test negative tokens
    gen_fail_record_test!(test_fail_negative_serial, SOA, [GOOD_DOMAIN_NAME, GOOD_DOMAIN_NAME, NEGATIVE_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER]);
    gen_fail_record_test!(test_fail_negative_minimum, SOA, [GOOD_DOMAIN_NAME, GOOD_DOMAIN_NAME, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, NEGATIVE_INTEGER]);

    // Test incorrect tokens counts
    gen_fail_record_test!(test_fail_eight_tokens, SOA, [GOOD_DOMAIN_NAME, GOOD_DOMAIN_NAME, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_DOMAIN_NAME]);
    gen_fail_record_test!(test_fail_six_tokens, SOA, [GOOD_DOMAIN_NAME, GOOD_DOMAIN_NAME, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER]);
    gen_fail_record_test!(test_fail_five_tokens, SOA, [GOOD_DOMAIN_NAME, GOOD_DOMAIN_NAME, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER]);
    gen_fail_record_test!(test_fail_four_tokens, SOA, [GOOD_DOMAIN_NAME, GOOD_DOMAIN_NAME, GOOD_INTEGER, GOOD_INTEGER]);
    gen_fail_record_test!(test_fail_three_tokens, SOA, [GOOD_DOMAIN_NAME, GOOD_DOMAIN_NAME, GOOD_INTEGER]);
    gen_fail_record_test!(test_fail_two_tokens, SOA, [GOOD_DOMAIN_NAME, GOOD_DOMAIN_NAME]);
    gen_fail_record_test!(test_fail_one_tokens, SOA, [GOOD_DOMAIN_NAME]);
    gen_fail_record_test!(test_fail_no_tokens, SOA, []);
}
