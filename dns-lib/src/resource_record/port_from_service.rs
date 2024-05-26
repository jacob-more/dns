use super::{ports::PortError, protocol::Protocol};

use std::{collections::HashMap, fs::File, io::{self, BufReader, Read}, time::Instant};

use lazy_static::lazy_static;
use xml::{reader::XmlEvent, EventReader, ParserConfig};


const RECORD_LOCAL_NAME: &'static str = "record";
const NAME_LOCAL_NAME: &'static str = "name";
const PROTOCOL_LOCAL_NAME: &'static str = "protocol";
const NUMBER_LOCAL_NAME: &'static str = "number";

fn parse_protocol<R>(parser: &mut EventReader<R>) -> io::Result<Option<Protocol>> where R: Read {
    let mut protocol = None;

    loop {
        let event = parser.next();
        match event {
            Ok(XmlEvent::EndDocument) => return Err(io::Error::from(io::ErrorKind::UnexpectedEof)),
            Ok(XmlEvent::StartElement { name, attributes: _, namespace: _ }) => return Err(io::Error::new(io::ErrorKind::InvalidData, format!("The protocol entry cannot have another nested entry. Found protocol entry named '{}'", name.local_name))),
            Ok(XmlEvent::Characters(characters)) => {
                match protocol {
                    Some(previous_protocol) => return Err(io::Error::new(io::ErrorKind::InvalidData, format!("The protocol entry was found twice in a single record. Can only appear once. First protocol define was '{previous_protocol}'. Second was '{characters}'"))),
                    None => {
                        protocol = match Protocol::from_str(&characters.to_uppercase()) {
                            Ok(protocol) => Some(protocol),
                            Err(protocol_error) => return Err(io::Error::new(io::ErrorKind::InvalidData, protocol_error.to_string())),
                        };
                    },
                }
            },
            Ok(XmlEvent::EndElement { name }) => {
                if name.local_name != PROTOCOL_LOCAL_NAME {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, format!("A protocol entry was closed with a '{}' tag", name.local_name)));
                }
                break;
            },
            Ok(_) => (),
            Err(error) => return Err(io::Error::new(io::ErrorKind::Other, error)),
        }
    }
    
    return Ok(protocol);
}

fn parse_name<R>(parser: &mut EventReader<R>) -> io::Result<Option<String>> where R: Read {
    let mut name = None;

    loop {
        let event = parser.next();
        match event {
            Ok(XmlEvent::EndDocument) => return Err(io::Error::from(io::ErrorKind::UnexpectedEof)),
            Ok(XmlEvent::StartElement { name, attributes: _, namespace: _ }) => return Err(io::Error::new(io::ErrorKind::InvalidData, format!("The name entry cannot have another nested entry. Found name entry named '{}'", name.local_name))),
            Ok(XmlEvent::Characters(characters)) => {
                match name {
                    Some(previous_name) => return Err(io::Error::new(io::ErrorKind::InvalidData, format!("The name entry was found twice in a single record. Can only appear once. First named '{previous_name}'. Second named '{characters}'"))),
                    None => name = Some(characters.trim().to_lowercase()),
                }
            },
            Ok(XmlEvent::EndElement { name }) => {
                if name.local_name != NAME_LOCAL_NAME {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, format!("A name entry was closed with a '{}' tag", name.local_name)));
                }
                break;
            },
            Ok(_) => (),
            Err(error) => return Err(io::Error::new(io::ErrorKind::Other, error)),
        }
    }
    
    return Ok(name);
}

fn parse_ports<R>(parser: &mut EventReader<R>) -> io::Result<Option<Vec<u16>>> where R: Read {
    let mut ports = None;

    loop {
        let event = parser.next();
        match event {
            Ok(XmlEvent::EndDocument) => return Err(io::Error::from(io::ErrorKind::UnexpectedEof)),
            Ok(XmlEvent::StartElement { name, attributes: _, namespace: _ }) => return Err(io::Error::new(io::ErrorKind::InvalidData, format!("The port (number) entry cannot have another nested entry. Found port (number) entry named '{}'", name.local_name))),
            Ok(XmlEvent::Characters(characters)) => {
                match ports {
                    Some(_) => return Err(io::Error::new(io::ErrorKind::InvalidData, format!("The port (number) entry was found twice in a single record. Can only appear once. Redefined with ports '{characters}'"))),
                    None => {
                        match characters.split("-").collect::<Vec<&str>>().as_slice() {
                            &[port] => {
                                let port = match u16::from_str_radix(port, 10) {
                                    Ok(port) => port,
                                    Err(int_error) => return Err(io::Error::new(io::ErrorKind::InvalidData, int_error)),
                                };
                                ports = Some(vec![port]);
                            },
                            &[lower_bound, upper_bound] => {
                                let lower_bound = match u16::from_str_radix(lower_bound, 10) {
                                    Ok(lower_bound) => lower_bound,
                                    Err(int_error) => return Err(io::Error::new(io::ErrorKind::InvalidData, int_error)),
                                };
                                let upper_bound = match u16::from_str_radix(upper_bound, 10) {
                                    Ok(upper_bound) => upper_bound,
                                    Err(int_error) => return Err(io::Error::new(io::ErrorKind::InvalidData, int_error)),
                                };
                                if lower_bound > upper_bound {
                                    return Err(io::Error::new(io::ErrorKind::InvalidData, format!("The port (number) entry has a lower bound that is greater than the upper bound ({lower_bound} > {upper_bound}). Found '{characters}'. The lower bound must be at most equal to the upper bound")));
                                }
                                ports = Some((lower_bound..=upper_bound).collect());
                            },
                            _ => return Err(io::Error::new(io::ErrorKind::InvalidData, format!("The port (number) must have either a single non-negative integer ('\\d+') or a range formatted as '\\d+-\\d+'. Found '{characters}'"))),
                        }
                    },
                }
            },
            Ok(XmlEvent::EndElement { name }) => {
                if name.local_name != NUMBER_LOCAL_NAME {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, format!("A port (number) entry was closed with a '{}' tag", name.local_name)));
                }
                break;
            },
            Ok(_) => (),
            Err(error) => return Err(io::Error::new(io::ErrorKind::Other, error)),
        }
    }
    
    return Ok(ports);
}

