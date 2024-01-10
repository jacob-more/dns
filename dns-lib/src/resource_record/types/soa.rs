use dns_macros::{ToWire, FromWire, FromTokenizedRecord, RTypeCode, ToPresentation};

use crate::{types::c_domain_name::CDomainName, resource_record::time::Time};

/// (Original) https://datatracker.ietf.org/doc/html/rfc1035#section-3.3.13
#[derive(Clone, PartialEq, Eq, Hash, Debug, ToWire, FromWire, ToPresentation, FromTokenizedRecord, RTypeCode)]
pub struct SOA {
    mname: CDomainName,
    rname: CDomainName,
    serial: u32,
    refresh: Time,
    retry: Time,
    expire: Time,
    minimum: u32,
}

impl SOA {
    #[inline]
    pub fn new(mname: CDomainName, rname: CDomainName, serial: u32, refresh: Time, retry: Time, expire: Time, minimum: u32,) -> Self {
        Self { mname, rname, serial, refresh, retry, expire, minimum }
    }

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
    use crate::{serde::wire::circular_test::gen_test_circular_serde_sanity_test, types::c_domain_name::CDomainName, resource_record::time::Time};
    use super::SOA;

    gen_test_circular_serde_sanity_test!(
        record_circular_serde_sanity_test,
        SOA {
            mname: CDomainName::from_utf8("name_server.example.com.").unwrap(),
            rname: CDomainName::from_utf8("responsible_person.example.com.").unwrap(),
            serial: 12,
            refresh: Time::new(60),
            retry: Time::new(15),
            expire: Time::new(86400),
            minimum: 0,
        }
    );
}

#[cfg(test)]
mod tokenizer_tests {
    use crate::{serde::presentation::test_from_tokenized_record::{gen_ok_record_test, gen_fail_record_test}, types::c_domain_name::CDomainName, resource_record::time::Time};
    use super::SOA;

    const GOOD_DOMAIN: &str = "www.example.com.";
    const BAD_DOMAIN: &str = "..www.example.com.";
    const GOOD_INTEGER: &str = "10";
    const BAD_INTEGER: &str = "bad_integer";
    const NEGATIVE_INTEGER: &str = "-1";

    // Good SOA record
    gen_ok_record_test!(
        test_ok,
        SOA,
        SOA {
            mname: CDomainName::from_utf8(GOOD_DOMAIN).unwrap(),
            rname: CDomainName::from_utf8(GOOD_DOMAIN).unwrap(),
            serial: 10,
            refresh: Time::new(10),
            retry: Time::new(10),
            expire: Time::new(10),
            minimum: 10
        },
        [GOOD_DOMAIN, GOOD_DOMAIN, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER]
    );

    // Test bad tokens
    gen_fail_record_test!(test_fail_bad_mname, SOA, [BAD_DOMAIN, GOOD_DOMAIN, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER]);
    gen_fail_record_test!(test_fail_bad_rname, SOA, [GOOD_DOMAIN, BAD_DOMAIN, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER]);
    gen_fail_record_test!(test_fail_bad_serial, SOA, [GOOD_DOMAIN, GOOD_DOMAIN, BAD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER]);
    gen_fail_record_test!(test_fail_bad_refresh, SOA, [GOOD_DOMAIN, GOOD_DOMAIN, GOOD_INTEGER, BAD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER]);
    gen_fail_record_test!(test_fail_bad_retry, SOA, [GOOD_DOMAIN, GOOD_DOMAIN, GOOD_INTEGER, GOOD_INTEGER, BAD_INTEGER, GOOD_INTEGER, GOOD_INTEGER]);
    gen_fail_record_test!(test_fail_bad_expire, SOA, [GOOD_DOMAIN, GOOD_DOMAIN, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, BAD_INTEGER, GOOD_INTEGER]);
    gen_fail_record_test!(test_fail_bad_minimum, SOA, [GOOD_DOMAIN, GOOD_DOMAIN, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, BAD_INTEGER]);

    // Test negative tokens
    gen_fail_record_test!(test_fail_negative_serial, SOA, [GOOD_DOMAIN, GOOD_DOMAIN, NEGATIVE_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER]);
    gen_fail_record_test!(test_fail_negative_minimum, SOA, [GOOD_DOMAIN, GOOD_DOMAIN, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, NEGATIVE_INTEGER]);

    // Test incorrect tokens counts
    gen_fail_record_test!(test_fail_eight_tokens, SOA, [GOOD_DOMAIN, GOOD_DOMAIN, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_DOMAIN]);
    gen_fail_record_test!(test_fail_six_tokens, SOA, [GOOD_DOMAIN, GOOD_DOMAIN, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER]);
    gen_fail_record_test!(test_fail_five_tokens, SOA, [GOOD_DOMAIN, GOOD_DOMAIN, GOOD_INTEGER, GOOD_INTEGER, GOOD_INTEGER]);
    gen_fail_record_test!(test_fail_four_tokens, SOA, [GOOD_DOMAIN, GOOD_DOMAIN, GOOD_INTEGER, GOOD_INTEGER]);
    gen_fail_record_test!(test_fail_three_tokens, SOA, [GOOD_DOMAIN, GOOD_DOMAIN, GOOD_INTEGER]);
    gen_fail_record_test!(test_fail_two_tokens, SOA, [GOOD_DOMAIN, GOOD_DOMAIN]);
    gen_fail_record_test!(test_fail_one_tokens, SOA, [GOOD_DOMAIN]);
    gen_fail_record_test!(test_fail_no_tokens, SOA, []);
}
