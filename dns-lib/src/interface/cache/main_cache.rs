use std::{io::{Read, self}, fs::File};

use async_trait::async_trait;
use tinyvec::tiny_vec;
use tokio::io::AsyncReadExt;
use ux::u3;

use crate::{query::{message::Message, qr::QR}, serde::presentation::zone_file_reader::{ZoneFileReader, ZoneToken}, resource_record::{opcode::OpCode, rcode::RCode}};

pub trait MainCache {
    fn get(&self, query: &Message) -> Message;
    fn insert(&mut self, records: &Message);
    fn clean(&mut self);

    #[inline]
    fn load_from_tokenizer(&mut self, tokenizer: ZoneFileReader) {
        for token in tokenizer {
            match token {
                Ok(ZoneToken::ResourceRecord(record)) => self.insert(&Message {
                    id: 0,
                    qr: QR::Response,
                    opcode: OpCode::Query,
                    authoritative_answer: false,
                    truncation: false,
                    recursion_desired: false,
                    recursion_available: false,
                    z: u3::new(0),
                    rcode: RCode::NoError,
                    question: tiny_vec![],
                    answer: vec![record],
                    authority: vec![],
                    additional: vec![],
                }),
                Ok(ZoneToken::Include { file_path, domain_name }) => {
                    // Read in the file and store it in the buffer. The buffer will be the feed for
                    // the sub-tokenizer
                    let mut buffer = String::new();
                    let mut file = match File::open(file_path) {
                        Ok(file) => file,
                        Err(error) => {
                            println!("{error}");
                            continue;
                        },
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
                            self.load_from_tokenizer(sub_tokenizer)
                        },
                        None => self.load_from_tokenizer(sub_tokenizer),
                    }
                },
                Err(error) => println!("{error}"),
            }
        }
    }

    #[inline]
    fn load_from_string(&mut self, string: &str) {
        self.load_from_tokenizer(ZoneFileReader::new(&string))
    }

    #[inline]
    fn load_from_file(&mut self, file: &mut std::fs::File) -> io::Result<()> {
        let mut buffer = String::new();
        file.read_to_string(&mut buffer)?;
        self.load_from_string(&buffer);
        Ok(())
    }
}

#[async_trait]
pub trait AsyncMainCache {
    async fn get(&self, query: &Message) -> Message;
    async fn insert(&self, records: &Message);
    async fn clean(&self);

    #[inline]
    async fn load_from_tokenizer<'a>(&self, tokenizer: ZoneFileReader<'a>) {
        for token in tokenizer {
            match token {
                Ok(ZoneToken::ResourceRecord(record)) => {
                    let message = Message {
                        id: 0,
                        qr: QR::Response,
                        opcode: OpCode::Query,
                        authoritative_answer: false,
                        truncation: false,
                        recursion_desired: false,
                        recursion_available: false,
                        z: u3::new(0),
                        rcode: RCode::NoError,
                        question: tiny_vec![],
                        answer: vec![record],
                        authority: vec![],
                        additional: vec![],
                    };
                    self.insert(&message).await
                },
                Ok(ZoneToken::Include { file_path, domain_name }) => {
                    // Read in the file and store it in the buffer. The buffer will be the feed for
                    // the sub-tokenizer
                    let mut buffer = String::new();
                    let mut file = match tokio::fs::File::open(file_path).await {
                        Ok(file) => file,
                        Err(error) => {
                            println!("{error}");
                            continue;
                        },
                    };
                    if let Err(error) = file.read_to_string(&mut buffer).await {
                        println!("{error}");
                        continue;
                    }
                    let mut sub_tokenizer = ZoneFileReader::new(&buffer);

                    // If defined, set the origin for the sub-tokenizer to the one provided.
                    match domain_name {
                        Some(origin) => {
                            let origin = origin.to_string();
                            sub_tokenizer.set_origin(origin.as_str());
                            self.load_from_tokenizer(sub_tokenizer).await
                        },
                        None => self.load_from_tokenizer(sub_tokenizer).await,
                    }
                },
                Err(error) => println!("{error}"),
            }
        }
    }

    #[inline]
    async fn load_from_string(&self, string: &str) {
        self.load_from_tokenizer(ZoneFileReader::new(&string)).await
    }

    #[inline]
    async fn load_from_file(&self, file: &mut tokio::fs::File) -> io::Result<()> {
        let mut buffer = String::new();
        file.read_to_string(&mut buffer).await?;
        self.load_from_string(&buffer).await;
        Ok(())
    }
}
