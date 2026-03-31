#!/usr/bin/env bash
# =============================================================================
# 3-scenarios.sh — Integration test scenario for yak-mania
# =============================================================================
# This script exercises the full game lifecycle by invoking the Lambda through
# cargo lambda invoke. It runs 29 steps covering player registration, game
# state transitions, yak buy/sell operations, and error cases.
#
# Prerequisites:
#   - 1-infra.sh running in Terminal 1 (DynamoDB Local + X-Ray dumper)
#   - 2-lambda.sh running in Terminal 2 (cargo lambda watch)
#
# For each step the script:
#   1. Announces what it will do and the expected result
#   2. Invokes the Lambda with the appropriate event payload
#   3. Pretty-prints the response with jq
#   4. Verifies the response matches expectations (success/error)
#   5. Displays a colored OK/NOK result
#
# Steps are tightly coupled: if any step fails, the script stops immediately
# and prints the summary report.
# =============================================================================

set -euo pipefail

# -- Configuration ------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
EVENTS_DIR="$SCRIPT_DIR/appsync_events"

DYNAMODB_LOCAL_ENDPOINT="http://localhost:8000"
TABLE_NAME="yak-mania-backend"

# Generate secrets for all player operations in this test run
TEMP_PLAYER_SECRET="$(uuidgen)"
PLAYER1_SECRET="$(uuidgen)"
PLAYER2_SECRET="$(uuidgen)"

# Tracked IDs — populated as the scenario progresses
TEMP_PLAYER_ID=""
P1_ID=""
P2_ID=""
YAK_ID=""

# Counters
PASS=0
FAIL=0

# -- Colors -------------------------------------------------------------------

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# -- Report & exit function ---------------------------------------------------
# Called after each step to print the summary and exit on failure.
# Since steps are tightly coupled, there is no point continuing after a failure.

report_and_exit() {
    echo ""
    echo -e "${BOLD}${CYAN}==========================================================================${NC}"
    if [[ $FAIL -gt 0 ]]; then
        echo -e "${BOLD}${CYAN}  SCENARIO ABORTED${NC}"
    else
        echo -e "${BOLD}${CYAN}  SCENARIO COMPLETE${NC}"
    fi
    echo -e "${BOLD}${CYAN}==========================================================================${NC}"
    echo -e "  ${GREEN}Passed: $PASS / 29${NC}"
    echo ""

    if [[ $FAIL -gt 0 ]]; then
        echo -e "  ${RED}${BOLD}STEP FAILED — aborting${NC}"
        exit 1
    else
        echo -e "  ${GREEN}${BOLD}ALL STEPS PASSED${NC}"
        exit 0
    fi
}

# -- Helper functions ---------------------------------------------------------

# invoke_lambda <event_template> [sed_replacements...]
# Applies sed replacements to the template, writes to a temp file, invokes the
# Lambda, and stores the raw response in $RESPONSE. Cleans up the temp file.
invoke_lambda() {
    local template="$1"
    shift
    local tmp_file
    tmp_file=$(mktemp /tmp/yak-mania-test-XXXXXX.json)

    # Start from the template and apply all sed replacements
    cp "$template" "$tmp_file"
    for replacement in "$@"; do
        sed -i "$replacement" "$tmp_file"
    done

    # Invoke the Lambda and capture the raw response
    # cargo lambda invoke returns the Lambda output on stdout
    RESPONSE=$(cargo lambda invoke appsync-source --data-file "$tmp_file" 2>/dev/null) || true

    # Clean up temp file
    rm -f "$tmp_file"
}

# print_step <step_number> <description> <expected>
# Prints a visually distinct step header.
print_step() {
    local num="$1"
    local desc="$2"
    local expected="$3"
    echo ""
    echo -e "${BOLD}${CYAN}==========================================================================${NC}"
    echo -e "${BOLD}${CYAN}  STEP $num: $desc${NC}"
    echo -e "${YELLOW}  Expected: $expected${NC}"
    echo -e "${BOLD}${CYAN}==========================================================================${NC}"
}

