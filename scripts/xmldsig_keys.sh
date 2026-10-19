#!/usr/bin/env bash
# Genera material criptográfico EFÍMERO para las pruebas de xmldsig.
#
# Uso: scripts/xmldsig_keys.sh <directorio>
#
# El directorio debe ser temporal (mktemp -d) y borrarse al terminar. Ningún
# archivo generado se versiona ni se imprime en logs. Sujeto y vigencia de los
# certificados son fijos (deterministas); las claves cambian en cada ejecución.
set -euo pipefail

dir="${1:?uso: xmldsig_keys.sh <directorio>}"
openssl_bin="${OPENSSL_BIN:-/opt/homebrew/opt/openssl@3/bin/openssl}"
[[ -x "$openssl_bin" ]] || openssl_bin="$(command -v openssl)"
umask 077
mkdir -p "$dir"

# `req -not_before`/`-x509v1` existen desde OpenSSL 3.4. Con una CLI anterior
# (Ubuntu 24.04 trae 3.0) se usa el generador Rust equivalente de
# crates/xmldsig/tests/support/keys.rs, que produce el mismo juego.
if ! "$openssl_bin" req -help 2>&1 | grep -q -- '-not_before'; then
    root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
    XMLDSIG_WRITE_KEYS="$(cd "$dir" && pwd)" cargo test -q --manifest-path "$root/Cargo.toml" \
        -p cpeforge-xmldsig --test parity write_keys -- --ignored >/dev/null
    exit 0
fi

ext="$dir/ext.cnf"
cat > "$ext" <<'CNF'
[req]
distinguished_name = dn
prompt = no
[dn]
C = PE
O = EMPRESA SINTETICA S.A.C.
CN = 20000000001
[v3]
basicConstraints = critical, CA:FALSE
keyUsage = critical, digitalSignature, nonRepudiation, keyEncipherment, dataEncipherment
extendedKeyUsage = clientAuth, emailProtection
subjectKeyIdentifier = hash
CNF

make_identity() {
    local name="$1" not_before="$2" not_after="$3"
    "$openssl_bin" genpkey -quiet -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out "$dir/$name.key"
    "$openssl_bin" req -new -x509 -key "$dir/$name.key" -config "$ext" -extensions v3 \
        -set_serial 4660 -not_before "$not_before" -not_after "$not_after" -sha256 -out "$dir/$name.crt"
    # PEM combinado (clave + certificado), el formato que espera SignedXml.
    cat "$dir/$name.key" "$dir/$name.crt" > "$dir/$name.pem"
    "$openssl_bin" pkey -in "$dir/$name.key" -traditional -out "$dir/$name.rsa.pem"
    "$openssl_bin" pkey -in "$dir/$name.key" -pubout -out "$dir/$name.pub.pem"
}

make_identity a 20240101000000Z 20340101000000Z
make_identity b 20240101000000Z 20340101000000Z
make_identity expired 20200101000000Z 20210101000000Z

