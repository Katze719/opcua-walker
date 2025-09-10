#!/bin/bash
# Generate test certificates for OPC-UA Walker
echo "Generating test certificates..."

# Generate private key
openssl genrsa -out client.key 2048

# Generate certificate signing request  
openssl req -new -key client.key -out client.csr -subj "/C=US/ST=Test/L=Test/O=TestOrg/CN=OpcUaClient"

# Generate self-signed certificate
openssl x509 -req -days 365 -in client.csr -signkey client.key -out client.crt

# Clean up CSR file
rm client.csr

echo "Generated files:"
echo "- client.key (private key)"
echo "- client.crt (certificate)"
echo ""
echo "Test certificate authentication with:"
echo "./target/release/opcua-walker -c client.crt -k client.key -v info"

