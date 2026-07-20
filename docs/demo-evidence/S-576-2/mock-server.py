#!/usr/bin/env python3
"""
Minimal mock Jira API server for S-576-2 demo recordings.
Handles both the attachment list (S-576-1 style) AND the S-576-2 download endpoints:
  GET /rest/api/3/issue/<KEY>?fields=attachment
  GET /rest/api/3/attachment/<AID>            (metadata)
  GET /rest/api/3/attachment/content/<AID>    (file bytes)

Issue key routing:
  DEMO-10  -> 2 attachments: 30001=architecture.pdf, 30002=screenshot.png
  DEMO-20  -> 0 attachments
  DEMO-30  -> 404
  DEMO-40  -> 401 (all routes)
  DEMO-50  -> 2 attachments (30001 ok, 30003 content-500 partial-fail)
  DEMO-60  -> 1 attachment with path-traversal filename ../../etc/passwd (AID 30004)
  DEMO-70  -> 5 attachments with varied timestamps/types (newest + filter demos)
  DEMO-80  -> 1 attachment with degenerate name ".." (AID 30005)
  DEMO-90  -> 1 attachment with CON.txt filename (AID 30006) for device-name demo

Attachment ID routing (metadata endpoint /rest/api/3/attachment/<AID>):
  30001..30006, 30010..30014 -> success with fixture data
  99404                      -> 404
  99401                      -> 401
  99403                      -> 403
  99500                      -> 500
  (any other AID)            -> 404

Content endpoint /rest/api/3/attachment/content/<AID>:
  30001  -> 1024 bytes of text
  30002  -> 512 bytes of text
  30003  -> 500 (used for partial-fail demo in DEMO-50)
  30004..30006, 30010..30014 -> 64 bytes of text
  (others) -> 404

Usage: python3 mock-server.py <port>
"""

import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import urlparse, parse_qs

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 19877

# ---------------------------------------------------------------------------
# Fixture helpers
# ---------------------------------------------------------------------------

def make_attachment(id_, filename, mime, size_bytes, created="2026-07-10T14:23:11.000+0000"):
    return {
        "id": id_,
        "self": f"https://demo.atlassian.net/rest/api/3/attachment/{id_}",
        "filename": filename,
        "author": {
            "self": "https://demo.atlassian.net/rest/api/3/user?accountId=acc001",
            "accountId": "acc001",
            "displayName": "Alice Smith",
            "avatarUrls": {"48x48": "https://demo.atlassian.net/avatar/48x48.png"},
            "accountType": "atlassian",
            "active": True,
            "timeZone": "UTC",
        },
        "created": created,
        "size": size_bytes,
        "mimeType": mime,
        "content": f"https://demo.atlassian.net/secure/attachment/{id_}/{filename}",
    }

# ---------------------------------------------------------------------------
# Issue fixtures (for GET /rest/api/3/issue/<KEY>?fields=attachment)
# ---------------------------------------------------------------------------

