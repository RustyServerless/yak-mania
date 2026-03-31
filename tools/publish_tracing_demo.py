#!/usr/bin/env python3
"""Publish a dummy message to the tracing-demo SNS topic with a valid X-Ray trace context.

This ensures SNS receives an upstream trace header and propagates it through
SQS to the subscriber Lambdas, producing a single connected trace instead of
four independent ones.

Usage:
    python tools/publish_tracing_demo.py <topic-arn>
"""

import argparse
import json
import secrets
import time

import boto3


def generate_trace_id() -> str:
    """Generate a valid X-Ray trace ID: 1-<hex_time>-<24_hex_chars>."""
    hex_time = format(int(time.time()), "08x")
    unique = secrets.token_hex(12)
    return f"1-{hex_time}-{unique}"


def generate_segment_id() -> str:
    """Generate a 16-character hex segment/parent ID."""
    return secrets.token_hex(8)


def put_root_segment(xray_client, trace_id: str, segment_id: str) -> None:
    """Send a root segment to X-Ray so the trace actually exists."""
    segment = {
        "trace_id": trace_id,
        "id": segment_id,
        "name": "tracing-demo-publisher",
        "start_time": time.time(),
        "end_time": time.time(),
        "in_progress": False,
    }
    xray_client.put_trace_segments(TraceSegmentDocuments=[json.dumps(segment)])


def inject_trace_header(trace_id: str, parent_id: str):
    """Return a boto3 event handler that injects the X-Amzn-Trace-Id header."""

    def _add_header(params, **_kwargs):
        header = f"Root={trace_id};Parent={parent_id};Sampled=1"
        params["headers"]["X-Amzn-Trace-Id"] = header

    return _add_header


def main():
    parser = argparse.ArgumentParser(
        description="Publish a traced dummy message to an SNS topic."
    )
    parser.add_argument("topic_arn", help="ARN of the SNS topic to publish to")
    parser.add_argument(
        "--message", default="tracing-demo test message", help="Message body to publish"
    )
    args = parser.parse_args()

    region = args.topic_arn.split(":")[3]
    session = boto3.Session(region_name=region)

    trace_id = generate_trace_id()
    parent_id = generate_segment_id()

    # Register the root segment in X-Ray
    xray_client = session.client("xray")
    put_root_segment(xray_client, trace_id, parent_id)

    # Create an SNS client with the trace header injected into every request
    sns_client = session.client("sns")
    sns_client.meta.events.register(
        "before-call.sns.Publish", inject_trace_header(trace_id, parent_id)
    )

    response = sns_client.publish(
        TopicArn=args.topic_arn,
        Message=args.message,
    )

    print(f"Trace ID : {trace_id}")
    print(f"Parent ID: {parent_id}")
    print(f"Message ID: {response['MessageId']}")


if __name__ == "__main__":
    main()
