#!/bin/bash -e

wget -q -O "port-assignments" "https://www.iana.org/assignments/service-names-port-numbers/service-names-port-numbers.xml"
python port-assignments-to-rust-match.py > port_from_service.rs
mv port_from_service.rs ../dns-lib/src/resource_record/port_from_service.rs

rm port-assignments