fn parse_record<R>(parser: &mut EventReader<R>) -> io::Result<Option<(String, Protocol, Vec<u16>)>> where R: Read {
    let mut record_name = None;
    let mut record_protocol = None;
    let mut record_ports = None;

    loop {
        let event = parser.next();
        match event {
            Ok(XmlEvent::EndDocument) => return Err(io::Error::from(io::ErrorKind::UnexpectedEof)),
            Ok(XmlEvent::StartElement { name, attributes: _, namespace: _ }) => {
                match name.local_name.as_str() {
                    NAME_LOCAL_NAME => match (record_name, parse_name(parser)) {
                        (Some(_), _) => return Err(io::Error::new(io::ErrorKind::InvalidData, format!("The name entry was found twice within a record. It can only appear once."))),
                        (None, Ok(name)) => record_name = name,
                        (None, Err(error)) => return Err(error),
                    },
                    PROTOCOL_LOCAL_NAME => match (record_protocol, parse_protocol(parser)) {
                        (Some(_), _) => return Err(io::Error::new(io::ErrorKind::InvalidData, format!("The protocol entry was found twice within a record. It can only appear once."))),
                        (None, Ok(protocol)) => record_protocol = protocol,
                        (None, Err(error)) => return Err(error),
                    },
                    NUMBER_LOCAL_NAME => match (record_ports, parse_ports(parser)) {
                        (Some(_), _) => return Err(io::Error::new(io::ErrorKind::InvalidData, format!("The port (number) entry was found twice within a record. It can only appear once."))),
                        (None, Ok(ports)) => record_ports = ports,
                        (None, Err(error)) => return Err(error),
                    },
                    _ => (),
                }
            },
            Ok(XmlEvent::EndElement { name }) => {
                if name.local_name != RECORD_LOCAL_NAME {
                    continue;
                }
                break;
            },
            Ok(_) => (),
            Err(error) => return Err(io::Error::new(io::ErrorKind::Other, error)),
        }
    }

    match (record_name, record_protocol, record_ports) {
        (None, _, _) => return Ok(None),
        (_, None, _) => return Ok(None),
        (_, _, None) => return Ok(None),
        (Some(name), Some(protocol), Some(ports)) => return Ok(Some((name, protocol, ports))),
    }
}

fn load_port_service_map() -> io::Result<HashMap<(String, Protocol), Vec<u16>>> {
    let start_time = Instant::now();

    let file = BufReader::new(File::open("./port-assignments.xml")?);

    let config = ParserConfig::new()
        .trim_whitespace(true)
        .cdata_to_characters(true)
        .ignore_comments(true)
        .coalesce_characters(true)
        // If we ever see the depth go past 5, the file format has changed and we need to review the
        // changes.
        .max_entity_expansion_depth(5);

    let mut parser = EventReader::new_with_config(file, config);

    let mut port_service_map: HashMap<(String, Protocol), Vec<u16>> = HashMap::new();

    loop {
        let event = parser.next();
        match event {
            Ok(event) => match event {
                XmlEvent::StartDocument { version: _, encoding: _, standalone: _ } => (),
                XmlEvent::EndDocument => break,
                XmlEvent::ProcessingInstruction { name: _, data: _ } => (),
                XmlEvent::StartElement { name, attributes: _, namespace: _ } => match name.local_name.as_str() {
                    RECORD_LOCAL_NAME => match parse_record(&mut parser) {
                        Ok(Some((name, protocol, ports))) => {
                            if let Some(stored_ports) = port_service_map.get_mut(&(name.clone(), protocol.clone())) {
                                stored_ports.extend(ports)
                            } else {
                                port_service_map.insert((name, protocol), ports);
                            }
                        },
                        Ok(None) => (),
                        Err(error) => println!("Failed to parse port: {error}"),
                    },
                    _ => (),
                },
                _ => (),
            },
            Err(error) => return Err(io::Error::new(io::ErrorKind::Other, error)),
        }
    }

    let end_time = Instant::now();
    let total_duration = end_time - start_time;
    println!("Loading Port Service Mappings took {} ms", total_duration.as_millis());
    
    Ok(port_service_map)
}

lazy_static! {
    pub static ref PORT_SERVICE_MAP: HashMap<(String, Protocol), Vec<u16>> = load_port_service_map().unwrap();
}

#[inline]
pub fn port_from_service(service: String, protocol: Protocol) -> Result<&'static [u16], PortError> {
    match PORT_SERVICE_MAP.get(&(service.clone(), protocol.clone())) {
        Some(ports) => Ok(ports),
        None => Err(PortError::UnknownMnemonic(service, protocol)),
    }
}
