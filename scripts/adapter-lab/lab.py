#!/usr/bin/env python3
"""Helpers of the adapter lab (run-lab.sh). Standard library only; run as python3 -I.

  lab.py payload FIXTURE CWD [key=value ...]   a hook payload from a captured fixture, re-pointed at CWD
  lab.py verdict STATUS_JSON CRITERION         one line, | separated: state, holds, cards, last run outcome
  lab.py adapter PROJECT_DIR STATUS_JSON       one line, | separated: the adapter's choice, the gaps
  lab.py table RESULT_TSV ...                  the summary table, then its totals
"""
import json
import os
import sys

# The columns of a project's result row (one line of tab separated fields, in this order).
FIELDS = ["project", "reader", "reader_expected", "gaps", "red_expected", "red_observed", "red_verdict",
          "green_expected", "green_observed", "green_verdict", "cards", "note"]


def payload(fixture, cwd, *pairs):
    doc = json.load(open(fixture))
    doc["cwd"] = cwd
    for pair in pairs:
        key, value = pair.split("=", 1)
        if key.startswith("@"):
            doc[key[1:]] = value
        else:
            doc.setdefault("tool_input", {})[key] = value
    print(json.dumps(doc))


def verdict(status_path, criterion):
    try:
        doc = json.load(open(status_path))
    except (OSError, ValueError):
        print("unreadable|||")
        return
    crit = next((c for c in doc.get("criteria", []) if c.get("id") == criterion), {})
    cards = [c.get("hold") or c.get("question") or "?" for c in doc.get("cards", [])]
    runs = [r for r in doc.get("runs", []) if r.get("criterion_id") == criterion]
    last = runs[-1].get("outcome", "") if runs else ""
    in_flight = any(r.get("in_flight") for r in doc.get("runs", []))
    state = crit.get("state_name") or "?"
    if in_flight:
        state = "running"
    print("|".join([state, ",".join(crit.get("holds") or []), ",".join(cards), last]))


def adapter(project_dir, status_path):
    """The adapter's choice per runner: .familiar/adapter (the committed file) when the binary writes
    one, else anything the status JSON shows under a key naming a reader or an adapter, else none."""
    path = os.path.join(project_dir, ".familiar", "adapter")
    if os.path.isfile(path):
        readers, gaps = [], []
        for line in open(path).read().splitlines():
            try:
                row = json.loads(line)
            except ValueError:
                readers.append("(unreadable line)")
                continue
            tag = row.get("runner", "?") + ("@" + row["root"] if row.get("root") else "")
            readers.append("%s=%s" % (tag, row.get("reader", "?")))
            gaps += ["%s:%s" % (tag, g if isinstance(g, str) else json.dumps(g, sort_keys=True)) for g in row.get("gaps", [])]
        print("%s|%s" % (";".join(readers) or "-", ";".join(gaps) or "-"))
        return
    try:
        doc = json.load(open(status_path))
    except (OSError, ValueError):
        doc = {}
    found = {}

    def walk(node, trail):
        if isinstance(node, dict):
            for key, value in node.items():
                if any(w in key.lower() for w in ("reader", "adapter")) and value not in (None, "", [], {}):
                    found[".".join(trail + [key])] = value
                else:
                    walk(value, trail + [key])
        elif isinstance(node, list):
            for i, value in enumerate(node):
                walk(value, trail + [str(i)])

    walk(doc, [])
    if found:
        print("%s|-" % json.dumps(found, sort_keys=True))
    else:
        print("-|-")


def kind_of(row):
    """skipped, WRONG (a false seal or green), ok, or DIFF (the stages' other differences)."""
    verdicts = [v for v in (row["red_verdict"], row["green_verdict"]) if v not in ("", "-")]
    if "skipped" in verdicts:
        return "skipped"
    if "WRONG" in verdicts:
        return "WRONG"
    if verdicts and all(v == "ok" for v in verdicts):
        return "ok"
    return "DIFF"


def table(paths):
    rows = []
    for p in paths:
        for line in open(p).read().splitlines():
            if line.strip():
                rows.append(dict(zip(FIELDS, (line.split("\t") + [""] * len(FIELDS))[:len(FIELDS)])))
    if not rows:
        print("no project ran")
        return
    head = ["project", "adapter (gaps)", "red: want / got", "green: want / got", "cards", "verdict"]
    body = []
    for r in rows:
        if r["red_verdict"] == "skipped":
            body.append([r["project"], "-", "-", "-", "-", "skipped: " + r["note"]])
            continue
        reader = r["reader"]
        if reader in ("-", ""):
            reader = "tables" + (" (want %s)" % r["reader_expected"] if r["reader_expected"] else "")
        elif r["reader_expected"] and r["reader_expected"] not in reader:
            reader += " (want %s)" % r["reader_expected"]
        if r["gaps"] not in ("-", ""):
            reader += " gaps: " + r["gaps"]
        red = "%s / %s" % (r["red_expected"], r["red_observed"])
        green = "%s / %s" % (r["green_expected"], r["green_observed"]) if r["green_expected"] not in ("-", "") else "-"
        verdicts = [v for v in (r["red_verdict"], r["green_verdict"]) if v not in ("", "-")]
        cell = "ok" if kind_of(r) == "ok" else " ".join(v for v in verdicts if v != "ok") or "DIFF"
        if kind_of(r) != "ok" and r["red_verdict"] != "ok" and r["red_verdict"] not in ("", "-"):
            cell = "red " + r["red_verdict"] + (" green " + r["green_verdict"] if r["green_verdict"] not in ("", "-", "ok") else "")
        elif kind_of(r) != "ok" and r["green_verdict"] not in ("", "-", "ok"):
            cell = "green " + r["green_verdict"]
        if r["note"]:
            cell += "  [%s]" % r["note"]
        body.append([r["project"], reader, red, green, r["cards"] or "-", cell])
    widths = [max(len(str(row[i])) for row in [head] + body) for i in range(len(head) - 1)]
    out = []
    for row in [head] + body:
        out.append("  ".join(str(c).ljust(widths[i]) for i, c in enumerate(row[:-1])) + "  " + row[-1])
    print("\n".join(out))
    kinds = [kind_of(r) for r in rows]
    print()
    print("%d projects: %d as expected, %d differ, %d WRONG (a seal or a green that is not true), %d skipped"
          % (len(rows), kinds.count("ok"), kinds.count("DIFF"), kinds.count("WRONG"), kinds.count("skipped")))
    print("verdicts: ok · WRONG a seal or green that is not true · FALSE-ALARM a true red or green refused ·"
          " CARD a card where none was expected (marked ! in the cards column) · DIFF anything else")


def main(argv):
    if len(argv) < 2:
        print(__doc__)
        return 2
    cmd, args = argv[1], argv[2:]
    if cmd == "payload":
        payload(*args)
    elif cmd == "verdict":
        verdict(*args)
    elif cmd == "adapter":
        adapter(*args)
    elif cmd == "table":
        table(args)
    else:
        print(__doc__)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
