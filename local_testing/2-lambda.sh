#!/usr/bin/env bash
# =============================================================================
# 2-lambda.sh — Run the appsync-source Lambda locally
# =============================================================================
# This script launches `cargo lambda watch` for the appsync-source Lambda.
# It must be run from its own terminal (Terminal 2).
#
# The Lambda will listen for invocations from `cargo lambda invoke` (Script 3).
# It connects to the local DynamoDB started by Script 1.
#
# Environment variables configure:
#   - Logging: verbose for the Lambda, quiet for HTTP/TLS internals
#   - AWS credentials: fake values (DynamoDB Local doesn't check them)
#   - DynamoDB endpoint: redirected to localhost:8000
# =============================================================================

set -euo pipefail
MANIFEST_PATH=$(cargo metadata --format-version=1 | jq -r .workspace_root)/Cargo.toml
LAMBDA_PKG="appsync-source"

echo "Starting cargo lambda watch"
echo "The Lambda will connect to DynamoDB at http://localhost:8000"
echo "========================================================================="

# RUST_LOG: debug for the Lambda code, info for noisy dependencies
# AWS_*: fake credentials — DynamoDB Local accepts anything
# BACKEND_TABLE_NAME: must match the table created by 1-infra.sh
# AWS_ENDPOINT_URL_DYNAMODB: redirect DynamoDB calls to the local instance
RUST_LOG=debug,hyper=info,h2=info,tracing=info,aws_config=info,aws_smithy_runtime=info,aws_smithy_runtime_api=info,rustls=info \
AWS_ACCESS_KEY_ID="fakeMyKeyId" \
AWS_SECRET_ACCESS_KEY="fakeSecretAccessKey" \
AWS_REGION=eu-west-3 \
BACKEND_TABLE_NAME=yak-mania-backend \
AWS_ENDPOINT_URL_DYNAMODB=http://localhost:8000 \
cargo lambda watch --manifest-path $MANIFEST_PATH -p $LAMBDA_PKG
