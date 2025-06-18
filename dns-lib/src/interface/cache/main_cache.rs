use std::{
    fs::File,
    io::{self, Read},
    time::Instant,
};

use async_trait::async_trait;
use futures::{Stream, StreamExt};
use tokio::io::AsyncReadExt;

use crate::serde::presentation::zone_file_reader::{ZoneFileReader, ZoneToken};

use super::{CacheMeta, CacheQuery, CacheRecord, CacheResponse, MetaAuth};

pub trait MainCache {
    fn get(&self, query: &CacheQuery) -> CacheResponse;
    fn insert_record(&mut self, record: CacheRecord);
    fn insert_iter(&mut self, records: impl Iterator<Item = CacheRecord> + Send) {
        records.for_each(|record| self.insert_record(record));
    }
    fn clean(&mut self);

    #[inline]
    fn load_from_tokenizer(&mut self, tokenizer: ZoneFileReader, authoritative: MetaAuth) {
        let insertion_time = Instant::now();
        let meta = CacheMeta {
            auth: authoritative,
            insertion_time,
        };
        for token in tokenizer {
            match token {
                Ok(ZoneToken::ResourceRecord(record)) => self.insert_record(CacheRecord {
                    meta: meta.clone(),
                    record,
                }),
                Ok(ZoneToken::Include {
                    file_path,
                    domain_name,
                }) => {
                    // Read in the file and store it in the buffer. The buffer will be the feed for
                    // the sub-tokenizer
                    let mut buffer = String::new();
                    let mut file = match File::open(file_path) {
                        Ok(file) => file,
                        Err(error) => {
                            println!("{error}");
                            continue;
                        }
                    };
                    if let Err(error) = file.read_to_string(&mut buffer) {
                        println!("{error}");
                        continue;
                    }
                    let mut sub_tokenizer = ZoneFileReader::new(&buffer);

                    // If defined, set the origin for the sub-tokenizer to the one provided.
                    match domain_name {
                        Some(origin) => {
                            let origin = origin.to_string();
                            sub_tokenizer.set_origin(origin.as_str());
                            self.load_from_tokenizer(sub_tokenizer, authoritative)
                        }
                        None => self.load_from_tokenizer(sub_tokenizer, authoritative),
                    }
                }
                Err(error) => println!("{error}"),
            }
        }
    }

    #[inline]
    fn load_from_string(&mut self, string: &str, authoritative: MetaAuth) {
        self.load_from_tokenizer(ZoneFileReader::new(string), authoritative)
    }

    #[inline]
    fn load_from_file(
        &mut self,
        file: &mut std::fs::File,
        authoritative: MetaAuth,
    ) -> io::Result<()> {
        let mut buffer = String::new();
        file.read_to_string(&mut buffer)?;
        self.load_from_string(&buffer, authoritative);
        Ok(())
    }
}

#[async_trait]
pub trait AsyncMainCache {
    async fn get(&self, query: &CacheQuery) -> CacheResponse;
    async fn insert_record(&self, record: CacheRecord);
    async fn insert_stream(&self, records: impl Stream<Item = CacheRecord> + Send) {
        records
            .for_each_concurrent(None, |record| self.insert_record(record))
            .await;
    }
    async fn insert_iter(&self, records: impl Iterator<Item = CacheRecord> + Send) {
        self.insert_stream(futures::stream::iter(records)).await;
    }
    async fn clean(&self);

    #[inline]
    async fn load_from_tokenizer<'a>(
        &self,
        tokenizer: ZoneFileReader<'a>,
        authoritative: MetaAuth,
    ) {
        let insertion_time = Instant::now();
        let meta = CacheMeta {
            auth: authoritative,
            insertion_time,
        };
        futures::stream::iter(tokenizer)
            .for_each_concurrent(None, |token| {
                let meta = meta.clone();
                async move {
                    match token {
                        Ok(ZoneToken::ResourceRecord(record)) => {
                            self.insert_record(CacheRecord { meta, record }).await
                        }
                        Ok(ZoneToken::Include {
                            file_path,
                            domain_name,
                        }) => {
                            // Read in the file and store it in the buffer. The buffer will be the feed for
                            // the sub-tokenizer
                            let mut buffer = String::new();
                            let mut file = match tokio::fs::File::open(file_path).await {
                                Ok(file) => file,
                                Err(error) => {
                                    println!("{error}");
                                    return;
                                }
                            };
                            if let Err(error) = file.read_to_string(&mut buffer).await {
                                println!("{error}");
                                return;
                            }
                            let mut sub_tokenizer = ZoneFileReader::new(&buffer);

                            // If defined, set the origin for the sub-tokenizer to the one provided.
                            match domain_name {
                                Some(origin) => {
                                    let origin = origin.to_string();
                                    sub_tokenizer.set_origin(origin.as_str());
                                    self.load_from_tokenizer(sub_tokenizer, authoritative).await
                                }
                                None => {
                                    self.load_from_tokenizer(sub_tokenizer, authoritative).await
                                }
                            }
                        }
                        Err(error) => println!("{error}"),
                    }
                }
            })
            .await;
    }

    #[inline]
    async fn load_from_string(&self, string: &str, authoritative: MetaAuth) {
        self.load_from_tokenizer(ZoneFileReader::new(string), authoritative)
            .await
    }

    #[inline]
    async fn load_from_file(
        &self,
        file: &mut tokio::fs::File,
        authoritative: MetaAuth,
    ) -> io::Result<()> {
        let mut buffer = String::new();
        file.read_to_string(&mut buffer).await?;
        self.load_from_string(&buffer, authoritative).await;
        Ok(())
    }
}