# Certificados con extensiones variadas para getPurposes()/getExtensions():
# claves EC P-256 efímeras; sujeto, extensiones y vigencia fijos.
cat > "$dir/ext-profiles.cnf" <<'CNF'
[req]
distinguished_name = dn
prompt = no
[dn]
C = PE
O = EMPRESA SINTETICA S.A.C.
CN = 20000000001
[ca]
basicConstraints = critical, CA:TRUE, pathlen:1
keyUsage = critical, keyCertSign, cRLSign
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid:always
[server]
keyUsage = digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth, clientAuth, 1.3.6.1.4.1.311.10.3.3, 1.3.6.1.4.1.55555.9
subjectAltName = @san
issuerAltName = email:emisor@ejemplo.test
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid, issuer:always
crlDistributionPoints = URI:http://crl.ejemplo.test/ca.crl
authorityInfoAccess = OCSP;URI:http://ocsp.ejemplo.test, caIssuers;URI:http://ca.ejemplo.test/ca.cer
certificatePolicies = 2.23.140.1.2.1, @pol
nsCertType = server, client
nsComment = "comentario sintetico"
1.3.6.1.4.1.55555.1 = ASN1:UTF8String:hola
1.3.6.1.4.1.55555.2 = DER:FF00FE
[san]
DNS.1 = www.ejemplo.test
DNS.2 = *.ejemplo.test
email.1 = usuario@ejemplo.test
URI.1 = https://ejemplo.test/ruta
IP.1 = 192.0.2.1
IP.2 = 2001:db8::1
RID.1 = 1.2.3.4
otherName.1 = 1.3.6.1.4.1.311.20.2.3;UTF8:upn@ejemplo.test
otherName.2 = 1.3.6.1.4.1.55555.3;UTF8:desconocido
dirName.1 = dir_sect
[dir_sect]
C = PE
O = "EMPRESA, S.A.C.; <x>"
OU = " espacio"
L = "a+b=c\\d\"e"
CN = Ñandú
[pol]
policyIdentifier = 1.3.6.1.4.1.55555.10
CPS.1 = https://ejemplo.test/cps
userNotice.1 = @notice
[notice]
explicitText = "Aviso sintetico"
organization = "Organizacion"
noticeNumbers = 1, 2
[tsa]
basicConstraints = CA:FALSE
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, timeStamping
[codesign]
keyUsage = critical, digitalSignature
extendedKeyUsage = codeSigning
[nsca]
nsCertType = sslCA, emailCA, objCA
[kuonly]
keyUsage = keyCertSign
[invalid]
keyUsage = DER:05:00
[saninvalid]
subjectAltName = DER:05:00
CNF

make_ext_identity() {
    local name="$1" section="$2" issuer="${3:-}"
    "$openssl_bin" genpkey -quiet -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out "$dir/ext-$name.key"
    if [[ "$section" == v1 ]]; then
        "$openssl_bin" req -new -x509 -x509v1 -key "$dir/ext-$name.key" -config "$dir/ext-profiles.cnf" \
            -set_serial 4662 -not_before 20240101000000Z -not_after 20340101000000Z -sha256 -out "$dir/ext-$name.crt"
    elif [[ -z "$issuer" ]]; then
        "$openssl_bin" req -new -x509 -key "$dir/ext-$name.key" -config "$dir/ext-profiles.cnf" -extensions "$section" \
            -set_serial 4660 -not_before 20240101000000Z -not_after 20340101000000Z -sha256 -out "$dir/ext-$name.crt"
    else
        "$openssl_bin" req -new -key "$dir/ext-$name.key" -config "$dir/ext-profiles.cnf" -out "$dir/ext-$name.csr"
        "$openssl_bin" x509 -req -in "$dir/ext-$name.csr" -CA "$dir/ext-$issuer.crt" -CAkey "$dir/ext-$issuer.key" \
            -extfile "$dir/ext-profiles.cnf" -extensions "$section" -set_serial 4661 \
            -not_before 20240101000000Z -not_after 20340101000000Z -sha256 -out "$dir/ext-$name.crt" 2>/dev/null
        rm -f "$dir/ext-$name.csr"
    fi
    "$openssl_bin" pkcs12 -export -inkey "$dir/ext-$name.key" -in "$dir/ext-$name.crt" \
        -passout "pass:clave-sintetica-de-prueba" -out "$dir/ext-$name.pfx"
}
for profile in ca tsa codesign nsca kuonly invalid saninvalid v1; do
    make_ext_identity "$profile" "$profile"
done
make_ext_identity server server ca
rm -f "$dir/ext-profiles.cnf"

pass="clave-sintetica-de-prueba"
printf '%s' "$pass" > "$dir/pfx.pass"
"$openssl_bin" pkcs12 -export -inkey "$dir/a.key" -in "$dir/a.crt" -passout "pass:$pass" -out "$dir/a.pfx"
"$openssl_bin" pkcs8 -topk8 -in "$dir/a.key" -v2 aes-256-cbc -passout "pass:$pass" -out "$dir/a.enc.pem"
"$openssl_bin" rand -hex 32 | tr -d '\n' > "$dir/hmac.key"
rm -f "$ext"
