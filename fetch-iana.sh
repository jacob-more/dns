#!/bin/bash -e

# Gets an updated version of the file "root.hints" from IANA. This file is used for boot-strapping
# the DNS.

wget -q -O "root.hints" "https://www.internic.net/domain/named.root"

# Gets an updated version of the file "root.zone" from IANA. This file can be used for
# boot-strapping the DNS.

wget -q -O "root.zone" "https://www.internic.net/domain/root.zone"

# Gets an updated version of the file "port-assignments.xml" from IANA. This file is used for
# converting well-known-services into their respective port numbers. By using a configurable file,
# custom ports can be added as needed.

wget -q -O "port-assignments.xml" "https://www.iana.org/assignments/service-names-port-numbers/service-names-port-numbers.xml"
