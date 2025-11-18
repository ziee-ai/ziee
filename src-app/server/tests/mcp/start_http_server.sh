#!/bin/bash
# Start HTTP MCP test server (streamable-http weather server)

PID_FILE="/tmp/mcp_http_server.pid"
LOG_FILE="/tmp/mcp_http_server.log"
SERVER_SCRIPT="/tmp/mcp-streamable-http/python-example/server/weather.py"

# Check if server is already running
if [ -f "$PID_FILE" ]; then
    PID=$(cat "$PID_FILE")
    if ps -p $PID > /dev/null 2>&1; then
        echo "HTTP server already running (PID: $PID)"
        exit 0
    else
        rm -f "$PID_FILE"
    fi
fi

# Check if weather.py exists
if [ ! -f "$SERVER_SCRIPT" ]; then
    echo "ERROR: Weather server script not found at: $SERVER_SCRIPT"
    echo "Please ensure the streamable-http repo is cloned to /tmp/mcp-streamable-http"
    exit 1
fi

echo "Starting HTTP MCP server on http://localhost:8123..."

# Start server in background
nohup python3 "$SERVER_SCRIPT" --port=8123 > "$LOG_FILE" 2>&1 &
PID=$!
echo $PID > "$PID_FILE"

# Wait for server to be ready (check health)
MAX_WAIT=30
WAITED=0
while [ $WAITED -lt $MAX_WAIT ]; do
    if curl -s http://localhost:8123/ > /dev/null 2>&1; then
        echo "HTTP server ready (PID: $PID)"
        exit 0
    fi
    sleep 1
    WAITED=$((WAITED + 1))
done

# Server didn't start in time
echo "ERROR: HTTP server failed to start within ${MAX_WAIT} seconds"
echo "Check logs at: $LOG_FILE"
kill $PID 2>/dev/null
rm -f "$PID_FILE"
exit 1
