# OPC-UA Walker 🔍

A CLI tool written in Rust for exploring OPC-UA servers and their capabilities.

## Features

- 🔍 **Server Discovery**: Discover OPC-UA server capabilities and available services
- 📁 **Address Space Browser**: Browse the address space structure of OPC-UA servers
- 📖 **Variable Reader**: Read values from specific variable nodes
- 🔎 **Search by Name**: Find and read nodes by searching their display names
- ⚡ **Method Calling**: Call methods on OPC-UA servers with arguments
- ℹ️ **Server Info**: Display detailed server information and namespaces
- 🎨 **Colored Output**: User-friendly, colored console output
- 🔒 **Multiple Authentication Methods**: Support for Anonymous, Username/Password, and X.509 Certificate authentication

## Installation

### Prerequisites

- Rust 1.89 or higher
- Cargo

### Build from Source

```bash
git clone https://github.com/Katze719/opcua-walker
cd opcua-walker
cargo build --release
```

The executable will be located at `target/release/opcua-walker`.

## Quick Start

### 1. Start a Test Server

You have two options to start a test server:

#### Option A: Docker (if available)
```bash
# Start the OPC-UA test server
docker-compose up -d

# Verify it's running
docker-compose ps
```

#### Option B: Python (alternative)
```bash
# Start the Python-based test server directly
./start_test_server.sh

# This will:
# - Create a Python virtual environment
# - Install required dependencies
# - Start the OPC-UA server on localhost:4840
```

### 2. Test the CLI

```bash
# Display server information
./target/release/opcua-walker info

# Browse the server's address space
./target/release/opcua-walker browse

# Read a test variable
./target/release/opcua-walker read "ns=2;s=Counter"

# Run automated tests
./test_all.sh

# Stop the server when done (if using Docker)
docker-compose down
```

## Usage

### Basic Syntax

```bash
opcua-walker [OPTIONS] <COMMAND>
```

### Available Commands

- `discover`: Display server capabilities and available services
- `browse`: Browse address space and show all available nodes  
- `read <node-id>`: Read value of a specific variable
- `read --search <name>`: Find and read nodes by searching their display names
- `call <method-id> <object-id>`: Call a method on the server
- `info`: Display server information and namespaces

### Options

- `-e, --endpoint <URL>`: OPC-UA Server Endpoint URL (default: `opc.tcp://localhost:4840`)
- `-u, --username <USERNAME>`: Username for authentication
- `-p, --password <PASSWORD>`: Password for authentication  
- `-c, --cert <CERT_FILE>`: Client certificate file path for X.509 authentication
- `-k, --key <KEY_FILE>`: Client private key file path for X.509 authentication
- `-v, --verbose`: Enable detailed output
- `-h, --help`: Show help
- `-V, --version`: Show version

### Examples

#### Display Server Information
```bash
opcua-walker info
opcua-walker -e "opc.tcp://192.168.1.100:4840" info
```

#### Discover Server Capabilities
```bash
opcua-walker discover
opcua-walker -v discover  # With detailed output
```

#### Browse Address Space
```bash
opcua-walker browse
opcua-walker browse --node "ns=1;i=1001" --depth 5
```

#### Read Variable
```bash
opcua-walker read "ns=1;s=Temperature"
opcua-walker read "ns=0;i=2258"  # Server.ServerStatus.CurrentTime
```

#### Search and Read by Name
```bash
# Search for nodes containing "Temperature" in their name
opcua-walker read --search "Temperature"

# Search for multiple terms
opcua-walker read --search "Counter" "Pressure" "Status"

# Search with all attributes
opcua-walker read --search "Temperature" --all-attributes
```

#### Call Methods
```bash
# Call a method by name (auto-search for method and object)
opcua-walker call "Reboot"

# Call a method with exact node IDs
opcua-walker call "ns=2;s=ResetCounter" "ns=2;s=CounterObject"

# Call a method with arguments (auto-search)
opcua-walker call "AddNumbers" --args "5,10"

# Call with JSON arguments (exact IDs)
opcua-walker call "ns=2;s=ProcessData" "ns=2;s=DataObject" --args '[42, "test"]'

# Verbose output to see search details
opcua-walker call "Reboot" --verbose
```

