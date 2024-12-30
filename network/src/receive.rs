use dns_lib::{query::message::Message, serde::wire::{from_wire::FromWire, read_wire::ReadWire}};
use tokio::{io::AsyncReadExt, net::UdpSocket};

use crate::errors;


#[inline]
pub async fn read_udp_message<const BUFFER_SIZE: usize>(udp_socket: &UdpSocket) -> Result<Message, errors::UdpReceiveError> {
    debug_assert!(u16::MAX as usize <= BUFFER_SIZE);

    // Step 1: Setup buffer. Make sure it is within the configured size.
    let mut buffer = [0; BUFFER_SIZE];
    // TODO: bound buffer based on configuration

    // Step 2: Get the bytes from the UDP socket.
    let received_byte_count = udp_socket.recv(&mut buffer).await?;

    // Step 3: Deserialize the Message received on UDP socket.
    let mut wire = ReadWire::from_bytes(&buffer[..received_byte_count]);
    let message = Message::from_wire_format(&mut wire)?;

    return Ok(message);
}

#[inline]
pub async fn read_stream_message<const BUFFER_SIZE: usize>(tcp_stream: &mut (impl AsyncReadExt + Unpin)) -> Result<Message, errors::StreamReceiveError> {
    debug_assert!(u16::MAX as usize <= BUFFER_SIZE);

    // Step 1: Deserialize the u16 representing the size of the rest of the data. This is the first
    //         2 bytes of data.
    let mut wire_size = [0, 0];
    match tcp_stream.read_exact(&mut wire_size).await {
        Ok(bytes_read) => {
            if bytes_read != 2 {
                return Err(errors::StreamReceiveError::IncorrectNumberBytes {
                    stream_protocol: "TCP",
                    expected: 2,
                    received: bytes_read,
                });
            }
        },
        Err(io_error) => {
            return Err(errors::StreamReceiveError::Io {
                stream_protocol: "TCP",
                error: io_error.into(),
            });
        },
    };

    let expected_message_size = u16::from_be_bytes(wire_size);
    if expected_message_size > (BUFFER_SIZE as u16) {
        return Err(errors::StreamReceiveError::IncorrectLengthByte {
            stream_protocol: "TCP",
            limit: BUFFER_SIZE as u16,
            received: expected_message_size,
        });
    }

    // Step 2: Read the rest of the packet.
    // Note: It MUST be the size of the previous u16 (expected_message_size).
    let mut tcp_buffer = [0; BUFFER_SIZE];
    // TODO: bound tcp_buffer based on configuration
    match tcp_stream.read_exact(&mut tcp_buffer[..expected_message_size as usize]).await {
        Ok(bytes_read) => {
            if bytes_read != (expected_message_size as usize) {
                return Err(errors::StreamReceiveError::IncorrectNumberBytes {
                    stream_protocol: "TCP",
                    expected: expected_message_size,
                    received: bytes_read,
                });
            }
        },
        Err(io_error) => {
            return Err(errors::StreamReceiveError::Io {
                stream_protocol: "TCP",
                error: io_error.into(),
            });
        },
    }

    // Step 3: Deserialize the Message from the buffer.
    let mut wire = ReadWire::from_bytes(&mut tcp_buffer[..expected_message_size as usize]);
    match Message::from_wire_format(&mut wire) {
        Ok(message) => Ok(message),
        Err(read_wire_error) => Err(errors::StreamReceiveError::Deserialization {
            stream_protocol: "TCP",
            error: read_wire_error,
        }),
    }
}