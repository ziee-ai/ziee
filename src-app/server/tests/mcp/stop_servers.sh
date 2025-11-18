#!/bin/bash
# Stop all MCP test servers

SSE_PID_FILE="/tmp/mcp_sse_server.pid"
HTTP_PID_FILE="/tmp/mcp_http_server.pid"

echo "Stopping MCP test servers..."

# Stop SSE server
if [ -f "$SSE_PID_FILE" ]; then
    PID=$(cat "$SSE_PID_FILE")
    if ps -p $PID > /dev/null 2>&1; then
        echo "Stopping SSE server (PID: $PID)..."
        kill $PID
        sleep 1
        # Force kill if still running
        if ps -p $PID > /dev/null 2>&1; then
            kill -9 $PID
        fi
        echo "SSE server stopped"
    fi
    rm -f "$SSE_PID_FILE"
else
    echo "SSE server not running"
fi

# Stop HTTP server
if [ -f "$HTTP_PID_FILE" ]; then
    PID=$(cat "$HTTP_PID_FILE")
    if ps -p $PID > /dev/null 2>&1; then
        echo "Stopping HTTP server (PID: $PID)..."
        kill $PID
        sleep 1
        # Force kill if still running
        if ps -p $PID > /dev/null 2>&1; then
            kill -9 $PID
        fi
        echo "HTTP server stopped"
    fi
    rm -f "$HTTP_PID_FILE"
else
    echo "HTTP server not running"
fi

# Clean up log files
rm -f /tmp/mcp_sse_server.log
rm -f /tmp/mcp_http_server.log

echo "All MCP test servers stopped"
