#!/usr/bin/env python3
"""Validate a native ice lab self-test log without reading screen pixels."""

import argparse
import json
import statistics
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("log", type=Path)
    parser.add_argument("--source", choices=["synthetic", "live"], default="synthetic")
    args = parser.parse_args()
    rows = [json.loads(line) for line in args.log.read_text().splitlines() if line.strip()]
    failures = []

    def check(condition, message):
        print(f"{'PASS' if condition else 'FAIL'} {message}")
        if not condition:
            failures.append(message)

    check(bool(rows), "telemetry exists")
    if not rows:
        raise SystemExit(1)
    events = {row["event"] for row in rows}
    expected = {"initialized", "running", "dissolve", "stopped"}
    if args.source == "synthetic":
        expected |= {"scripted-contact", "self-test-complete"}
    check(expected <= events, "run includes baseline, render, dissolve and stop")
    check(not ({"error", "stop-error", "gpu-drain-timeout"} & events)
          and all(row["gpuErrors"] == 0 for row in rows), "no reported capture or GPU errors")
    running = [row for row in rows if row["state"] == "running" and row["event"] == "sample"]
    check(len(running) >= 5, "at least five active sampling intervals")
    check(all(row["source"] == args.source for row in running), "source is labelled consistently")
    if args.source == "synthetic":
        check(all(row["captureFrames"] == 0 and not row["captureActive"] for row in running),
              "synthetic run does not claim captured frames")
    else:
        check(bool(running) and running[-1]["captureFrames"] > running[0]["captureFrames"]
              and all(row["captureActive"] for row in running), "live capture delivers frames during rendering")
    check(any(row.get("gpuP50Ms", 0) > 0 for row in running), "GPU execution actually measured")
    check(any(row.get("inputToSubmitMs") is not None for row in rows), "contact reaches renderer submission")
    check(rows[-1]["renderedFrames"] >= 100, "at least 100 render submissions")
    check(rows[-1].get("completedFrames", 0) == rows[-1]["renderedFrames"], "all submitted GPU frames complete")
    stopped_index = next((i for i, row in enumerate(rows) if row["event"] == "stopped"), len(rows))
    after = rows[stopped_index:]
    idle = [row for row in after if row["event"] == "sample"]
    check(len(idle) >= 3, "at least three post-stop idle samples")
    check(bool(after) and all(row["state"] == "idle" and not row["captureActive"] and not row["renderActive"]
                             and row["texturesHeld"] == 0 and row["inFlight"] == 0 for row in after),
          "stopped samples have no session textures, capture, render timer or GPU work")
    check(bool(after) and len({(row["renderedFrames"], row["captureFrames"]) for row in after}) == 1,
          "frame counters stop advancing after stop")
    check(all(row["residentMB"] > 0 and row["cpuPercent"] >= 0 for row in rows), "process measurements available")
    baseline = [row for row in rows[:next((i for i, r in enumerate(rows) if r["event"] == "starting"), 0)]
                if row["event"] == "sample"]
    for label, group in [("baseline", baseline), (f"{args.source} active", running), ("post-stop idle", idle)]:
        if group:
            report = {"phase": label, "samples": len(group)}
            for key in ["cpuPercent", "residentMB", "frameP95Ms", "gpuP50Ms"]:
                values = [r[key] for r in group if r.get(key) is not None]
                if values:
                    report[key] = {"median": round(statistics.median(values), 3), "min": round(min(values), 3), "max": round(max(values), 3)}
            print(json.dumps(report))
    print(f"{args.source.capitalize()} lifecycle verification only. Visual quality, display latency and multi-display performance need separate validation.")
    raise SystemExit(bool(failures))


if __name__ == "__main__":
    main()
