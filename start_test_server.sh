#!/bin/bash

# Start OPC-UA Test Server (Python version)
# This script starts the Python-based OPC-UA test server directly

echo "🏃 Starting OPC-UA Test Server..."
echo "================================="

# Check if Python is available
if ! command -v python3 &> /dev/null; then
    echo "❌ Python3 is not installed"
    echo "   Please install Python 3.7+ to run the test server"
    exit 1
fi

# Create virtual environment if it doesn't exist
if [ ! -d "venv" ]; then
    echo "🔧 Creating Python virtual environment..."
    python3 -m venv venv
fi

# Activate virtual environment
echo "⚡ Activating virtual environment..."
source venv/bin/activate

# Install dependencies
echo "📦 Installing dependencies..."
pip install asyncua

# Check if test server script exists
if [ ! -f "scripts/test_server.py" ]; then
    echo "❌ Test server script not found at scripts/test_server.py"
    exit 1
fi

# Start the server
echo ""
echo "🚀 Starting OPC-UA Test Server..."
echo "   Endpoint: opc.tcp://localhost:4840/opcua/"
echo "   Press Ctrl+C to stop"
echo ""

python scripts/test_server.py 
