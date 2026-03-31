# Local Testing

Run the Lambda and DynamoDB locally to integration-test the game logic without deploying to AWS.

## Prerequisites

All tools are provided by the Nix flake (`nix/flake.nix`). If you're not using Nix, ensure the following are installed:

- **Java 21+** JRE — runs DynamoDB Local
- **AWS CLI v2** — creates the local DynamoDB table
- **jq** — pretty-prints JSON responses and X-Ray traces
- **socat** — receives X-Ray UDP datagrams
- **cargo-lambda** — runs and invokes the Lambda locally
- **Rust toolchain** — compiles the Lambda

DynamoDB Local is downloaded automatically on first run to `~/.local/share/dynamodb_local/`.

## Usage

You need **3 separate terminals**. Start each script in order and leave it running.

### Terminal 1 — Infrastructure

```bash
./local_testing/1-infra.sh
```

Starts DynamoDB Local (in-memory), creates the backend table, and launches an X-Ray trace dumper on UDP:2000. Press **Ctrl+C** to cleanly shut everything down.

### Terminal 2 — Lambda

```bash
./local_testing/2-lambda.sh
```

Runs `cargo lambda watch` for the `appsync-source` Lambda. The first launch triggers a full Rust compilation — subsequent runs use incremental builds. Lambda logs appear in this terminal.

### Terminal 3 — Test scenario

```bash
./local_testing/3-scenarios.sh
```

Runs a 29-step integration test covering player registration, game lifecycle (start/stop/reset), yak buy/sell operations, secret validation, and error cases. Each step prints the request, response, and a colored **OK** / **NOK** verdict.

## How it works

The event payloads in `appsync_events/` contain placeholders (`#PLAYER_ID#`, `#PLAYER_SECRET#`, `#YAK_ID#`, `#PLAYER_NAME#`, `#NEW_NAME#`) that the scenario script substitutes at runtime using `sed`. Temp files are written to `/tmp/` and cleaned up after each invocation.

## Scenario steps

| #  | Operation | Expected |
|----|-----------|----------|
| 1  | Register TempPlayer | OK |
| 2  | Rename TempPlayer with wrong secret | Error: wrong secret |
| 3  | Rename to OffensiveName69 | OK |
| 4  | Admin removes TempPlayer | OK |
| 5  | Register Player1 | OK |
| 6  | Register Player2 | OK |
| 7  | Query game state | 2 players, 0 yaks, RESET |
| 8  | Player1 buy baby yak | Error: game not started |
| 9  | Admin reset | Error: not stopped |
| 10 | Admin stop | Error: not started |
| 11 | Admin start | OK |
| 12 | Player1 buy grown yak | Error: none available |
| 13 | Player1 buy unsheared yak | Error: none available |
| 14 | Player1 buy baby yak | OK (tracked yak created) |
| 15 | Player1 buy baby yak again | Error: already has job |
| 16 | Player1 sell unsheared yak | Error: wrong job |
| 17 | Player1 sell sheared yak | Error: wrong job |
| 18 | Player1 sell grown yak with wrong secret | Error: wrong secret |
| 19 | Player1 sell grown yak | OK (yak to warehouse) |
| 20 | Player2 buy grown yak | OK (verify yak ID) |
| 21 | Player2 sell unsheared yak | OK (yak to shearing shed) |
| 22 | Player2 buy baby yak | OK |
| 23 | Admin stop | OK |
| 24 | Player1 buy baby yak | Error: game stopped |
| 25 | Player1 buy unsheared yak | OK (works while stopped) |
| 26 | Player1 sell sheared yak | OK (yak deleted) |
| 27 | Query game state | 2 players, 1 yak, STOPPED |
| 28 | Admin reset | OK |
| 29 | Query game state | 2 players, 0 yaks, RESET |