ISSUE_FIXTURES = {
    "DEMO-10": {
        "status": 200,
        "body": {
            "key": "DEMO-10",
            "fields": {
                "attachment": [
                    make_attachment("30001", "architecture.pdf", "application/pdf", 1024),
                    make_attachment("30002", "screenshot.png", "image/png", 512),
                ]
            }
        }
    },
    "DEMO-20": {
        "status": 200,
        "body": {
            "key": "DEMO-20",
            "fields": {"attachment": []}
        }
    },
    "DEMO-30": {
        "status": 404,
        "body": {"errorMessages": ["Issue Does Not Exist"], "errors": {}}
    },
    "DEMO-40": {
        "status": 401,
        "body": {"errorMessages": ["You must be logged in to access this resource."], "errors": {}}
    },
    "DEMO-50": {
        "status": 200,
        "body": {
            "key": "DEMO-50",
            "fields": {
                "attachment": [
                    make_attachment("30001", "architecture.pdf", "application/pdf", 1024),
                    make_attachment("30003", "report.csv", "text/csv", 256),
                ]
            }
        }
    },
    "DEMO-60": {
        "status": 200,
        "body": {
            "key": "DEMO-60",
            "fields": {
                "attachment": [
                    make_attachment("30004", "../../etc/passwd", "text/plain", 64),
                ]
            }
        }
    },
    "DEMO-70": {
        "status": 200,
        "body": {
            "key": "DEMO-70",
            "fields": {
                "attachment": [
                    make_attachment("30010", "report-old.pdf", "application/pdf", 512,
                                   created="2026-07-01T09:00:00.000+0000"),
                    make_attachment("30011", "data-mid.csv", "text/csv", 256,
                                   created="2026-07-05T10:30:00.000+0000"),
                    make_attachment("30012", "photo-new.jpg", "image/jpeg", 204800,
                                   created="2026-07-10T14:23:11.000+0000"),
                    make_attachment("30013", "diagram.png", "image/png", 51200,
                                   created="2026-07-08T11:00:00.000+0000"),
                    make_attachment("30014", "data-oldest.xlsx",
                                   "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                                   8192, created="2026-07-03T08:00:00.000+0000"),
                ]
            }
        }
    },
    "DEMO-80": {
        "status": 200,
        "body": {
            "key": "DEMO-80",
            "fields": {
                "attachment": [
                    make_attachment("30005", "..", "text/plain", 64),
                ]
            }
        }
    },
    "DEMO-90": {
        "status": 200,
        "body": {
            "key": "DEMO-90",
            "fields": {
                "attachment": [
                    make_attachment("30006", "CON.txt", "text/plain", 64),
                ]
            }
        }
    },
    # KEY 403 for batch path demo
    "DEMO-403": {
        "status": 403,
        "body": {"errorMessages": ["You do not have the permission to see the specified issue."], "errors": {}}
    },
}

# ---------------------------------------------------------------------------
# Attachment metadata fixtures (for GET /rest/api/3/attachment/<AID>)
# ---------------------------------------------------------------------------

ATTACHMENT_META = {
    "30001": make_attachment("30001", "architecture.pdf", "application/pdf", 1024),
    "30002": make_attachment("30002", "screenshot.png", "image/png", 512),
    "30003": make_attachment("30003", "report.csv", "text/csv", 256),
    "30004": make_attachment("30004", "../../etc/passwd", "text/plain", 64),
    "30005": make_attachment("30005", "..", "text/plain", 64),
    "30006": make_attachment("30006", "CON.txt", "text/plain", 64),
    "30010": make_attachment("30010", "report-old.pdf", "application/pdf", 512),
    "30011": make_attachment("30011", "data-mid.csv", "text/csv", 256),
    "30012": make_attachment("30012", "photo-new.jpg", "image/jpeg", 204800),
    "30013": make_attachment("30013", "diagram.png", "image/png", 51200),
    "30014": make_attachment("30014", "data-oldest.xlsx", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet", 8192),
    "99404": None,   # 404
    "99401": "401",  # 401
    "99403": "403",  # 403
    "99500": "500",  # 500
}

# ---------------------------------------------------------------------------
# Content bodies (for GET /rest/api/3/attachment/content/<AID>)
# ---------------------------------------------------------------------------

CONTENT_30001 = (b"# Architecture Overview\n\n"
                 b"This document describes the system architecture.\n\n"
                 b"Components:\n"
                 b"  - API Gateway\n"
                 b"  - Auth Service\n"
                 b"  - Data Layer\n\n"
                 + b"x" * (1024 - 120))  # pad to 1024 bytes

CONTENT_30002 = (b"\x89PNG\r\n\x1a\n"  # PNG magic bytes
                 + b"FAKE PNG DATA FOR DEMO " * 20
                 + b"x" * (512 - 8 - 440))[:512]

CONTENT_SMALL = b"Attachment content for demo recording.\n" * 2  # ~76 bytes


class MockHandler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        pass  # silence

    def send_json(self, status, body_dict):
        body = json.dumps(body_dict).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def send_bytes(self, status, data, content_type="application/octet-stream"):
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        parsed = urlparse(self.path)
        parts = [p for p in parsed.path.split("/") if p]
        # parts[0]='rest', parts[1]='api', parts[2]='3', ...

        # --- GET /rest/api/3/issue/<KEY> ---
        if (len(parts) >= 5 and parts[:4] == ["rest", "api", "3", "issue"]):
            key = parts[4].upper()
            fixture = ISSUE_FIXTURES.get(key)
            if fixture:
                self.send_json(fixture["status"], fixture["body"])
            else:
                self.send_json(404, {"errorMessages": ["Issue Does Not Exist"], "errors": {}})
            return

        # --- GET /rest/api/3/attachment/content/<AID> ---
        if (len(parts) >= 6 and parts[:5] == ["rest", "api", "3", "attachment", "content"]):
            aid = parts[5]
            if aid == "30001":
                self.send_bytes(200, CONTENT_30001, "application/pdf")
            elif aid == "30002":
                self.send_bytes(200, CONTENT_30002, "image/png")
            elif aid == "30003":
                # Partial-fail scenario: content returns 500
                self.send_json(500, {"errorMessages": ["Internal Server Error"], "errors": {}})
            elif aid in ("30004", "30005", "30006",
                         "30010", "30011", "30012", "30013", "30014"):
                self.send_bytes(200, CONTENT_SMALL, "application/octet-stream")
            else:
                self.send_json(404, {"errorMessages": ["Attachment not found"], "errors": {}})
            return

        # --- GET /rest/api/3/attachment/<AID> (metadata) ---
        if (len(parts) >= 5 and parts[:4] == ["rest", "api", "3", "attachment"]
                and parts[4] != "content"):
            aid = parts[4]
            meta = ATTACHMENT_META.get(aid)
            if meta is None and aid in ATTACHMENT_META:
                # Explicit 404
                self.send_json(404, {"errorMessages": [f"Attachment {aid} not found or not accessible."], "errors": {}})
                return
            if meta == "401":
                self.send_json(401, {"errorMessages": ["You must be logged in to access this resource."], "errors": {}})
                return
            if meta == "403":
                self.send_json(403, {"errorMessages": [f"Permission denied: cannot access attachment {aid}."], "errors": {}})
                return
            if meta == "500":
                self.send_json(500, {"errorMessages": ["Internal server error"], "errors": {}})
                return
            if meta and isinstance(meta, dict):
                self.send_json(200, meta)
                return
            # Unknown AID → 404
            self.send_json(404, {"errorMessages": [f"Attachment {aid} not found or not accessible."], "errors": {}})
            return

        # Fallback
        self.send_json(404, {"errorMessages": ["not found"], "errors": {}})


if __name__ == "__main__":
    server = HTTPServer(("127.0.0.1", PORT), MockHandler)
    print(f"mock-server listening on http://127.0.0.1:{PORT}", flush=True)
    server.serve_forever()
