use std::time::Instant;

use dns_lib::resource_record::resource_record::ResourceRecord;


#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CachedRecord {
    pub insertion_time: Instant,
    pub record: ResourceRecord
}

impl CachedRecord {
    #[inline]
    pub fn is_expired(&self) -> bool {
        self.insertion_time.elapsed().as_secs() >= self.record.ttl().as_secs() as u64
    }
}
