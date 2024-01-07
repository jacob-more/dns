#!/bin/bash -e

./get-port-assignments.sh > port-assignments
cat port-assignments | python port-assignments-to-rust-match.py > port_from_service.rs
mv port_from_service.rs ../dns-lib/src/resource_record/port_from_service.rs

rm port-assignments
