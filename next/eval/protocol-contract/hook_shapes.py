"""Distinct hook payload shapes from a spool of recorded hook events, for regression tests.

Keeps only structure: event name, field names, background task type/status pairs, cron
count, notification type and whether an agent_id was present. No ids, paths or text.

    python3 -I hook_shapes.py ~/.keepline/spool/events > ../../crates/keeplined/tests/fixtures/hook_shapes.jsonl
"""
import json
import os
import sys

RUNTIMES = {"claude-code": "claude", "claude": "claude", "codex": "codex"}


def shape(event):
    p = event.get("payload")
    if not isinstance(p, dict) or not isinstance(p.get("hook_event_name"), str):
        return None
    tasks = p.get("background_tasks")
    crons = p.get("session_crons")
    return {
        "runtime": RUNTIMES.get(event.get("runtime"), "other"),
        "event": p["hook_event_name"],
        "keys": sorted(p.keys()),
        "tasks": None
        if not isinstance(tasks, list)
        else sorted([str(t.get("type", "")), str(t.get("status", ""))] for t in tasks if isinstance(t, dict)),
        "crons": len(crons) if isinstance(crons, list) else None,
        "notification": p.get("notification_type") if isinstance(p.get("notification_type"), str) else None,
        "agent_id": isinstance(p.get("agent_id"), str) and p["agent_id"] != "",
    }


def main(root):
    seen = {}
    for name in os.listdir(root):
        if not name.endswith(".json"):
            continue
        try:
            with open(os.path.join(root, name), encoding="utf-8") as f:
                s = shape(json.load(f))
        except (OSError, ValueError):
            continue
        if s is not None:
            seen.setdefault(json.dumps(s, sort_keys=True, ensure_ascii=False), 0)
            seen[json.dumps(s, sort_keys=True, ensure_ascii=False)] += 1
    for line in sorted(seen):
        print(line)
    print(f"{len(seen)} shapes from {sum(seen.values())} events", file=sys.stderr)


if __name__ == "__main__":
    main(sys.argv[1])
