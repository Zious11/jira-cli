#!/usr/bin/env python3
"""
Minimal mock Jira API server for S-576-1 demo recordings.
Serves GET /rest/api/3/issue/<KEY>?fields=attachment with canned fixtures.

Key routing:
  DEMO-1  -> multi-attachment success (table demo)
  DEMO-2  -> zero attachments
  DEMO-3  -> 404
  DEMO-4  -> 401
  DEMO-5  -> 403
  DEMO-6  -> 500
  DEMO-7  -> mixed-type attachments (filter demos)

Usage: python3 mock-server.py <port>
"""

import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import urlparse, parse_qs

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 19876

# ---------------------------------------------------------------------------
# Fixture data
# ---------------------------------------------------------------------------

def attachment(id_, filename, mime, size_bytes, display_name="Alice Smith", account_id="acc001"):
    return {
        "id": id_,
        "self": f"https://demo.atlassian.net/rest/api/3/attachment/{id_}",
        "filename": filename,
        "author": {
            "self": "https://demo.atlassian.net/rest/api/3/user?accountId=acc001",
            "accountId": account_id,
            "displayName": display_name,
            "avatarUrls": {"48x48": "https://demo.atlassian.net/avatar/48x48.png"},
            "accountType": "atlassian",
            "active": True,
            "timeZone": "UTC",
        },
        "created": "2026-07-10T14:23:11.000+0000",
        "size": size_bytes,
        "mimeType": mime,
        "content": f"https://demo.atlassian.net/secure/attachment/{id_}/{filename}",
    }

FIXTURES = {
    "DEMO-1": {
        "status": 200,
        "body": {
            "key": "DEMO-1",
            "fields": {
                "attachment": [
                    attachment("10001", "architecture-overview.pdf", "application/pdf", 43008),
                    attachment("10002", "screenshot.png", "image/png", 204800,
                               display_name="Bob Jones", account_id="acc002"),
                    attachment("10003", "test-results.csv", "text/csv", 8192),
                ]
            }
        }
    },
    "DEMO-2": {
        "status": 200,
        "body": {
            "key": "DEMO-2",
            "fields": {"attachment": []}
        }
    },
    "DEMO-3": {
        "status": 404,
        "body": {"errorMessages": ["Issue Does Not Exist"], "errors": {}}
    },
    "DEMO-4": {
        "status": 401,
        "body": {"errorMessages": ["You must be logged in to access this resource."], "errors": {}}
    },
    "DEMO-5": {
        "status": 403,
        "body": {"errorMessages": ["You do not have the permission to see the specified issue."], "errors": {}}
    },
    "DEMO-6": {
        "status": 500,
        "body": {"errorMessages": ["Internal server error"], "errors": {}}
    },
    "DEMO-7": {
        "status": 200,
        "body": {
            "key": "DEMO-7",
            "fields": {
                "attachment": [
                    attachment("20001", "report-A.pdf", "application/pdf", 1024),
                    attachment("20002", "report-1.pdf", "application/pdf", 2048),
                    attachment("20003", "report-10.pdf", "application/pdf", 65536),
                    attachment("20004", "photo.jpg", "image/jpeg", 51200),
                    attachment("20005", "diagram.png", "image/png", 204800),
                    attachment("20006", "data.csv", "text/csv", 512),
                ]
            }
        }
    },
}


class MockHandler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        pass  # silence

    def do_GET(self):
        parsed = urlparse(self.path)
        # path: /rest/api/3/issue/<KEY>
        parts = parsed.path.split("/")
        # parts = ['', 'rest', 'api', '3', 'issue', '<KEY>']
        if len(parts) >= 6 and parts[1:5] == ["rest", "api", "3", "issue"]:
            key = parts[5].upper()
            fixture = FIXTURES.get(key)
            if fixture:
                body = json.dumps(fixture["body"]).encode()
                self.send_response(fixture["status"])
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
                return
        self.send_response(404)
        body = b'{"errorMessages":["not found"]}'
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


if __name__ == "__main__":
    server = HTTPServer(("127.0.0.1", PORT), MockHandler)
    print(f"mock-server listening on http://127.0.0.1:{PORT}", flush=True)
    server.serve_forever()
