#!/bin/bash
# Start SSE MCP test server (mcp-weather-server)

PID_FILE="/tmp/mcp_sse_server.pid"
LOG_FILE="/tmp/mcp_sse_server.log"

# Check if server is already running
if [ -f "$PID_FILE" ]; then
    PID=$(cat "$PID_FILE")
    if ps -p $PID > /dev/null 2>&1; then
        echo "SSE server already running (PID: $PID)"
        exit 0
    else
        rm -f "$PID_FILE"
    fi
fi

echo "Starting MCP weather server in streamable-http mode on http://0.0.0.0:8080..."

# Start server in background (streamable-http mode instead of sse)
nohup python3 -m mcp_weather_server.server --mode streamable-http --port 8080 > "$LOG_FILE" 2>&1 &
PID=$!
echo $PID > "$PID_FILE"

# Wait for server to be ready (check health)
MAX_WAIT=30
WAITED=0
while [ $WAITED -lt $MAX_WAIT ]; do
    if curl -s http://localhost:8080/ > /dev/null 2>&1; then
        echo "SSE server ready (PID: $PID)"
        exit 0
    fi
    sleep 1
    WAITED=$((WAITED + 1))
done

# Server didn't start in time
echo "ERROR: SSE server failed to start within ${MAX_WAIT} seconds"
echo "Check logs at: $LOG_FILE"
kill $PID 2>/dev/null
rm -f "$PID_FILE"
exit 1