# print_response
# Pretty-prints the current $RESPONSE through jq.
print_response() {
    echo "$RESPONSE" | jq '.' 2>/dev/null || echo "$RESPONSE"
}

# check_success <step_number>
# Verifies the response is a successful result (no errorType in first element).
# Aborts the scenario immediately on failure.
check_success() {
    local num="$1"
    local has_error
    has_error=$(echo "$RESPONSE" | jq -r '.[0].errorType // empty' 2>/dev/null)
    if [[ -z "$has_error" ]]; then
        echo -e "  ${GREEN}${BOLD}[STEP $num] OK${NC}"
        PASS=$((PASS + 1))
    else
        echo -e "  ${RED}${BOLD}[STEP $num] NOK — unexpected error: $has_error${NC}"
        FAIL=$((FAIL + 1))
        report_and_exit
    fi
}

# check_error <step_number> [expected_error_type]
# Verifies the response contains an error. Optionally checks the error type.
# Aborts the scenario immediately on failure.
check_error() {
    local num="$1"
    local expected_type="${2:-}"
    local actual_type
    actual_type=$(echo "$RESPONSE" | jq -r '.[0].errorType // empty' 2>/dev/null)
    if [[ -n "$actual_type" ]]; then
        if [[ -n "$expected_type" && "$actual_type" != "$expected_type" ]]; then
            echo -e "  ${RED}${BOLD}[STEP $num] NOK — expected error '$expected_type' but got '$actual_type'${NC}"
            FAIL=$((FAIL + 1))
            report_and_exit
        else
            echo -e "  ${GREEN}${BOLD}[STEP $num] OK (error: $actual_type)${NC}"
            PASS=$((PASS + 1))
        fi
    else
        echo -e "  ${RED}${BOLD}[STEP $num] NOK — expected an error but got a success response${NC}"
        FAIL=$((FAIL + 1))
        report_and_exit
    fi
}

# extract_field <jq_path>
# Extracts a value from the response using a jq path expression prefixed by ".[0].data.".
extract_field() {
    echo "$RESPONSE" | jq -r ".[0].data.$1" 2>/dev/null
}

# -- Reset DynamoDB table -----------------------------------------------------
# Drops and recreates the table so every run starts with a clean slate.
# This avoids having to restart 1-infra.sh between runs.

reset_dynamodb_table() {
    echo -n "Resetting DynamoDB table '$TABLE_NAME'..."

    # Delete the table (ignore errors if it doesn't exist yet)
    aws dynamodb delete-table \
        --endpoint-url "$DYNAMODB_LOCAL_ENDPOINT" \
        --region eu-west-3 \
        --no-cli-pager \
        --table-name "$TABLE_NAME" \
        > /dev/null 2>&1 || true

    # Recreate with the same schema as 1-infra.sh / templates/graphqlapi.yml
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

    # Wait until the table (and its GSI) is fully ACTIVE before proceeding
    aws dynamodb wait table-exists \
        --endpoint-url "$DYNAMODB_LOCAL_ENDPOINT" \
        --region eu-west-3 \
        --table-name "$TABLE_NAME"

    echo " OK"
}

# =============================================================================
# SCENARIO START
# =============================================================================

echo -e "${BOLD}Yak Mania — Integration Test Scenario${NC}"
echo ""

reset_dynamodb_table

# -- Step 1: Register a temporary player -------------------------------------

print_step 1 "Register temp player 'TempPlayer'" "OK — player created"
invoke_lambda "$EVENTS_DIR/mutation.registerNewPlayer.json" \
    "s/#PLAYER_NAME#/TempPlayer/" \
    "s/#PLAYER_SECRET#/$TEMP_PLAYER_SECRET/"
print_response
TEMP_PLAYER_ID=$(extract_field 'player.id')
echo -e "  Extracted player_id: ${CYAN}$TEMP_PLAYER_ID${NC}"
check_success 1

# -- Step 2: Update player name with wrong secret ----------------------------

