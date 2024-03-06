#!/bin/python

# Parses the {service,port,protocol} groups output by the script "get-port-assignments.sh" into a
# hard-coded file that lists out all the groups so that they can be easily read by a program. To run
# script, redirect the output from the script "get-port-assignments.sh" into this file and direct
# the output into a Rust file.

from typing import Dict, List, Optional, Set, Tuple, Union;
from defusedxml.ElementTree import parse


TAG_PREFIX = "{http://www.iana.org/assignments}"

TAG_RECORD   = f"{TAG_PREFIX}record"
TAG_NAME     = f"{TAG_PREFIX}name"
TAG_PROTOCOL = f"{TAG_PREFIX}protocol"
TAG_PORT     = f"{TAG_PREFIX}number"

PORT_FILE = "./port-assignments"


class Service:
    def __init__(self, name: str, protocol: str, port: int) -> None:
        self.name: str = name
        self.protocol: str = protocol
        self.ports: Union[Set[int], List[int]] = {port}

    def add_port(self, port: int):
        self.ports.add(port)

    def finalize(self):
        # Convert the ports from sets to lists, and make sure they are sorted numerically, least to
        # greatest.
        port_list = list(service.ports)
        sorted_port_list = sorted(port_list)
        # Then, convert them to strings
        str_sorted_port_list = map(lambda port: str(port), sorted_port_list)
        service.ports = str_sorted_port_list


def parse_record(record) -> Optional[Tuple[str, str, str]]:
    name = record.find(TAG_NAME)
    if (name is None) or (name.text is None):
        return None

    protocol = record.find(TAG_PROTOCOL)
    if (protocol is None) or (protocol.text is None):
        return None

    port = record.find(TAG_PORT)
    if (port is None) or (port.text is None):
        return None

    return name.text.lower(), protocol.text.lower(), port.text.lower()


def verify_port(port: str) -> int:
    try:
        int_port = int(port)
        if int_port < 0:
            raise ValueError(f"Port too small. Expected port to either be a number (i.e. sequence of digits), in the range 0 - 65,535 (inclusive) but it was '{int_port}'")
        elif int_port >= 2 ** 16:
            raise ValueError(f"Port too large. Expected port to either be a number (i.e. sequence of digits), in the range 0 - 65,535 (inclusive) but it was '{int_port}'")
        else:
            return int_port
    except:
        raise ValueError(f"Expected port to either be a number (i.e. sequence of digits), in the range 0 - 65,535 (inclusive) bit it was '{port}'")


if __name__ == "__main__":
    services: Dict[Tuple[str, str], Service] = dict()

    with open(PORT_FILE, mode="r") as xml_file:
        element_tree = parse(xml_file, forbid_dtd=True, forbid_entities=True, forbid_external=True)
        for child in element_tree.getroot():
            # Only parse records.
            if child.tag != TAG_RECORD:
                continue

            # The record must have all the required fields. Otherwise, we cannot create the match statement.
            record = parse_record(child)
            if record is None:
                continue

            (service, protocol, port) = record

            split_port = port.split("-")
            # Case 1: The port is given as a number
            if len(split_port) == 1:
                port = verify_port(port)
                if (service, protocol) in services:
                    services[(service, protocol)].add_port(port)
                else:
                    services[(service, protocol)] = Service(service, protocol, port)

            # Case 2: The port is given as a range.
            elif len(split_port) == 2:
                lower_port = verify_port(split_port[0])
                upper_port = verify_port(split_port[1])

                # Ensure that the upper port is the larger of the two.
                if lower_port > upper_port:
                    tmp = upper_port
                    upper_port = lower_port
                    lower_port = tmp

                for port in range(lower_port, upper_port + 1):
                    if (service, protocol) in services:
                        services[(service, protocol)].add_port(port)
                    else:
                        services[(service, protocol)] = Service(service, protocol, port)

            else:
                raise ValueError(f"Expected port to either be a number (i.e. sequence of digits) or a range (i.e. two sequences of digits separated by a dash '-'). Instead, port was '{port}'")


        # Once everything has been read in, output the same data, but grouped:
        # {service,protocol,port,port,...,port} etc. There will be one line in the file per
        # (service,protocol) pair.
        print("use super::{protocol::Protocol, ports::PortError};")
        print()
        print("#[inline]")
        print("pub fn port_from_service(service: &str, protocol: &Protocol) -> Result<&'static [u16], PortError> {")
        print("    match (service.to_lowercase().as_str(), protocol.mnemonic().to_lowercase().as_str()) {")
        for service in services.values():
            # First, ensure that the ports are sorted least to greatest.
            service.finalize()
            # Then, they are ready to be added to the match statement.
            print("        (\"{0}\", \"{1}\") => Ok(&[{2}]),".format(service.name, service.protocol, ", ".join(service.ports)))
        print("        _ => Err(PortError::UnknownMnemonic(service.to_string(), protocol.clone()))")
        print("    }")
        print("}")
