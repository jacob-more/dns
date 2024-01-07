#!/bin/python

# Parses the {service,port,protocol} groups output by the script "get-port-assignments.sh" into a
# hard-coded file that lists out all the groups so that they can be easily read by a program. To run
# script, redirect the output from the script "get-port-assignments.sh" into this file and direct
# the output into a Rust file.

import sys;

class Service:
    def __init__(self, name, protocol, port) -> None:
        self.name = name
        self.protocol = protocol
        self.ports = {port}

    def add_port(self, port):
        self.ports.add(port)


if __name__ == "__main__":
    # From stdin, read and parse out all the services and their associated ports/protocols
    services = dict()
    for input_line in sys.stdin.readlines():
        (service, port, protocol) = input_line.strip().split(',', maxsplit=2)
        service = service.lower()
        protocol = protocol.lower()
        if (service, protocol) in services:
            services[(service, protocol)].add_port(port)
        else:
            services[(service, protocol)] = Service(service, protocol, port)

    # Once everything has been read in, output the same data, but grouped:
    # {service,protocol,port,port,...,port} etc. There will be one line in the file per
    # (service,protocol) pair.
    print("use super::{protocol::Protocol, ports::PortError};")
    print()
    print("#[inline]")
    print("pub fn port_from_service(service: &str, protocol: &Protocol) -> Result<&'static [u16], PortError> {")
    print("    match (service.to_lowercase().as_str(), protocol.mnemonic().to_lowercase().as_str()) {")
    for service in services.values():
        print("        (\"{0}\", \"{1}\") => Ok(&[{2}]),".format(service.name, service.protocol, ", ".join(service.ports)))
    print("        _ => Err(PortError::UnknownMnemonic(service.to_string(), protocol.clone()))")
    print("    }")
    print("}")
