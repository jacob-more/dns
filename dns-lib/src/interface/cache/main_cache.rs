use std::io::{Read, self};

use async_trait::async_trait;
use tokio::io::AsyncReadExt;
use ux::u3;

use crate::{query::{message::Message, qr::QR}, serde::presentation::read_resource_records::ResourceRecordReader, resource_record::{opcode::OpCode, rcode::RCode}};

pub trait MainCache {
    fn get(&self, query: &Message) -> Message;
    fn insert(&mut self, records: &Message);
    fn clean(&mut self);

    #[inline]
    fn load_from_tokenizer(&mut self, tokenizer: ResourceRecordReader) {
        for token in tokenizer {
            match token {
                Ok(record) => self.insert(&Message {
                    id: 0,
                    qr: QR::Response,
                    opcode: OpCode::Query,
                    authoritative_answer: false,
                    truncation: false,
                    recursion_desired: false,
                    recursion_available: false,
                    z: u3::new(0),
                    rcode: RCode::NoError,
                    question: vec![],
                    answer: vec![record],
                    authority: vec![],
                    additional: vec![],
                }),
                Err(error) => println!("{error}"),
            }
        }
    }

    #[inline]
    fn load_from_string(&mut self, string: &str) {
        self.load_from_tokenizer(ResourceRecordReader::new(&string))
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
    async fn load_from_tokenizer<'a>(&self, tokenizer: ResourceRecordReader<'a>) {
        for token in tokenizer {
            match token {
                Ok(record) => {
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
                        question: vec![],
                        answer: vec![record],
                        authority: vec![],
                        additional: vec![],
                    };
                    self.insert(&message).await
                },
                Err(error) => println!("{error}"),
            }
        }
    }

    #[inline]
    async fn load_from_string(&self, string: &str) {
        self.load_from_tokenizer(ResourceRecordReader::new(&string)).await
    }

    #[inline]
    async fn load_from_file(&self, file: &mut tokio::fs::File) -> io::Result<()> {
        let mut buffer = String::new();
        file.read_to_string(&mut buffer).await?;
        self.load_from_string(&buffer).await;
        Ok(())
    }
}
