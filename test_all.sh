#!/bin/bash

# OPC-UA Walker Testing Script
# This script tests all major CLI functions against an OPC-UA server

ENDPOINT="${1:-opc.tcp://localhost:4840/opcua/}"
WALKER="./target/release/opcua-walker"

echo "🔍 Testing OPC-UA Walker with endpoint: $ENDPOINT"
echo "================================================="

# Check if the CLI binary exists
if [ ! -f "$WALKER" ]; then
    echo "❌ Error: OPC-UA Walker binary not found at $WALKER"
    echo "   Please build the project first: cargo build --release"
    exit 1
fi

# Check if Docker server is running for default endpoint
if [[ "$ENDPOINT" == *"localhost:4840"* ]]; then
    echo "🐳 Checking if test server is available..."
    
    # Check for Docker Compose
    if command -v docker-compose &> /dev/null; then
        if ! docker-compose ps | grep -q "opcua-test-server.*Up"; then
            echo "⚠️  Docker test server not running. Starting it..."
            docker-compose up -d
            sleep 5
        else
            echo "✅ Docker test server is running"
        fi
    # Check for native Python server process
    elif pgrep -f "python.*test_server.py" > /dev/null; then
        echo "✅ Python test server is running"
    else
        echo "⚠️  No test server found running."
        echo "   Start test server with:"
        if command -v docker-compose &> /dev/null; then
            echo "   • Docker: docker-compose up -d"
        fi
        echo "   • Python: ./start_test_server.sh (in another terminal)"
        echo ""
        echo "   Continuing with tests - some may fail if server is unavailable..."
    fi
    echo ""
fi

echo ""
echo "1️⃣  Testing INFO command..."
echo "----------------------------"
if $WALKER -e "$ENDPOINT" info; then
    echo "✅ Info command successful"
else
    echo "❌ Info command failed"
    echo "   Server might not be available at $ENDPOINT"
    if [[ "$ENDPOINT" == *"localhost:4840"* ]]; then
        echo "   Try starting a test server:"
        if command -v docker-compose &> /dev/null; then
            echo "   • Docker: docker-compose up -d"
        fi
        echo "   • Python: ./start_test_server.sh"
    fi
fi

echo ""
echo "2️⃣  Testing DISCOVER command..."
echo "--------------------------------"
if $WALKER -e "$ENDPOINT" discover; then
    echo "✅ Discover command successful"  
else
    echo "❌ Discover command failed"
fi

echo ""
echo "3️⃣  Testing BROWSE command..."
echo "------------------------------"
if $WALKER -e "$ENDPOINT" browse --depth 2; then
    echo "✅ Browse command successful"
else
    echo "❌ Browse command failed"
fi

echo ""
echo "4️⃣  Testing READ command (Server CurrentTime)..."
echo "--------------------------------------------------"
if $WALKER -e "$ENDPOINT" read "ns=0;i=2258"; then
    echo "✅ Standard read command successful"
else
    echo "❌ Standard read command failed"
fi

# Test custom variables if using our test server
if [[ "$ENDPOINT" == *"localhost:4840"* ]]; then
    echo ""
    echo "5️⃣  Testing custom variables (test server)..."
    echo "------------------------------------------------"
    
    # Test counter variable
    if $WALKER -e "$ENDPOINT" read "ns=2;s=Counter"; then
        echo "✅ Counter variable read successful"
    else
        echo "❌ Counter variable read failed"
    fi
    
    # Test temperature variable
    if $WALKER -e "$ENDPOINT" read "ns=2;s=Temperature"; then
        echo "✅ Temperature variable read successful"
    else
        echo "❌ Temperature variable read failed"  
    fi
fi

echo ""
echo "6️⃣  Testing VERBOSE mode..."
echo "----------------------------"
if $WALKER -e "$ENDPOINT" -v info >/dev/null 2>&1; then
    echo "✅ Verbose mode works"
else
    echo "❌ Verbose mode failed"
fi

echo ""
echo "7️⃣  Testing authentication parameter validation..."
echo "---------------------------------------------------"

# Test invalid auth combinations
if $WALKER -u admin info 2>&1 | grep -q "Username provided but password is missing"; then
    echo "✅ Username/password validation works"
else
    echo "❌ Username/password validation failed"
fi

if $WALKER -c cert.crt info 2>&1 | grep -q "Certificate provided but private key is missing"; then
    echo "✅ Certificate validation works"
else
    echo "❌ Certificate validation failed"
fi

echo ""
echo "🎉 All tests completed!"
echo "========================"

if [[ "$ENDPOINT" == *"localhost:4840"* ]]; then
    echo ""
    echo "🏃 Test server status:"
    
    # Check Docker status
    if command -v docker-compose &> /dev/null; then
        SERVER_STATUS=$(docker-compose ps --services --filter status=running | grep opcua-test-server || echo '')
        if [ -n "$SERVER_STATUS" ]; then
            echo "   Docker server: Running"
            echo "   Stop with: docker-compose down"
        fi
    fi
    
    # Check Python process status  
    if pgrep -f "python.*test_server.py" > /dev/null; then
        echo "   Python server: Running (PID: $(pgrep -f 'python.*test_server.py'))"
        echo "   Stop with: Ctrl+C in server terminal"
    fi
    
    if ! command -v docker-compose &> /dev/null && ! pgrep -f "python.*test_server.py" > /dev/null; then
        echo "   No test server detected running"
    fi
fi

echo ""
echo "💡 Tips:"
echo "   • Use -v flag for detailed output: $WALKER -v info"
echo "   • Try different endpoints with -e flag"
echo "   • Check TESTING.md for more testing scenarios"
if [[ "$ENDPOINT" == *"localhost:4840"* ]]; then
    echo "   • Test server provides these variables:"
    echo "     - ns=2;s=Counter, ns=2;s=Temperature, ns=2;s=Status"
    echo "   • Start test server:"
    if command -v docker-compose &> /dev/null; then
        echo "     - Docker: docker-compose up -d"
    fi
    echo "     - Python: ./start_test_server.sh"
else
    echo "   • Start local test server: ./start_test_server.sh"
fi 
