#!/bin/bash -e

# Gets an updated version of the file "root.hints" from IANA. This file is used for boot-strapping
# the DNS.

wget -q -O "root.hints" "https://www.internic.net/domain/named.root"

# Gets an updated version of the file "root.zone" from IANA. This file can be used for
# boot-strapping the DNS.

wget -q -O "root.zone" "https://www.internic.net/domain/root.zone"