print_step 2 "Update TempPlayer's name using wrong secret" "ERROR — wrong secret"
invoke_lambda "$EVENTS_DIR/mutation.updatePlayerName.json" \
    "s/#PLAYER_ID#/$TEMP_PLAYER_ID/" \
    "s/#PLAYER_SECRET#/$(uuidgen)/" \
    "s/#NEW_NAME#/OffensiveName69/"
print_response
check_error 2

# -- Step 3: Update player name to something mildly offensive ----------------

print_step 3 "Update TempPlayer's name to 'OffensiveName69'" "OK — name updated"
invoke_lambda "$EVENTS_DIR/mutation.updatePlayerName.json" \
    "s/#PLAYER_ID#/$TEMP_PLAYER_ID/" \
    "s/#PLAYER_SECRET#/$TEMP_PLAYER_SECRET/" \
    "s/#NEW_NAME#/OffensiveName69/"
print_response
check_success 3

# -- Step 4: Admin removes the temp player -----------------------------------

print_step 4 "Admin removes TempPlayer" "OK — player removed"
invoke_lambda "$EVENTS_DIR/admin.mutation.removePlayer.json" \
    "s/#PLAYER_ID#/$TEMP_PLAYER_ID/"
print_response
check_success 4

# -- Step 5: Register Player1 ------------------------------------------------

print_step 5 "Register 'Player1'" "OK — player created"
invoke_lambda "$EVENTS_DIR/mutation.registerNewPlayer.json" \
    "s/#PLAYER_NAME#/Player1/" \
    "s/#PLAYER_SECRET#/$PLAYER1_SECRET/"
print_response
P1_ID=$(extract_field 'player.id')
echo -e "  Extracted Player1 ID: ${CYAN}$P1_ID${NC}"
check_success 5

# -- Step 6: Register Player2 ------------------------------------------------

print_step 6 "Register 'Player2'" "OK — player created"
invoke_lambda "$EVENTS_DIR/mutation.registerNewPlayer.json" \
    "s/#PLAYER_NAME#/Player2/" \
    "s/#PLAYER_SECRET#/$PLAYER2_SECRET/"
print_response
P2_ID=$(extract_field 'player.id')
echo -e "  Extracted Player2 ID: ${CYAN}$P2_ID${NC}"
check_success 6

# -- Step 7: Query game state ------------------------------------------------

print_step 7 "Query game state" "OK — 2 players, 0 yaks, status RESET"
invoke_lambda "$EVENTS_DIR/query.gameState.json"
print_response
# Verify key fields
GAME_STATUS=$(extract_field 'game_status')
PLAYER_COUNT=$(extract_field 'players | length')
YAK_TOTAL=$(extract_field 'yak_counts | del(.in_nursery) | del(.total_sheared) | to_entries | map(.value) | add')
if [[ "$GAME_STATUS" == "RESET" && "$PLAYER_COUNT" == "2" && "$YAK_TOTAL" == "0" ]]; then
    echo -e "  ${GREEN}${BOLD}[STEP 7] OK${NC} (status=$GAME_STATUS, players=$PLAYER_COUNT, total_yaks=$YAK_TOTAL)"
    PASS=$((PASS + 1))
else
    echo -e "  ${RED}${BOLD}[STEP 7] NOK${NC} (status=$GAME_STATUS, players=$PLAYER_COUNT, total_yaks=$YAK_TOTAL)"
    FAIL=$((FAIL + 1))
    report_and_exit
fi

# -- Step 8: Player1 tries to buy a baby yak (game not started) --------------

print_step 8 "Player1 tries to buy a baby yak" "ERROR — game not started"
invoke_lambda "$EVENTS_DIR/mutation.buyBabyYak.json" \
    "s/#PLAYER_ID#/$P1_ID/" \
    "s/#PLAYER_SECRET#/$PLAYER1_SECRET/"
print_response
check_error 8 "InvalidGameStatus"

# -- Step 9: Admin tries to reset (game is not stopped) ----------------------

print_step 9 "Admin tries to reset the game" "ERROR — game not stopped (it's RESET)"
invoke_lambda "$EVENTS_DIR/admin.mutation.resetGame.json"
print_response
check_error 9

