import json
import os

from aws_xray_sdk.core import patch_all

patch_all()

import boto3

TABLE_NAME = os.environ["TABLE_NAME"]
LAMBDA_NAME = os.environ["LAMBDA_NAME"]
table = boto3.resource("dynamodb").Table(TABLE_NAME)


def handler(event, context):
    print(json.dumps(event, default=str))
    for record in event["Records"]:
        sns_message = json.loads(record["body"])
        message_id = sns_message["MessageId"]
        table.put_item(Item={"PK": message_id, "SK": LAMBDA_NAME})
