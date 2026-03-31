#!/usr/bin/env bash
# =============================================================================
# 1-infra.sh — Local infrastructure for integration testing
# =============================================================================
# This script sets up the local backend infrastructure:
#   1. Downloads DynamoDB Local if not already present
#   2. Starts DynamoDB Local (in-memory mode, no persistence)
#   3. Creates the backend DynamoDB table (matching templates/graphqlapi.yml)
#   4. Starts an X-Ray trace dumper (UDP:2000) to pretty-print traces
#
# Press Ctrl+C to cleanly shut down all background processes.
# =============================================================================

set -euo pipefail

# -- Configuration ------------------------------------------------------------

DYNAMODB_LOCAL_DIR="$HOME/.local/share/dynamodb_local"
DYNAMODB_LOCAL_JAR="$DYNAMODB_LOCAL_DIR/DynamoDBLocal.jar"
DYNAMODB_LOCAL_DOWNLOAD_URL="https://d1ni2b6xgvw0s0.cloudfront.net/v2.x/dynamodb_local_latest.zip"
DYNAMODB_LOCAL_ENDPOINT="http://localhost:8000"
TABLE_NAME="yak-mania-backend"
XRAY_PORT=2000

# PIDs of background processes, tracked for cleanup
DYNAMODB_PID=""
XRAY_PID=""

# -- Cleanup on exit (Ctrl+C or normal exit) ----------------------------------

cleanup() {
    echo ""
    echo "Shutting down..."
    # Kill X-Ray dumper if running
    if [[ -n "$XRAY_PID" ]] && kill -0 "$XRAY_PID" 2>/dev/null; then
        echo "  Stopping X-Ray dumper (PID $XRAY_PID)..."
        kill "$XRAY_PID" 2>/dev/null || true
        wait "$XRAY_PID" 2>/dev/null || true
    fi
    # Kill DynamoDB Local if running
    if [[ -n "$DYNAMODB_PID" ]] && kill -0 "$DYNAMODB_PID" 2>/dev/null; then
        echo "  Stopping DynamoDB Local (PID $DYNAMODB_PID)..."
        kill "$DYNAMODB_PID" 2>/dev/null || true
        wait "$DYNAMODB_PID" 2>/dev/null || true
    fi
    echo "Done."
    exit 0
}

trap cleanup SIGINT SIGTERM EXIT

# -- Step 1: Download DynamoDB Local if needed --------------------------------

if [[ ! -f "$DYNAMODB_LOCAL_JAR" ]]; then
    echo "DynamoDB Local not found at $DYNAMODB_LOCAL_JAR"
    echo "Downloading from $DYNAMODB_LOCAL_DOWNLOAD_URL ..."
    mkdir -p "$DYNAMODB_LOCAL_DIR"
    TMP_ZIP=$(mktemp /tmp/dynamodb_local_XXXXXX.zip)
    curl -fSL -o "$TMP_ZIP" "$DYNAMODB_LOCAL_DOWNLOAD_URL"
    unzip -o "$TMP_ZIP" -d "$DYNAMODB_LOCAL_DIR"
    rm -f "$TMP_ZIP"
    echo "DynamoDB Local installed to $DYNAMODB_LOCAL_DIR"
else
    echo "DynamoDB Local found at $DYNAMODB_LOCAL_JAR"
fi

# -- Step 2: Start DynamoDB Local (in-memory, shared database) ----------------

echo "Starting DynamoDB Local on port 8000 (in-memory mode)..."
java \
    -Djava.library.path="$DYNAMODB_LOCAL_DIR/DynamoDBLocal_lib" \
    -jar "$DYNAMODB_LOCAL_JAR" \
    -sharedDb -inMemory &
DYNAMODB_PID=$!
echo "DynamoDB Local started (PID $DYNAMODB_PID)"

# Wait for DynamoDB Local to become ready
echo -n "Waiting for DynamoDB Local to be ready"
for i in $(seq 1 30); do
    if aws dynamodb list-tables --endpoint-url "$DYNAMODB_LOCAL_ENDPOINT" --region eu-west-3 --no-cli-pager >/dev/null 2>&1; then
        echo " OK"
        break
    fi
    echo -n "."
    sleep 1
    if [[ $i -eq 30 ]]; then
        echo " FAILED (timed out after 30s)"
        exit 1
    fi
done

# -- Step 3: Create the DynamoDB table ---------------------------------------
# Matches the table defined in templates/graphqlapi.yml:
#   - Composite primary key: PK (Hash/String) + SK (Range/String)
#   - GSI "iType" on _TYPE (Hash/String), projecting ALL attributes
#   - On-demand billing (PAY_PER_REQUEST)

echo "Creating DynamoDB table '$TABLE_NAME'..."
aws dynamodb create-table \
    --endpoint-url "$DYNAMODB_LOCAL_ENDPOINT" \
    --region eu-west-3 \
    --no-cli-pager \
    --table-name "$TABLE_NAME" \
    --billing-mode PAY_PER_REQUEST \
    --attribute-definitions \
        AttributeName=PK,AttributeType=S \
        AttributeName=SK,AttributeType=S \
        AttributeName=_TYPE,AttributeType=S \
    --key-schema \
        AttributeName=PK,KeyType=HASH \
        AttributeName=SK,KeyType=RANGE \
    --global-secondary-indexes \
        'IndexName=iType,KeySchema=[{AttributeName=_TYPE,KeyType=HASH}],Projection={ProjectionType=ALL}' \
    > /dev/null

echo "Table '$TABLE_NAME' created successfully."

# -- Step 4: Start X-Ray trace dumper ----------------------------------------
# The Lambda (via awssdk-instrumentation) sends X-Ray segments as UDP datagrams
# to port 2000. Each datagram has a short header line followed by a JSON body.
# We use socat to receive datagrams, then strip the header and pretty-print
# the JSON payload with jq.

echo "Starting X-Ray trace dumper on UDP:$XRAY_PORT..."
echo "(X-Ray traces from the Lambda will be pretty-printed below)"
echo "========================================================================="

socat -u UDP-RECV:$XRAY_PORT STDOUT | while IFS= read -r line; do
    # The X-Ray daemon protocol sends a JSON header line like:
    #   {"format":"json","version":1}
    # followed by the actual trace segment JSON.
    # We attempt to pretty-print every line through jq; non-JSON lines pass through as-is.
    echo "$line" | jq '.' 2>/dev/null || echo "$line"
done &
XRAY_PID=$!
echo "X-Ray dumper started (PID $XRAY_PID)"

# -- Keep running until Ctrl+C -----------------------------------------------

echo ""
echo "Infrastructure is ready. Press Ctrl+C to stop."
echo "========================================================================="
wait