# -- Step 10: Admin tries to stop (game is not started) ---------------------

print_step 10 "Admin tries to stop the game" "ERROR — game not started"
invoke_lambda "$EVENTS_DIR/admin.mutation.stopGame.json"
print_response
check_error 10

# -- Step 11: Admin starts the game -----------------------------------------

print_step 11 "Admin starts the game" "OK — game status STARTED"
invoke_lambda "$EVENTS_DIR/admin.mutation.startGame.json"
print_response
check_success 11

# -- Step 12: Player1 tries to buy a grown yak (none in warehouse) ----------

print_step 12 "Player1 tries to buy a grown yak" "ERROR — none available in warehouse"
invoke_lambda "$EVENTS_DIR/mutation.buyGrownYak.json" \
    "s/#PLAYER_ID#/$P1_ID/" \
    "s/#PLAYER_SECRET#/$PLAYER1_SECRET/"
print_response
check_error 12

# -- Step 13: Player1 tries to buy an unsheared yak (none in shearing shed) -

print_step 13 "Player1 tries to buy an unsheared yak" "ERROR — none available in shearing shed"
invoke_lambda "$EVENTS_DIR/mutation.buyUnshearedYak.json" \
    "s/#PLAYER_ID#/$P1_ID/" \
    "s/#PLAYER_SECRET#/$PLAYER1_SECRET/"
print_response
check_error 13

# -- Step 14: Player1 buys a baby yak (becomes BREEDER) ---------------------

print_step 14 "Player1 buys a baby yak" "OK — yak created, Player1 is BREEDER"
invoke_lambda "$EVENTS_DIR/mutation.buyBabyYak.json" \
    "s/#PLAYER_ID#/$P1_ID/" \
    "s/#PLAYER_SECRET#/$PLAYER1_SECRET/"
print_response
# This is THE yak that will be tracked through the entire lifecycle
YAK_ID=$(extract_field 'player.assignment.yak.id')
echo -e "  Extracted YAK_ID: ${CYAN}$YAK_ID${NC}"
check_success 14

# -- Step 15: Player1 tries to buy another baby yak (already has a job) ------

print_step 15 "Player1 tries to buy another baby yak" "ERROR — already has a job"
invoke_lambda "$EVENTS_DIR/mutation.buyBabyYak.json" \
    "s/#PLAYER_ID#/$P1_ID/" \
    "s/#PLAYER_SECRET#/$PLAYER1_SECRET/"
print_response
check_error 15 "InvalidPlayerStatus"

# -- Step 16: Player1 tries to sell an unsheared yak (wrong job) -------------

print_step 16 "Player1 tries to sell an unsheared yak" "ERROR — has another job (BREEDER, not DRIVER)"
invoke_lambda "$EVENTS_DIR/mutation.sellUnshearedYak.json" \
    "s/#PLAYER_ID#/$P1_ID/" \
    "s/#PLAYER_SECRET#/$PLAYER1_SECRET/" \
    "s/#YAK_ID#/$YAK_ID/"
print_response
check_error 16

# -- Step 17: Player1 tries to sell a sheared yak (wrong job) ----------------

print_step 17 "Player1 tries to sell a sheared yak" "ERROR — has another job (BREEDER, not SHEARER)"
invoke_lambda "$EVENTS_DIR/mutation.sellShearedYak.json" \
    "s/#PLAYER_ID#/$P1_ID/" \
    "s/#PLAYER_SECRET#/$PLAYER1_SECRET/" \
    "s/#YAK_ID#/$YAK_ID/"
print_response
check_error 17

# -- Step 18: Player1 tries to sell a grown yak with wrong secret ------------

print_step 18 "Player1 tries to sell a grown yak with wrong secret" "ERROR — wrong secret"
invoke_lambda "$EVENTS_DIR/mutation.sellGrownYak.json" \
    "s/#PLAYER_ID#/$P1_ID/" \
    "s/#PLAYER_SECRET#/$(uuidgen)/" \
    "s/#YAK_ID#/$YAK_ID/"
