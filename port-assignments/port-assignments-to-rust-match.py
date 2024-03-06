#!/bin/python

# Parses the {service,port,protocol} groups output by the script "get-port-assignments.sh" into a
# hard-coded file that lists out all the groups so that they can be easily read by a program. To run
# script, redirect the output from the script "get-port-assignments.sh" into this file and direct
# the output into a Rust file.

import sys
from typing import Optional, Tuple;
from defusedxml.ElementTree import parse


class Service:
    def __init__(self, name, protocol, port) -> None:
        self.name = name
        self.protocol = protocol
        self.ports = {port}

    def add_port(self, port):
        self.ports.add(port)


def parse_record(record) -> Optional[Tuple[str, str, str]]:
    name = record.find("{http://www.iana.org/assignments}name")
    if (name is None) or (name.text is None):
        return None
    protocol = record.find("{http://www.iana.org/assignments}protocol")
    if (protocol is None) or (protocol.text is None):
        return None
    port = record.find("{http://www.iana.org/assignments}number")
    if (port is None) or (port.text is None):
        return None
    return name.text.lower(), protocol.text.lower(), port.text.lower()


if __name__ == "__main__":
    # From stdin, read and parse out all the services and their associated ports/protocols
    services = dict()
    xml_file = open("./port-assignments")
    element_tree = parse(xml_file, forbid_dtd=True, forbid_entities=True, forbid_external=True)
    for child in element_tree.getroot():
        if child.tag != "{http://www.iana.org/assignments}record":
            continue
        data = parse_record(child)
        if data is None:
            continue
        (service, protocol, port) = data
        # Case 1: The port is given as a range.
        if len(port.split("-")) != 1:
            lower, upper = port.split("-")
            lower = int(lower)
            upper = int(upper)
            for port in range(lower, upper + 1):
                if (service, protocol) in services:
                    services[(service, protocol)].add_port(str(port))
                else:
                    services[(service, protocol)] = Service(service, protocol, str(port))
        # Case 2: The port is given as a number
        else:
            if (service, protocol) in services:
                services[(service, protocol)].add_port(port)
            else:
                services[(service, protocol)] = Service(service, protocol, port)

    # Quickly go back through and sort the order that ports are listed.
    for service in services.values():
        service.ports = sorted(list(service.ports))

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
