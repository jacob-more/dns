use std::net::SocketAddr;

use dns_lib::{query::message::Message, serde::wire::{write_wire::WriteWire, compression_map::CompressionMap, to_wire::ToWire, read_wire::ReadWire, from_wire::FromWire}};
use tokio::{net::UdpSocket, io};

const MAX_MESSAGE_SIZE: usize = 4096;

pub async fn write_udp_message(udp_socket: &UdpSocket, message: &Message) -> io::Result<()> {
    // Step 1: Serialize Data
    let raw_message = &mut [0_u8; MAX_MESSAGE_SIZE];
    let mut raw_message = WriteWire::from_bytes(raw_message);
    if let Err(wire_error) = message.to_wire_format(&mut raw_message, &mut Some(CompressionMap::new())) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, wire_error));
    };
    let wire_length = raw_message.len();

    // Step 2: Bounds check against the configurations.
    //  TODO: No configuration options have been defined yet.

    // Step 3: Send the message via UDP.
    let bytes_written = udp_socket.send(raw_message.current_state()).await?;
    // Verify that the correct number of bytes were sent.
    if bytes_written != wire_length {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Incorrect number of bytes sent to UDP socket; Expected {wire_length} bytes but sent {bytes_written} bytes"),
        ));
    }

    return Ok(());
}

pub async fn read_udp_message(udp_socket: &UdpSocket) -> io::Result<(Message, SocketAddr)> {
    // Step 1: Setup buffer. Make sure it is within the configured size.
    let mut buffer = [0; MAX_MESSAGE_SIZE];
    let mut buffer = &mut buffer[..MAX_MESSAGE_SIZE];

    // Step 2: Get the bytes from the UDP socket.
    let (received_byte_count, peer) = udp_socket.recv_from(&mut buffer).await?;

    // Step 3: Deserialize the Message received on UDP socket.
    let mut wire = ReadWire::from_bytes(&mut buffer[..received_byte_count]);
    let message = match Message::from_wire_format(&mut wire) {
        Ok(message) => message,
        Err(wire_error) => return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            wire_error,
        )),
    };

    return Ok((message, peer));
}
