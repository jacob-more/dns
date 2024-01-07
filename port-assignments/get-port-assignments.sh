#!/bin/bash -e

# Parses the file hosted by IANA that list out the port assignments. It reads for the
# {service,port,protocol} groups (ignoring those that don't have a defined port). They are output in
# a comma-separated format that can be easily read in later.
# 
# In the future, this could be switched to a file that parses the XML version of the file.

IANA_PORTS="https://www.iana.org/assignments/service-names-port-numbers/service-names-port-numbers.csv"

# Downloads the the list of port assignments from IANA and outputs the contents to stdout.
function download_ports_file () {
    wget -q -O - $IANA_PORTS
}

# Cuts out the fields from the csv (from stdin) that are not the port or the name of the service.
# Outputs to stdout.
function select_named_ports () {
    cut -s -d, -f1-3 | cut -d\n -f1 | grep -E "^([^ \(\)]+),([0-9]+),([^ ]+)$"
}

# we only want to split at new lines. Not spaces.
IFS=$'\n'
for line in $(download_ports_file | select_named_ports);
do
    echo "$line"
done
