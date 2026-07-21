#!/usr/bin/env python3
"""
Minimal mock Jira API server for S-576-3 demo recordings.
Handles upload (POST multipart), issue attachment list (GET), and delete (DELETE).

Endpoints:
  GET  /rest/api/3/issue/<KEY>?fields=attachment  -> attachment list
  POST /rest/api/3/issue/<KEY>/attachments        -> upload response
  DELETE /rest/api/3/attachment/<AID>             -> delete attachment

Issue fixtures:
  DEMO-1  -> 2 attachments (upload target, replace-existing demo)
  DEMO-2  -> 0 attachments (replace-existing zero-match demo)
  DEMO-3  -> 404
  DEMO-4  -> 401 on issue GET
  DEMO-5  -> 403 on POST
  DEMO-6  -> 413 on POST (file too large)

Upload response (POST /rest/api/3/issue/<KEY>/attachments):
  Returns a JSON array (Jira echo format) for successful uploads.

Delete response (DELETE /rest/api/3/attachment/<AID>):
  204 for known AIDs, 404 for AID 99404 (benign-skip demo).

Usage: python3 mock-server.py <port>
"""

import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import urlparse

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 19878

REQUEST_LOG = []  # global request log for ordering assertion demos


def make_attachment(id_, filename, mime="application/pdf", size=1024,
                    created="2026-07-10T14:23:11.000+0000"):
    return {
        "id": id_,
        "self": f"https://demo.atlassian.net/rest/api/3/attachment/{id_}",
        "filename": filename,
        "author": {
            "self": "https://demo.atlassian.net/rest/api/3/user?accountId=acc001",
            "accountId": "acc001",
            "displayName": "Alice Smith",
        },
        "created": created,
        "size": size,
        "mimeType": mime,
        "content": f"https://demo.atlassian.net/secure/attachment/{id_}/{filename}",
    }


ISSUE_FIXTURES = {
    "DEMO-1": {
        "status": 200,
        "body": {
            "key": "DEMO-1",
            "fields": {
                "attachment": [
                    make_attachment("40001", "report.pdf", size=2048),
                    make_attachment("40002", "report.pdf", size=2048),  # duplicate name
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
    # DEMO-5 and DEMO-6 have valid issue GET, but POST errors
    "DEMO-5": {
        "status": 200,
        "body": {"key": "DEMO-5", "fields": {"attachment": []}}
    },
    "DEMO-6": {
        "status": 200,
        "body": {"key": "DEMO-6", "fields": {"attachment": []}}
    },
}

# Upload POST response fixtures by KEY
UPLOAD_RESPONSES = {
    "DEMO-1": {"status": 200, "body": [make_attachment("50001", "report.pdf", size=43008)]},
    "DEMO-2": {"status": 200, "body": [make_attachment("50002", "diagram.pdf", size=8192)]},
    "DEMO-3": {"status": 404, "body": {"errorMessages": ["Issue Does Not Exist"], "errors": {}}},
    "DEMO-5": {"status": 403, "body": {"errorMessages": ["Forbidden"], "errors": {}}},
    "DEMO-4": {"status": 401, "body": {"errorMessages": ["You must be logged in to access this resource."], "errors": {}}},
    "DEMO-6": {"status": 413, "body": {"errorMessages": ["Request Entity Too Large"], "errors": {}}},
    "DEMO-MULTI": {"status": 200, "body": [
        make_attachment("50003", "file1.txt", mime="text/plain", size=512),
        make_attachment("50004", "file2.txt", mime="text/plain", size=256),
    ]},
    # Default for any other key
    "_default": {"status": 200, "body": [make_attachment("50099", "upload.txt", mime="text/plain", size=128)]},
}

# For dry-run: issue fixture for DEMO-1 is reused (list GET fires, no mutations)

KNOWN_DELETE_AIDS = {"40001", "40002"}  # valid for delete


class MockHandler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        pass  # silence

    def send_json(self, status, body):
        data = json.dumps(body).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        parsed = urlparse(self.path)
        parts = [p for p in parsed.path.split("/") if p]
        # GET /rest/api/3/issue/<KEY>
        if len(parts) >= 5 and parts[:4] == ["rest", "api", "3", "issue"]:
            key = parts[4].upper()
            fixture = ISSUE_FIXTURES.get(key)
            if fixture:
                self.send_json(fixture["status"], fixture["body"])
            else:
                self.send_json(200, {"key": key, "fields": {"attachment": []}})
            return
        self.send_json(404, {"errorMessages": ["not found"], "errors": {}})

    def do_POST(self):
        parsed = urlparse(self.path)
        parts = [p for p in parsed.path.split("/") if p]
        # POST /rest/api/3/issue/<KEY>/attachments
        if (len(parts) >= 6 and parts[:4] == ["rest", "api", "3", "issue"]
                and parts[5] == "attachments"):
            key = parts[4].upper()
            REQUEST_LOG.append(("POST", f"/rest/api/3/issue/{key}/attachments"))

            # Consume the body (needed to avoid broken pipe)
            content_length = int(self.headers.get("Content-Length", 0))
            _ = self.rfile.read(content_length) if content_length > 0 else b""

            resp = UPLOAD_RESPONSES.get(key, UPLOAD_RESPONSES["_default"])

            # DEMO-MULTI key for multi-file uploads
            if key == "DEMO-MULTI" or key.startswith("MULTI"):
                resp = UPLOAD_RESPONSES["DEMO-MULTI"]

            self.send_json(resp["status"], resp["body"])
            return
        self.send_json(404, {"errorMessages": ["not found"], "errors": {}})

    def do_DELETE(self):
        parsed = urlparse(self.path)
        parts = [p for p in parsed.path.split("/") if p]
        # DELETE /rest/api/3/attachment/<AID>
        if len(parts) >= 5 and parts[:4] == ["rest", "api", "3", "attachment"]:
            aid = parts[4]
            REQUEST_LOG.append(("DELETE", f"/rest/api/3/attachment/{aid}"))

            if aid == "99404":
                # Benign 404 (stale AID)
                self.send_json(404, {"errorMessages": ["Attachment not found"], "errors": {}})
            elif aid in KNOWN_DELETE_AIDS:
                # 204 No Content
                self.send_response(204)
                self.send_header("Content-Length", "0")
                self.end_headers()
            else:
                # Unknown AID — treat as 204 for simplicity
                self.send_response(204)
                self.send_header("Content-Length", "0")
                self.end_headers()
            return
        self.send_json(404, {"errorMessages": ["not found"], "errors": {}})


if __name__ == "__main__":
    server = HTTPServer(("127.0.0.1", PORT), MockHandler)
    print(f"mock-server listening on http://127.0.0.1:{PORT}", flush=True)
    server.serve_forever()
