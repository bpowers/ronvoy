#!/usr/bin/env bash
set -euo pipefail
set -x

if [[ $# -ne 1 ]]; then
  echo "usage: $0 HOSTNAME" >&2
  exit 1
fi

export PATH="/opt/homebrew/Cellar/openssl@3/3.0.1/bin:$PATH"

hostname="$1"
key="${hostname}.key"
csry="${hostname}.csr"
crt="${hostname}.crt"

organization="Very Good Building Company, LLC"
email="ron@verygoodbuildingcompany.llc"

if [ ! -e ca.crt ]; then
  echo "generating ca.crt"
  cat >ca.conf <<EOF
[ req ]
prompt = no
distinguished_name = ca.verygoodbuildingcompany.llc

[ ca ]
default_ca      = ca_default

[ ca_default ]
new_certs_dir    = .
database         = certs.db
default_md       = sha256
preserve         = no
policy           = policy_anything
RANDFILE         = .rand
email_in_dn      = no
rand_serial      = yes
unique_subject   = no
name_opt         = ca_default
cert_opt         = ca_default
copy_extensions  = copy
default_days     = 365
default_crl_days = 30
private_key      = ca.key
certificate      = ca.crt

[ v3_ca ]
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid:always,issuer
basicConstraints = critical,CA:true


[ policy_anything ]
countryName             = optional
stateOrProvinceName     = optional
localityName            = optional
organizationName        = optional
organizationalUnitName  = optional
commonName              = supplied
emailAddress            = optional


[ ca.verygoodbuildingcompany.llc ]
CN = ca.verygoodbuildingcompany.llc
countryName = US
stateOrProvinceName = Indiana
localityName = Pawnee
organizationName = $organization
EOF

  touch certs.db
  openssl ecparam -name prime256v1 -param_enc named_curve -genkey -noout -out ca.key
  openssl req -new -key ca.key -out ca.csr -config ./ca.conf
  openssl ca -create_serial -out ca.crt -days 365 -keyfile ca.key -selfsign -extensions v3_ca -config ./ca.conf -infiles ca.csr
  # in addition to ca.crt, openssl seems to generate e.g. 5FA764F32D9C1DA7420130519AE5B27689598688.pem.  Delete that extra file
  sha1sum * | grep ca.crt | cut -d ' ' -f 1 | while read hash; do
    sha1sum * | grep "$hash" | grep -v ca.crt | cut -d ' ' -f 3 | xargs rm
  done
fi

sign_key() {
  local key="$1"
  local csr="$2"
  local crt="$3"

  cat >cert.config <<EOF
[ req ]
prompt = no
distinguished_name = req_distinguished_name
req_extensions = v3_req

[ req_distinguished_name ]
O = $organization
CN = $hostname
emailAddress = $email

[ v3_req ]
basicConstraints = CA:FALSE
keyUsage = digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth, clientAuth
subjectAltName = @alt_names

[ alt_names ]
DNS.1 = $hostname

EOF

  openssl req -new -key "$key" -out "$csr" -config cert.config
  openssl ca -config ./ca.conf -in "$csr" -out "$crt"
  openssl x509 -in "$crt" -text -noout | grep 'Subject:'
}

alg='ecdsa-p256'
key="${hostname}.${alg}.key"
csr="${hostname}.${alg}.csr"
crt="${hostname}.${alg}.crt"

openssl ecparam -name prime256v1 -param_enc named_curve -genkey -noout -out "$key"
sign_key "$key" "$csr" "$crt"

rm -f *.csr
