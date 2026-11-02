#!/usr/bin/env python3
"""Collect the texts behind 「等你回答」/「出错了」 for the reasons rules (keepline-core/assets/reasons.json).

Reads, the way keeplined does:
  - Claude Code transcripts (~/.claude/projects): AskUserQuestion questions, permission
    denials ("requires approval", "haven't granted"), API error messages;
  - Codex rollouts (~/.codex/sessions): request_user_input questions, approval outputs;
  - <config_dir>/reasons-misses.jsonl: texts the app saw and no rule took.

Writes one JSON line per distinct text: {"kind", "text", "count", "from"}, sanitized for the
repo: a question keeps only its short title (as keeplined writes it, 【title】), paths under
$HOME become ~, ids and request ids are dropped, and text is cut to 200 characters.

  scripts/mine-reasons.py > /tmp/reasons.jsonl              # everything
  scripts/mine-reasons.py --misses                          # only what the app missed
  scripts/mine-reasons.py --check crates/keepline-core/tests/reasons-corpus.jsonl
      # prints rows not yet in the corpus, to review and append
"""
import argparse, collections, glob, json, os, re, sys

APPROVAL = ("requires approval", "haven't granted", "waiting for approval")
KEYS = ("AskUserQuestion", "isApiErrorMessage", "request_user_input") + APPROVAL
HOME = os.path.expanduser("~")


def config_dir():
    base = os.environ.get("XDG_CONFIG_HOME") or os.path.join(HOME, ".config")
    return os.path.join(base, "keepline")


def text_of(v):
    if isinstance(v, str):
        return v
    if isinstance(v, list):
        return "\n".join(text_of(x) for x in v)
    if isinstance(v, dict):
        for k in ("text", "content"):
            if k in v:
                return text_of(v[k])
    return ""


def clean(t):
    t = t.replace(HOME, "~")
    t = re.sub(r'"request_id":"[^"]*"', '"request_id":"…"', t)
    t = re.sub(r"\[[0-9a-f]{20,}\]", "[…]", t)
    t = re.sub(r"\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b", "…", t)
    t = re.sub(r"\s+", " ", t).strip()
    return t[:200]


def headers(args):
    qs = args.get("questions") if isinstance(args, dict) else None
    for q in qs if isinstance(qs, list) else []:
        if isinstance(q, dict) and q.get("header"):
            yield f"【{q['header']}】"


rows = collections.Counter()


def add(kind, text, origin):
    text = clean(text)
    if text:
        rows[(kind, text, origin)] += 1


def claude(path):
    with open(path, errors="replace") as f:
        for line in f:
            if not any(k in line for k in KEYS):
                continue
            try:
                v = json.loads(line)
            except ValueError:
                continue
            blocks = (v.get("message") or {}).get("content")
            if v.get("isApiErrorMessage"):
                add("error", text_of(blocks), "claude")
                continue
            for b in blocks if isinstance(blocks, list) else []:
                if not isinstance(b, dict):
                    continue
                if b.get("type") == "tool_use" and b.get("name") == "AskUserQuestion":
                    for h in headers(b.get("input")):
                        add("waiting", h, "claude")
                if b.get("type") == "tool_result" and b.get("is_error"):
                    t = text_of(b.get("content"))
                    if any(a in t for a in APPROVAL):
                        add("waiting", t, "claude")


def codex(path):
    with open(path, errors="replace") as f:
        for line in f:
            if not any(k in line for k in KEYS):
                continue
            try:
                p = json.loads(line).get("payload") or {}
            except ValueError:
                continue
            t = p.get("type")
            if t in ("function_call", "custom_tool_call") and str(p.get("name", "")).endswith("request_user_input"):
                try:
                    args = json.loads(p.get("arguments") or "{}")
                except ValueError:
                    args = {}
                for h in headers(args):
                    add("waiting", h, "codex")
            if t in ("function_call_output", "custom_tool_call_output"):
                out = p.get("output")
                out = out if isinstance(out, str) else json.dumps(out, ensure_ascii=False)
                # A long output that only mentions approval (a grep hit, a log) is not a
                # dialog; keeplined also needs a failing exit code, which outputs rarely carry.
                if len(out) < 500 and any(a in out for a in APPROVAL):
                    add("waiting", out, "codex")


def misses():
    path = os.path.join(config_dir(), "reasons-misses.jsonl")
    if not os.path.exists(path):
        return
    with open(path, errors="replace") as f:
        for line in f:
            try:
                r = json.loads(line)
                add(r["kind"], r["text"], "app")
            except (ValueError, KeyError):
                continue


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--misses", action="store_true", help="only the app's misses file")
    ap.add_argument("--check", metavar="CORPUS", help="print only rows not in this corpus")
    a = ap.parse_args()
    misses()
    if not a.misses:
        for p in glob.glob(f"{HOME}/.claude/projects/**/*.jsonl", recursive=True):
            claude(p)
        for p in glob.glob(f"{HOME}/.codex/sessions/**/*.jsonl", recursive=True):
            codex(p)
    known = set()
    if a.check:
        with open(a.check) as f:
            known = {(r["kind"], r["text"]) for r in map(json.loads, f) if r}
    merged = collections.Counter()
    origins = collections.defaultdict(set)
    for (kind, text, origin), n in rows.items():
        merged[(kind, text)] += n
        origins[(kind, text)].add(origin)
    for (kind, text), n in merged.most_common():
        if (kind, text) in known:
            continue
        print(json.dumps({"kind": kind, "text": text, "count": n,
                          "from": sorted(origins[(kind, text)])}, ensure_ascii=False))


main()
