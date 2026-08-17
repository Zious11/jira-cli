#!/usr/bin/env python3
"""
Mock server for S-604-3 demo recordings (jr component delete safety).
Port: 19880

CAUTION (safety): this server is a LOCAL, in-memory stand-in for Jira. It
never talks to any real Jira instance. `jr` is pointed at it exclusively via
the JR_BASE_URL debug-only seam (see CLAUDE.md "AI Agent Notes"). No live
mutation of any real Jira project ever occurs while recording these demos.

Endpoints:
  GET    /rest/api/3/project/{key}/components   - list components for a project
  GET    /rest/api/3/component/{id}              - single component (numeric
                                                    source/target confirming GET)
  POST   /rest/api/3/search/jql                  - BC-8.2.007 pre-delete snapshot
                                                    (routed on the `component =
                                                    <id>` clause in the JQL body)
  DELETE /rest/api/3/component/{id}              - the irreversible delete itself

Fixture components (all project FOO):
  10001  Backend    - snapshot "component = 10001" -> 2 issues (FOO-101, FOO-102)
  10002  Frontend   - move-to target, no snapshot needed
  10003  DriftComp  - snapshot ALWAYS returns a repeating nextPageToken ("loop"),
                      simulating the JRACLOUD-95368 anti-loop-guard drift
                      condition (BC-8.2.007 fail-closed path). DELETE on this id
                      returns 500 so an accidental DELETE call is loud/obvious
                      in the recording, not silently "successful".
  10004  Orphaned   - snapshot "component = 10004" -> 5 issues (FOO-301..FOO-305)

DELETE 10001 (with ?moveIssuesTo=10002) and DELETE 10004 (no moveIssuesTo) both
return 204. Every request is logged to stdout for the recording session log.
"""
import json
import re
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import urlparse, parse_qs

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 19880

COMPONENTS_FOO = [
    {
        "id": "10001",
        "name": "Backend",
        "description": None,
        "lead": None,
        "assigneeType": None,
        "project": None,
    },
    {
        "id": "10002",
        "name": "Frontend",
        "description": None,
        "lead": None,
        "assigneeType": None,
        "project": None,
    },
    {
        "id": "10003",
        "name": "DriftComp",
        "description": None,
        "lead": None,
        "assigneeType": None,
        "project": None,
    },
    {
        "id": "10004",
        "name": "Orphaned",
        "description": None,
        "lead": None,
        "assigneeType": None,
        "project": None,
    },
]

SINGLE_COMPONENT = {c["id"]: {**c, "project": "FOO"} for c in COMPONENTS_FOO}


def issue_row(key):
    return {
        "id": "90000",
        "key": key,
        "self": f"https://demo.atlassian.net/rest/api/3/issue/{key}",
        "fields": {},
    }


class MockHandler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        print(f"  [{self.command}] {self.path} -> {args[0] if args else ''}")

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

        m = re.match(r"^/rest/api/3/project/([^/]+)/components$", path)
        if m:
            key = m.group(1)
            if key == "FOO":
                self.send_json(200, COMPONENTS_FOO)
            else:
                self.send_json(200, [])
            return

        m = re.match(r"^/rest/api/3/component/([^/]+)$", path)
        if m:
            cid = m.group(1)
            if cid in SINGLE_COMPONENT:
                self.send_json(200, SINGLE_COMPONENT[cid])
            else:
                self.send_json(
                    404,
                    {"errorMessages": ["The component with id " + cid + " does not exist."], "errors": {}},
                )
            return

        self.send_json(404, {"errorMessages": ["Not found"], "errors": {}})

    def do_POST(self):
        parsed = urlparse(self.path)
        path = parsed.path
        length = int(self.headers.get("Content-Length", 0))
        raw = self.rfile.read(length) if length else b"{}"
        try:
            body = json.loads(raw)
        except Exception:
            body = {}

        if path == "/rest/api/3/search/jql":
            jql = body.get("jql", "")
            # BC-8.2.007: composed JQL is always "component = <resolvedId> ORDER BY key ASC"
            if "component = 10001" in jql:
                self.send_json(
                    200,
                    {"issues": [issue_row("FOO-101"), issue_row("FOO-102")], "isLast": True},
                )
                return
            if "component = 10003" in jql:
                # JRACLOUD-95368 drift simulation: ALWAYS returns the SAME
                # nextPageToken, regardless of the token the client just sent
                # back — the anti-loop guard aborts after seeing the repeat.
                self.send_json(
                    200,
                    {
                        "issues": [issue_row("FOO-201")],
                        "isLast": False,
                        "nextPageToken": "loop",
                    },
                )
                return
            if "component = 10004" in jql:
                self.send_json(
                    200,
                    {
                        "issues": [issue_row(f"FOO-30{i}") for i in range(1, 6)],
                        "isLast": True,
                    },
                )
                return
            # Unknown/unmapped snapshot query.
            self.send_json(200, {"issues": [], "isLast": True})
            return

        self.send_json(404, {"errorMessages": ["Not found"], "errors": {}})

    def do_DELETE(self):
        parsed = urlparse(self.path)
        path = parsed.path
        qs = parse_qs(parsed.query)
        m = re.match(r"^/rest/api/3/component/([^/]+)$", path)
        if not m:
            self.send_json(404, {"errorMessages": ["Not found"], "errors": {}})
            return
        cid = m.group(1)

        if cid == "10003":
            # Should be UNREACHABLE in a correctly fail-closed implementation
            # (BC-8.2.007) — loud 500 so a regression is obvious in the log,
            # not a quiet 204 that could be mistaken for "it worked".
            self.send_json(
                500,
                {"errorMessages": ["TEST-HARNESS: DELETE must never fire after a fail-closed snapshot abort."]},
            )
            return

        if cid in ("10001", "10004"):
            self.send_response(204)
            self.end_headers()
            return

        self.send_json(404, {"errorMessages": ["Component does not exist."], "errors": {}})


if __name__ == "__main__":
    server = HTTPServer(("127.0.0.1", PORT), MockHandler)
    print(f"mock-server listening on http://127.0.0.1:{PORT}")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