print_response
check_error 18

# -- Step 19: Player1 sells a grown yak (yak goes to WAREHOUSE) -------------

print_step 19 "Player1 sells a grown yak" "OK — yak moves to warehouse, Player1 job done"
invoke_lambda "$EVENTS_DIR/mutation.sellGrownYak.json" \
    "s/#PLAYER_ID#/$P1_ID/" \
    "s/#PLAYER_SECRET#/$PLAYER1_SECRET/" \
    "s/#YAK_ID#/$YAK_ID/"
print_response
check_success 19

# -- Step 20: Player2 buys a grown yak from warehouse (becomes DRIVER) ------

print_step 20 "Player2 buys a grown yak" "OK — Player2 is DRIVER, yak.id should be $YAK_ID"
invoke_lambda "$EVENTS_DIR/mutation.buyGrownYak.json" \
    "s/#PLAYER_ID#/$P2_ID/" \
    "s/#PLAYER_SECRET#/$PLAYER2_SECRET/"
print_response
# Verify the returned yak is the expected one
RETURNED_YAK=$(extract_field 'player.assignment.yak.id')
if [[ "$RETURNED_YAK" == "$YAK_ID" ]]; then
    echo -e "  Yak ID matches: ${GREEN}$RETURNED_YAK${NC}"
    echo -e "  ${GREEN}${BOLD}[STEP 20] OK${NC}"
    PASS=$((PASS + 1))
else
    echo -e "  Yak ID mismatch: expected ${CYAN}$YAK_ID${NC}, got ${RED}$RETURNED_YAK${NC}"
    echo -e "  ${RED}${BOLD}[STEP 20] NOK${NC}"
    FAIL=$((FAIL + 1))
    report_and_exit
fi

# -- Step 21: Player2 sells an unsheared yak (yak goes to SHEARING SHED) ----

print_step 21 "Player2 sells an unsheared yak" "OK — yak moves to shearing shed, Player2 job done"
invoke_lambda "$EVENTS_DIR/mutation.sellUnshearedYak.json" \
    "s/#PLAYER_ID#/$P2_ID/" \
    "s/#PLAYER_SECRET#/$PLAYER2_SECRET/" \
    "s/#YAK_ID#/$YAK_ID/"
print_response
check_success 21

# -- Step 22: Player2 buys a baby yak (becomes BREEDER) ---------------------

print_step 22 "Player2 buys a baby yak" "OK — new yak created, Player2 is BREEDER"
invoke_lambda "$EVENTS_DIR/mutation.buyBabyYak.json" \
    "s/#PLAYER_ID#/$P2_ID/" \
    "s/#PLAYER_SECRET#/$PLAYER2_SECRET/"
print_response
check_success 22

# -- Step 23: Admin stops the game ------------------------------------------

print_step 23 "Admin stops the game" "OK — game status STOPPED"
invoke_lambda "$EVENTS_DIR/admin.mutation.stopGame.json"
print_response
check_success 23

# -- Step 24: Player1 tries to buy a baby yak (game stopped) ----------------

print_step 24 "Player1 tries to buy a baby yak" "ERROR — game not started"
invoke_lambda "$EVENTS_DIR/mutation.buyBabyYak.json" \
    "s/#PLAYER_ID#/$P1_ID/" \
    "s/#PLAYER_SECRET#/$PLAYER1_SECRET/"
print_response
check_error 24 "InvalidGameStatus"

# -- Step 25: Player1 buys an unsheared yak from shearing shed (SHEARER) ----
# Note: buying from a waiting place works even when the game is stopped.
# The tracked yak (YAK_ID) was placed in the shearing shed at step 21.

print_step 25 "Player1 buys an unsheared yak (game stopped, but OK)" "OK — Player1 is SHEARER, yak.id should be $YAK_ID"
invoke_lambda "$EVENTS_DIR/mutation.buyUnshearedYak.json" \
    "s/#PLAYER_ID#/$P1_ID/" \
    "s/#PLAYER_SECRET#/$PLAYER1_SECRET/"
