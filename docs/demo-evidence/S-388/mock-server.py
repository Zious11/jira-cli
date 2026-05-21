#!/usr/bin/env python3
"""
Tiny HTTP mock server for S-388 demo recordings.

Usage: python3 mock-server.py <scenario> <port>

Scenarios:
  cross_std_to_subtask  - PUT TEST-1 -> 400; GET issue -> subtask:false; GET project -> subtask:true target
  cross_subtask_to_std  - PUT SUB-1  -> 400; GET issue -> subtask:true; GET project -> subtask:false target
  same_hierarchy_typo   - PUT TEST-2 -> 400; GET issue -> subtask:false; GET project -> subtask:false target
  no_parent_subtask     - PUT TEST-NP -> 400 (subtask parent error); GET issue info
  indeterminate         - PUT TEST-4  -> 400; GET issue -> subtask:false; GET project -> 503
  typo_name             - PUT TEST-8  -> 400; GET issue -> subtask:false; GET project -> list w/o Taks
"""

import sys
import json
from http.server import HTTPServer, BaseHTTPRequestHandler

SCENARIO = sys.argv[1] if len(sys.argv) > 1 else "cross_std_to_subtask"
PORT = int(sys.argv[2]) if len(sys.argv) > 2 else 9999

EDIT_400 = {
    "errorMessages": [],
    "errors": {"issuetype": "The issue type selected is invalid."}
}

EDIT_400_PARENT = {
    "errorMessages": [],
    "errors": {"parent": "Cannot remove parent from a sub-task."}
}

ISSUE_STANDARD = {
    "key": "TEST-1",
    "fields": {
        "summary": "A standard issue",
        "status": {"name": "To Do"},
        "issuetype": {"name": "Task", "subtask": False},
        "priority": {"name": "Medium"},
        "assignee": None,
        "project": {"key": "TEST", "name": "Test Project"}
    }
}

ISSUE_SUBTASK = {
    "key": "SUB-1",
    "fields": {
        "summary": "A sub-task issue",
        "status": {"name": "To Do"},
        "issuetype": {"name": "Sub-task", "subtask": True},
        "priority": {"name": "Medium"},
        "assignee": None,
        "project": {"key": "TEST", "name": "Test Project"}
    }
}

ISSUE_NO_PARENT = {
    "key": "TEST-NP",
    "fields": {
        "summary": "A sub-task issue",
        "status": {"name": "To Do"},
        "issuetype": {"name": "Sub-task", "subtask": True},
        "priority": {"name": "Medium"},
        "assignee": None,
        "project": {"key": "TEST", "name": "Test Project"}
    }
}

PROJECT_WITH_SUBTASK = {
    "key": "TEST",
    "name": "Test Project",
    "issueTypes": [
        {"name": "Task", "subtask": False},
        {"name": "Bug", "subtask": False},
        {"name": "Sub-task", "subtask": True}
    ]
}

PROJECT_STANDARD_ONLY = {
    "key": "TEST",
    "name": "Test Project",
    "issueTypes": [
        {"name": "Task", "subtask": False},
        {"name": "Bug", "subtask": False},
        {"name": "Story", "subtask": False}
    ]
}

PROJECT_NO_TAKS = {
    "key": "TEST",
    "name": "Test Project",
    "issueTypes": [
        {"name": "Story", "subtask": False},
        {"name": "Bug", "subtask": False},
        {"name": "Task", "subtask": False}
    ]
}

PROJECT_503 = {
    "errorMessages": ["Service temporarily unavailable."],
    "errors": {}
}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        pass  # suppress request logging

    def send_json(self, code, data):
        body = json.dumps(data).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_PUT(self):
        self.rfile.read(int(self.headers.get("Content-Length", 0)))
        if SCENARIO == "no_parent_subtask":
            self.send_json(400, EDIT_400_PARENT)
        else:
            self.send_json(400, EDIT_400)

    def do_GET(self):
        path = self.path.split("?")[0]

        # Issue GET
        if "/rest/api/3/issue/" in path:
            if SCENARIO == "cross_std_to_subtask":
                self.send_json(200, ISSUE_STANDARD)
            elif SCENARIO == "cross_subtask_to_std":
                self.send_json(200, ISSUE_SUBTASK)
            elif SCENARIO == "same_hierarchy_typo":
                self.send_json(200, ISSUE_STANDARD)
            elif SCENARIO == "no_parent_subtask":
                self.send_json(200, ISSUE_NO_PARENT)
            elif SCENARIO == "indeterminate":
                self.send_json(200, ISSUE_STANDARD)
            elif SCENARIO == "typo_name":
                self.send_json(200, ISSUE_STANDARD)
            else:
                self.send_json(200, ISSUE_STANDARD)
            return

        # Project GET
        if "/rest/api/3/project/" in path:
            if SCENARIO == "cross_std_to_subtask":
                self.send_json(200, PROJECT_WITH_SUBTASK)
            elif SCENARIO == "cross_subtask_to_std":
                self.send_json(200, PROJECT_STANDARD_ONLY)
            elif SCENARIO == "same_hierarchy_typo":
                self.send_json(200, PROJECT_STANDARD_ONLY)
            elif SCENARIO == "no_parent_subtask":
                self.send_json(200, PROJECT_WITH_SUBTASK)
            elif SCENARIO == "indeterminate":
                self.send_json(503, PROJECT_503)
            elif SCENARIO == "typo_name":
                self.send_json(200, PROJECT_NO_TAKS)
            else:
                self.send_json(200, PROJECT_STANDARD_ONLY)
            return

        # Fallback: 404
        self.send_response(404)
        self.end_headers()


httpd = HTTPServer(("127.0.0.1", PORT), Handler)
httpd.serve_forever()
