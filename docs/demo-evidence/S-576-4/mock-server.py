#!/usr/bin/env python3
"""
Mock server for S-576-4 demo recordings (jr issue attachment delete).
Port: 19879
Endpoints:
  GET  /rest/api/3/attachment/{id}       - metadata fetch (gate pre-prompt)
  DELETE /rest/api/3/attachment/{id}     - delete attachment
  GET  /rest/api/3/issue/{key}?fields=attachment - issue attachment list

AID routing:
  99001  → GET: report.pdf (old 2025-12-01); DELETE: 204
  99002  → GET: notes.txt (old 2026-01-15); DELETE: 204
  99003  → GET: recent.pdf (new 2026-07-20); DELETE: 204
  99404  → GET: 404 (with DEC-168 body); DELETE: 404 DEC-168 body
  99403  → GET: 403; DELETE: 403
  99500  → GET: 500; DELETE: 500
  99998  → DELETE: 404 (benign skip in bulk — "already deleted")
  99999  → DELETE: 204 (third AID in partial-404 bulk test)

Issue fixtures:
  DEMO-1   → 2 old attachments (99001, 99002) — both >30d
  DEMO-2   → empty
  DEMO-10  → 1 old (99001) + 1 recent (99003)
  DEMO-404 → 404 issue not found
"""
import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import urlparse, parse_qs

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 19879

# Attachment metadata fixtures
ATTACHMENT_META = {
    "99001": {
        "id": "99001",
        "filename": "report.pdf",
        "mimeType": "application/pdf",
        "size": 43008,
        "created": "2025-12-01T09:00:00.000+0000",
        "author": {"accountId": "demo-uid-1", "displayName": "Demo User"},
        "self": "http://demo.atlassian.net/rest/api/3/attachment/99001",
        "content": "http://demo.atlassian.net/rest/api/3/attachment/content/99001",
    },
    "99002": {
        "id": "99002",
        "filename": "notes.txt",
        "mimeType": "text/plain",
        "size": 1024,
        "created": "2026-01-15T14:30:00.000+0000",
        "author": {"accountId": "demo-uid-1", "displayName": "Demo User"},
        "self": "http://demo.atlassian.net/rest/api/3/attachment/99002",
        "content": "http://demo.atlassian.net/rest/api/3/attachment/content/99002",
    },
    "99003": {
        "id": "99003",
        "filename": "recent.pdf",
        "mimeType": "application/pdf",
        "size": 2048,
        "created": "2026-07-20T10:00:00.000+0000",
        "author": {"accountId": "demo-uid-1", "displayName": "Demo User"},
        "self": "http://demo.atlassian.net/rest/api/3/attachment/99003",
        "content": "http://demo.atlassian.net/rest/api/3/attachment/content/99003",
    },
    "99999": {
        "id": "99999",
        "filename": "third.txt",
        "mimeType": "text/plain",
        "size": 512,
        "created": "2025-11-01T08:00:00.000+0000",
        "author": {"accountId": "demo-uid-1", "displayName": "Demo User"},
        "self": "http://demo.atlassian.net/rest/api/3/attachment/99999",
        "content": "http://demo.atlassian.net/rest/api/3/attachment/content/99999",
    },
}

ISSUE_FIXTURES = {
    "DEMO-1": {
        "key": "DEMO-1",
        "id": "10001",
        "fields": {
            "attachment": [
                ATTACHMENT_META["99001"],
                ATTACHMENT_META["99002"],
            ]
        },
    },
    "DEMO-2": {
        "key": "DEMO-2",
        "id": "10002",
        "fields": {"attachment": []},
    },
    "DEMO-10": {
        "key": "DEMO-10",
        "id": "10010",
        "fields": {
            "attachment": [
                ATTACHMENT_META["99001"],  # old (2025-12-01)
                ATTACHMENT_META["99003"],  # recent (2026-07-20)
            ]
        },
    },
    "DEMO-404": None,
}


class MockHandler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        print(f"  [{self.command}] {self.path} → {args[0] if args else ''}")

    def send_json(self, code, body):
        data = json.dumps(body).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        parsed = urlparse(self.path)
        path = parsed.path
        qs = parse_qs(parsed.query)

        # GET /rest/api/3/attachment/{id} — metadata
        if path.startswith("/rest/api/3/attachment/") and "content" not in path:
            aid = path.split("/rest/api/3/attachment/")[-1].strip("/")
            if aid == "99404":
                self.send_json(404, {
                    "errorMessages": ["Attachment does not exist."],
                    "errors": {},
                })
            elif aid == "99403":
                self.send_json(403, {
                    "errorMessages": ["Permission denied."],
                    "errors": {},
                })
            elif aid == "99500":
                self.send_json(500, {
                    "errorMessages": ["Internal Server Error."],
                    "errors": {},
                })
            elif aid in ATTACHMENT_META:
                self.send_json(200, ATTACHMENT_META[aid])
            else:
                # Default: return a generic attachment metadata
                self.send_json(200, {
                    "id": aid,
                    "filename": f"file-{aid}.txt",
                    "mimeType": "text/plain",
                    "size": 256,
                    "created": "2025-10-01T00:00:00.000+0000",
                    "author": {"accountId": "demo-uid-1", "displayName": "Demo User"},
                })
            return

        # GET /rest/api/3/issue/{key}?fields=attachment
        if path.startswith("/rest/api/3/issue/"):
            key = path.split("/rest/api/3/issue/")[-1].strip("/")
            if key not in ISSUE_FIXTURES or ISSUE_FIXTURES[key] is None:
                self.send_json(404, {
                    "errorMessages": [f"Issue Does Not Exist"],
                    "errors": {},
                })
                return
            self.send_json(200, ISSUE_FIXTURES[key])
            return

        self.send_json(404, {"errorMessages": ["Not found"], "errors": {}})

    def do_DELETE(self):
        parsed = urlparse(self.path)
        path = parsed.path
        aid = path.split("/rest/api/3/attachment/")[-1].strip("/")

        if aid == "99404":
            self.send_json(404, {
                "errorMessages": ["Attachment does not exist."],
                "errors": {},
            })
        elif aid == "99403":
            self.send_json(403, {
                "errorMessages": ["Permission denied."],
                "errors": {},
            })
        elif aid == "99500":
            self.send_json(500, {
                "errorMessages": ["Internal Server Error."],
                "errors": {},
            })
        elif aid == "99998":
            # benign 404 in bulk (already deleted / stale)
            self.send_json(404, {
                "errorMessages": ["Attachment does not exist."],
                "errors": {},
            })
        else:
            # 204 No Content success
            self.send_response(204)
            self.end_headers()


if __name__ == "__main__":
    server = HTTPServer(("127.0.0.1", PORT), MockHandler)
    print(f"mock-server listening on http://127.0.0.1:{PORT}")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