#### Authentication Examples

##### Anonymous Connection (default)
```bash
opcua-walker info
```

##### Username/Password Authentication
```bash
opcua-walker -u admin -p password info
```

##### X.509 Certificate Authentication
```bash
opcua-walker -c client.crt -k client.key info
opcua-walker --cert /path/to/client.pem --key /path/to/private.key -v discover
```

## Testing

### Quick Testing 

Choose your preferred method:

#### Method 1: Docker (if available)
```bash
# Start test server
docker-compose up -d

# Run all tests
./test_all.sh

# Stop test server
docker-compose down
```

#### Method 2: Python Server
```bash
# Start test server (in one terminal)
./start_test_server.sh

# Run tests (in another terminal) 
./test_all.sh

# Stop server with Ctrl+C in first terminal
```

### Comprehensive Testing

See [TESTING.md](TESTING.md) for detailed testing instructions including:
- Docker-based test server setup
- Python-based test server setup
- Integration Objects free OPC-UA simulator
- Online demo servers
- Authentication testing scenarios
- Troubleshooting common issues
- Performance testing
- Manual test procedures

### Test Servers

| Server Type | Endpoint | Description |
|-------------|----------|-------------|
| Docker (Recommended) | `opc.tcp://localhost:4840/opcua/` | Python-based test server with custom variables |
| Integration Objects | `opc.tcp://localhost:48010` | Free 48-hour OPC-UA simulator |
| Online Demo | `opc.tcp://opcuaserver.com:48010` | Public demo server (availability varies) |

### Docker Test Server Variables

The included Docker test server provides these test variables:

| Variable | Node ID | Type | Description |
|----------|---------|------|-------------|
| Counter | `ns=2;s=Counter` | Int32 | Incrementing counter |
| Temperature | `ns=2;s=Temperature` | Float | Simulated temperature |
| Pressure | `ns=2;s=Pressure` | Float | Simulated pressure |
| Status | `ns=2;s=Status` | String | Rotating status messages |
| Timestamp | `ns=2;s=Timestamp` | String | Current timestamp |
| Boolean | `ns=2;s=Boolean` | Boolean | Alternating boolean |
| DynamicString | `ns=2;s=DynamicString` | String | Dynamic message |

## Supported OPC-UA Features

### Client Features
- ✅ Session Management
- ✅ Security Policy: None
- ✅ Anonymous Authentication
- ✅ Username/Password Authentication
- ✅ X.509 Certificate Authentication
- ✅ Read Service
- ✅ Browse Service
- ✅ Method Call Service
- ✅ Node Search by Name
- ⏳ Write Service (planned)
- ⏳ Subscription Service (planned)

### Security
- ✅ No Security (SecurityPolicy: None)
- ⏳ Basic128Rsa15 (planned)
- ⏳ Basic256Sha256 (planned)

### Authentication Methods
- ✅ **Anonymous**: No authentication required
- ✅ **Username/Password**: Basic credential-based authentication
- ✅ **X.509 Certificate**: Certificate-based authentication using client certificates

## Development

### Code Structure

```
src/
├── main.rs           # CLI interface and main logic

docs/
├── README.md         # This file
├── TESTING.md        # Comprehensive testing guide
└── generate_test_certs.sh # Certificate generation script

testing/
├── docker-compose.yml # Test server setup
├── scripts/test_server.py # Python OPC-UA test server
└── test_all.sh       # Automated testing script
```

### Dependencies

- `opcua`: OPC-UA client implementation for Rust
- `clap`: Command-line interface framework
- `tokio`: Asynchronous runtime
- `anyhow`: Enhanced error handling
- `colored`: Colored console output
- `tabled`: Table formatting

### Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/new-feature`)
3. Commit your changes (`git commit -am 'Add new feature'`)
4. Push to the branch (`git push origin feature/new-feature`)
5. Create a Pull Request

## Certificate Authentication

### Supported Certificate Formats

- **PEM format**: `.pem`, `.crt`, `.cer` files
- **DER format**: `.der`, `.crt` files

### Certificate Requirements

- The certificate must be valid and not expired
- The private key must match the certificate
- For production use, certificates should be signed by a trusted CA
- Self-signed certificates are supported for testing

### Generating Test Certificates

You can generate self-signed certificates for testing:

```bash
# Use the provided script
./generate_test_certs.sh

# Or manually:
openssl genrsa -out client.key 2048
openssl req -new -key client.key -out client.csr
openssl x509 -req -days 365 -in client.csr -signkey client.key -out client.crt
```

## Roadmap

### Version 0.2.0
- [ ] Browse Service with recursive navigation
- [ ] Write Service implementation
- [ ] Enhanced namespace array display
- [ ] JSON/CSV export functions

### Version 0.3.0  
- [ ] Subscription and monitoring
- [ ] Advanced security policies
- [ ] Certificate validation options
- [ ] Discovery service

### Version 0.4.0
- [ ] Interactive mode
- [ ] Configuration files
- [ ] Batch operations
- [ ] Performance optimizations

## Known Limitations

- Currently only SecurityPolicy "None" is supported
- Write Service not yet implemented
- Subscriptions not yet available
- Namespace array is displayed as raw debug output

## Compatibility

Tested with:
- Standard OPC-UA server implementations
- Integration Objects OPC-UA Server Simulator
- Python-based OPC-UA servers (asyncua)
- open62541-based servers
- Eclipse Milo demo servers

## Troubleshooting

### Common Issues

1. **Connection refused**: Ensure the OPC-UA server is running and accessible
   ```bash
   # For Docker test server:
   docker-compose ps
   docker-compose logs opcua-test-server
   ```

2. **Certificate errors**: Verify certificate and key files exist and are readable
3. **Authentication failed**: Check username/password or certificate validity
4. **Permission denied**: Ensure proper file permissions for certificate files

### Method Calling Issues

**Error: BadTooManyOperations**
- The server is overwhelmed by browse operations during method search
- **Solution**: Use exact node IDs instead of method names:
  ```bash
  opcua-walker call "ns=2;s=RebootMethod" "ns=2;s=ServerObject"
  ```

**Error: BadLicenseNotAvailable**
- The server requires a license for method execution
- **Solution**: Contact server administrator or check licensing requirements

**Error: BadUnexpectedError**
- The method may require specific arguments or have execution restrictions
- **Solution**: Try with explicit arguments or check method signature:
  ```bash
  opcua-walker call "MethodName" --args '[arg1, arg2]'
  ```

**Method not found**
- Use `browse` to explore available methods:
  ```bash
  opcua-walker browse --depth 3
  ```
- Try with `--verbose` for detailed search information:
  ```bash
  opcua-walker call "MethodName" --verbose
  ```

### Debug Mode

Use the `--verbose` flag for detailed debugging information:

```bash
opcua-walker -v info
```

### Testing Connectivity

```bash
# Test if the server is responding
telnet localhost 4840

# Run the comprehensive test suite
./test_all.sh

# Test with online demo server
opcua-walker -e "opc.tcp://opcuaserver.com:48010" info
```

### Docker Troubleshooting

```bash
# Check if Docker is running
docker version

# Start test server
docker-compose up -d

# Check server logs
docker-compose logs opcua-test-server

# Stop and restart server
docker-compose down && docker-compose up -d

# Check server status
docker-compose ps
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Support

For questions or issues:
1. Check the [Issues](../issues) on GitHub
2. Create a new issue with detailed description
3. For security-related problems, contact maintainers directly

---

**Note**: OPC-UA Walker is under active development. Features and API may change in future versions.
