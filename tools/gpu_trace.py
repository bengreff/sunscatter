#!/usr/bin/env python3
"""GPU time per render pass on Metal, from an Instruments Metal System Trace.

The game's profiler cannot time GPU passes on Metal (`game::profile`), and
the benchmark's frame times move with the CPU load of whatever else runs.
This reads a trace instead: Bevy encodes each pass in its own command
buffer and they complete in order on one queue, so a command buffer's GPU
time is its completion minus the later of its commit and the previous
completion (exact while the GPU is busy, i.e. when it is the limit).

Record (it runs the game's release binary, `cargo build -p game --release`,
headless as `./run headless` does; one game at a time on the shared
machine, under `target/demo.lock`):

    xcrun xctrace record --template 'Metal System Trace' --no-prompt \\
        --time-limit 90s --output DIR/view.trace --env SUNSCATTER_HOME=DIR \\
        --env SUNSCATTER_OFFSCREEN=1 --env SUNSCATTER_DEV=1 \\
        --env SUNSCATTER_AGENT=7777:dev \\
        --env SUNSCATTER_RENDER_SCALE=1 --launch -- target/release/game

(each --env and its value are two arguments; use your own CARGO_TARGET_DIR's
binary), and set the view meanwhile through MCP (`tools/mcp.sh`: `shoot`
with a width and height, `set_camera`, `load_game`); the recording and
the game stop at the time limit (or Ctrl+C). Then, with DIR holding the
one trace:

    tools/gpu_trace.py DIR              # per second: GPU busy, frames, main pass
    tools/gpu_trace.py DIR 11 18        # and the passes between 11 and 18 s

The `bench` tool (the developer flag) times each of its rows while it
records: 4 s each (`bench::WARMUP` + `MEASURE`), the whole look first, then
the look without each part in `look::Parts::each_off` order; `profile`
gives the CPU side of the same seconds.
The exported tables are kept next to the trace (DIR/t_*.xml); delete the
trace when done, it is hundreds of MB.
"""
import subprocess
import sys
import xml.etree.ElementTree as ET
from collections import defaultdict
from pathlib import Path

TABLES = [
    "metal-application-encoders-list",
    "metal-application-command-buffer-submissions",
    "metal-command-buffer-completed",
]
MAIN_PASS = "main_opaque_pass_3d"


def trace_in(names: list[str]) -> str:
    """The trace among a directory's entries: the one `*.trace` there."""
    traces = sorted(n for n in names if n.endswith(".trace"))
    if len(traces) != 1:
        sys.exit(f"expected one *.trace in the directory, found {traces or 'none'}")
    return traces[0]


def export(d: Path) -> None:
    trace = d / trace_in([p.name for p in d.iterdir()])
    for t in TABLES:
        out = d / f"t_{t}.xml"
        if out.exists() and out.stat().st_size > 0:
            continue
        xpath = f'/trace-toc/run[@number="1"]/data/table[@schema="{t}"]'
        with open(out, "wb") as f:
            subprocess.run(["xcrun", "xctrace", "export", "--input", str(trace), "--xpath", xpath], stdout=f, check=True)


def rows(path: Path):
    """The table's rows as {column: (formatted, raw)}, resolving id/ref."""
    ids = {}
    for node in ET.parse(path).getroot().iter("node"):
        cols = [c.findtext("mnemonic") for c in node.find("schema").findall("col")]
        for row in node.findall("row"):
            out = {}
            for c, e in zip(cols, list(row)):
                ref = e.get("ref")
                if ref is not None:
                    out[c] = ids.get(ref)
                    continue
                v = (e.get("fmt"), e.text)
                if e.get("id") is not None:
                    ids[e.get("id")] = v
                out[c] = v
            yield out


def main() -> None:
    d = Path(sys.argv[1])
    t0 = float(sys.argv[2]) if len(sys.argv) > 2 else 0.0
    t1 = float(sys.argv[3]) if len(sys.argv) > 3 else 1e9
    export(d)
    labels = defaultdict(list)
    for r in rows(d / f"t_{TABLES[0]}.xml"):
        cb, lab = r.get("cmdbuffer-id"), r.get("encoder-label")
        if cb and lab:
            labels[cb[1]].append(lab[0])
    commit = {r["cmdbuffer-id"][1]: int(r["start"][1]) for r in rows(d / f"t_{TABLES[1]}.xml") if r.get("cmdbuffer-id")}
    done = sorted((int(r["timestamp"][1]), r["cmdbuffer-id"][1]) for r in rows(d / f"t_{TABLES[2]}.xml"))
    busy, frames, main = defaultdict(float), defaultdict(int), defaultdict(float)
    per_pass = defaultdict(lambda: [0.0, 0])
    prev, nframes = None, 0
    for t, cb in done:
        begin = commit.get(cb, t) if prev is None else max(commit.get(cb, t), prev)
        gpu = max(t - begin, 0) / 1e6
        prev = t
        sec = int(t / 1e9)
        busy[sec] += gpu
        names = labels.get(cb, [])
        if MAIN_PASS in names:
            frames[sec] += 1
            main[sec] += gpu
        if t0 <= t / 1e9 < t1:
            per_pass[" + ".join(dict.fromkeys(names or ["(unlabelled)"]))[:100]][0] += gpu
            nframes += MAIN_PASS in names
    print("  s   GPU busy ms   frames   main pass ms/frame")
    for s in sorted(busy):
        if frames[s]:
            print(f"{s:4d}   {busy[s]:8.1f}   {frames[s]:6d}   {main[s] / frames[s]:8.2f}")
    if len(sys.argv) > 2 and nframes:
        print(f"\n{t0}-{t1} s, {nframes} frames, GPU ms per frame:")
        for name, (ms, _) in sorted(per_pass.items(), key=lambda kv: -kv[1][0])[:20]:
            print(f"  {ms / nframes:7.3f}  {name}")
        print(f"  {sum(v[0] for v in per_pass.values()) / nframes:7.3f}  total")


if __name__ == "__main__":
    main()