print_response
# Verify the returned yak is the expected one
RETURNED_YAK=$(extract_field 'player.assignment.yak.id')
if [[ "$RETURNED_YAK" == "$YAK_ID" ]]; then
    echo -e "  Yak ID matches: ${GREEN}$RETURNED_YAK${NC}"
    echo -e "  ${GREEN}${BOLD}[STEP 25] OK${NC}"
    PASS=$((PASS + 1))
else
    echo -e "  Yak ID mismatch: expected ${CYAN}$YAK_ID${NC}, got ${RED}$RETURNED_YAK${NC}"
    echo -e "  ${RED}${BOLD}[STEP 25] NOK${NC}"
    FAIL=$((FAIL + 1))
    report_and_exit
fi

# -- Step 26: Player1 sells a sheared yak (yak is deleted) ------------------

print_step 26 "Player1 sells a sheared yak" "OK — yak sheared (deleted), Player1 job done"
invoke_lambda "$EVENTS_DIR/mutation.sellShearedYak.json" \
    "s/#PLAYER_ID#/$P1_ID/" \
    "s/#PLAYER_SECRET#/$PLAYER1_SECRET/" \
    "s/#YAK_ID#/$YAK_ID/"
print_response
check_success 26

# -- Step 27: Query game state (after stop) ----------------------------------
# Expected: 2 players, 1 yak (P2's baby yak from step 22), status STOPPED

print_step 27 "Query game state" "OK — 2 players, 1 yak total, status STOPPED"
invoke_lambda "$EVENTS_DIR/query.gameState.json"
print_response
GAME_STATUS=$(extract_field 'game_status')
PLAYER_COUNT=$(extract_field 'players | length')
YAK_TOTAL=$(extract_field 'yak_counts | del(.in_nursery) | del(.total_sheared) | to_entries | map(.value) | add')
if [[ "$GAME_STATUS" == "STOPPED" && "$PLAYER_COUNT" == "2" && "$YAK_TOTAL" == "1" ]]; then
    echo -e "  ${GREEN}${BOLD}[STEP 27] OK${NC} (status=$GAME_STATUS, players=$PLAYER_COUNT, total_yaks=$YAK_TOTAL)"
    PASS=$((PASS + 1))
else
    echo -e "  ${RED}${BOLD}[STEP 27] NOK${NC} (status=$GAME_STATUS, players=$PLAYER_COUNT, total_yaks=$YAK_TOTAL)"
    FAIL=$((FAIL + 1))
    report_and_exit
fi

# -- Step 28: Admin resets the game ------------------------------------------

print_step 28 "Admin resets the game" "OK — game status RESET, yaks cleared"
invoke_lambda "$EVENTS_DIR/admin.mutation.resetGame.json"
print_response
check_success 28

# -- Step 29: Query game state (after reset) ---------------------------------
# Expected: 2 players, 0 yaks, status RESET

print_step 29 "Query game state (after reset)" "OK — 2 players, 0 yaks, status RESET"
invoke_lambda "$EVENTS_DIR/query.gameState.json"
print_response
GAME_STATUS=$(extract_field 'game_status')
PLAYER_COUNT=$(extract_field 'players | length')
YAK_TOTAL=$(extract_field 'yak_counts | del(.in_nursery) | del(.total_sheared) | to_entries | map(.value) | add')
if [[ "$GAME_STATUS" == "RESET" && "$PLAYER_COUNT" == "2" && "$YAK_TOTAL" == "0" ]]; then
    echo -e "  ${GREEN}${BOLD}[STEP 29] OK${NC} (status=$GAME_STATUS, players=$PLAYER_COUNT, total_yaks=$YAK_TOTAL)"
    PASS=$((PASS + 1))
else
    echo -e "  ${RED}${BOLD}[STEP 29] NOK${NC} (status=$GAME_STATUS, players=$PLAYER_COUNT, total_yaks=$YAK_TOTAL)"
    FAIL=$((FAIL + 1))
    report_and_exit
fi

# =============================================================================
# SUMMARY — only reached if all steps passed
# =============================================================================

report_and_exit
