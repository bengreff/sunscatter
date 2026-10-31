#!/usr/bin/env python3
"""Flies the playtest save (docs/technical/testing.md): the Falcon 9 Crew to orbit, debug off, then
Dragon alone in orbit (the second stage deorbited), saved as `playtest`. Through a running game's MCP
server (`./run headless [port]`); each phase stops time at its end.

    tools/playtest_flight.py <port> all            the whole flight (~10 min of real time: run it in
                                                   the background), then `playtest.ron` in the saves
    tools/playtest_flight.py <port> <phase> ...    one phase: new, pad, kick, maxq, meco, stage2,
                                                   insert [h_m pe_m], dragon, deorbit, descend, save
    tools/playtest_flight.py <port> check <name>   load a save (pause first) and fly on 150 s, then 1 h
                                                   at warp 6

Copy the save over docs/plans/playtest-2026-10-01-orbit.ron (`./run play` loads it with Cmd+L).
"""
import json, math, sys, time, urllib.request

GM = 3.986004418e14  # Earth (m³/s²)


def call(tool, args=None):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
                       "params": {"name": tool, "arguments": args or {}}}).encode()
    req = urllib.request.Request(f"http://127.0.0.1:{PORT}/mcp", data=body, headers={
        "Authorization": "Bearer dev", "Content-Type": "application/json",
        "Accept": "application/json, text/event-stream"})
    r = json.loads(urllib.request.urlopen(req, timeout=290).read())
    if "error" in r:
        raise RuntimeError(r["error"])
    t = r["result"]["content"][0]["text"]
    if r["result"].get("isError"):
        raise RuntimeError(f"{tool}: {t}")
    try:
        return json.loads(t)
    except ValueError:
        return t


def vessel(vid=None):
    return call("get_vessel", {"id": vid if vid is not None else call("get_state")["active_vessel"]})


def state_of(answer):
    """A `step` or `run_until` answer's sample (osculating apsides, heights above the ellipsoid)."""
    return answer.get("state") or {} if isinstance(answer, dict) else {}


def say(tag, answer=None):
    s = state_of(answer) if answer is not None else {}
    v = vessel()
    a = v.get("attitude") or {}
    alt = v.get("altitude_m") or 0.0
    print(f"{tag}: {alt / 1e3:.1f} km, {v.get('status')}, pitch {a.get('pitch_deg')}, "
          f"ap {s.get('apoapsis_m')}, pe {s.get('periapsis_m')}, fleet {s.get('fleet')}", flush=True)


def new():
    call("pause")
    call("new_flight", {"craft": "falcon9-crew"})
    call("pause")


def pad():
    """testing.md steps 1-3: ignition, then liftoff at full thrust."""
    call("pause")
    call("set_controls", {"throttle": 1})
    call("stage")
    call("step", {"seconds": 3})
    call("stage")
    say("liftoff +10 s", call("step", {"seconds": 10}))


def kick():
    """Steps 4-5: the pitch kick east, then the gravity turn on surface prograde."""
    ax = vessel()["attitude"]["axes_enu"]
    call("set_controls", {"sas": False, "rotate": [0.04 * ax[b][1] for b in ("x", "y", "z")]})
    call("step", {"seconds": 2.5})
    call("set_controls", {"rotate": [0, 0, 0]})
    call("step", {"seconds": 3})
    call("set_controls", {"sas": True, "hold": "stability"})
    call("step", {"seconds": 4})
    call("set_controls", {"hold": "prograde", "speed_reference": "surface"})
    say("gravity turn")


def maxq():
    say("max-Q", call("run_until", {"max_q": True, "warp": 2, "seconds": 90, "timeout_s": 240}))


def meco():
    """Step 7: 8 % kept in the booster's tanks."""
    say("feeding 8 %", call("run_until", {"feeding_below": 0.08, "warp": 2, "seconds": 120, "timeout_s": 240}))


def stage2():
    """Step 8: MECO, separation, the second stage's ignition; saved as `s2` to retry the insertion."""
    call("stage")
    call("step", {"seconds": 2})
    call("stage")
    call("run_until", {"separation": True, "seconds": 10, "timeout_s": 120})
    call("step", {"seconds": 3})
    call("stage")
    call("step", {"seconds": 6})
    call("set_controls", {"sas": True, "hold": "stability"})
    say("MVac +6 s")
    call("pause")
    call("save_game", {"name": "s2"})


def insert(h_target=200e3, pe_target=190e3, budget_s=270.0):
    """Step 9: steer the pitch so the vertical speed comes to zero at `h_target` (the deceleration that
    does it while far below, a damped approach near it), turning by 1 s rotate pulses about
    up × heading then SAS stability; the throttle closes when the periapsis passes `pe_target`.
    Returns whether it did (else call again: it continues)."""
    deadline, prev = time.time() + budget_s, None
    while time.time() < deadline:
        v = vessel()
        r, u = v["position_m"], v["velocity_ms"]
        rr = math.sqrt(sum(c * c for c in r))
        vv = sum(a * b for a, b in zip(r, u)) / rr
        vh = math.sqrt(max(sum(c * c for c in u) - vv * vv, 0.0))
        h = v["altitude_m"]
        att = v["attitude"]
        acc = 8.0  # the thrust's acceleration, from the last round's velocity change less gravity
        if prev is not None:
            pu, dt = prev
            acc = math.sqrt(sum(((b - c) / dt + GM * x / rr ** 3) ** 2 for b, c, x in zip(u, pu, r)))
        if h < h_target - 8e3 and vv > 0:
            want_av = -vv * vv / (2.0 * (h_target - h))
        else:
            want_av = -(h - h_target) / 40.0 ** 2 - 2.0 * vv / 40.0
        need = want_av + GM / rr ** 2 - vh * vh / rr
        err = math.degrees(math.asin(max(min(need / acc, 0.8), -0.4))) - att["pitch_deg"]
        step = 4.0
        if abs(err) > 1.0:
            hr = math.radians(att["heading_deg"])
            axis = (-math.cos(hr), math.sin(hr), 0.0)  # up × heading: positive tips the nose down
            k = (-1.0 if err > 0 else 1.0) * max(min(0.06 * abs(err), 0.3), 0.05)
            call("set_controls", {"sas": False, "rotate": [k * sum(p * q for p, q in zip(axis, att["axes_enu"][b]))
                                                           for b in ("x", "y", "z")]})
            call("step", {"seconds": 1})
            call("set_controls", {"rotate": [0, 0, 0], "sas": True, "hold": "stability"})
            step = 3.0
        s = state_of(call("step", {"seconds": step}))
        prev = (u, 1.0 + step if abs(err) > 1.0 else step)
        print(f"  {h / 1e3:.1f} km, vertical {vv:.0f} m/s, pitch {att['pitch_deg']}, pe {s.get('periapsis_m')}",
              flush=True)
        if (s.get("periapsis_m") or -1e9) > pe_target:
            call("set_controls", {"throttle": 0})
            return True
    return False


def dragon():
    """Step 10 and Dragon's separation: SECO, then Dragon (the active vessel) leaves the second stage."""
    call("set_controls", {"throttle": 0})
    call("stage")
    call("step", {"seconds": 5})
    call("stage")
    call("run_until", {"separation": True, "seconds": 10, "timeout_s": 120})
    say("Dragon +5 s", call("step", {"seconds": 5}))


def others():
    st = call("get_state")
    return [x["id"] for x in st["vessels"] if x["id"] != st["active_vessel"]]


def deorbit():
    """The second stage keeps its computer powered, so the debris rule keeps it (D083): −100 m/s at
    its next apoapsis puts its periapsis under the ground."""
    for vid in others():
        aps = [a for a in vessel(vid)["orbit"]["apsides"] if a["kind"] == "apoapsis"]
        plan = call("set_plan", {"id": vid, "burns": [{"in_s": aps[0]["in_s"], "prograde": -100.0}]})
        print(f"vessel {vid}: burn in {aps[0]['in_s']:.0f} s", json.dumps(plan)[:300], flush=True)
        call("run_until", {"seconds": max(aps[0]["in_s"] - 60.0, 1.0), "warp": 6, "timeout_s": 240})
        call("step", {"seconds": 60})
        call("step", {"seconds": 60})


def descend():
    """Runs on until the second stage has re-entered, broken up and gone (fleet 1)."""
    for _ in range(6):
        if not others():
            break
        call("run_until", {"seconds": 600, "warp": 6, "timeout_s": 240})
    say("alone", call("step", {"seconds": 1}))


def save(name="playtest"):
    call("pause")
    print(call("save_game", {"name": name}), flush=True)


def check(name):
    call("pause")
    print(call("load_game", {"name": name}), flush=True)
    call("step", {"seconds": 1})
    say("150 s on", call("step", {"seconds": 150}))
    say("1 h on at warp 6", call("run_until", {"seconds": 3600, "warp": 6, "timeout_s": 240}))


if __name__ == "__main__":
    PORT = int(sys.argv[1])
    phase, args = sys.argv[2], sys.argv[3:]
    if phase == "all":
        new(), pad(), kick(), maxq(), meco(), stage2()
        while not insert():
            pass
        dragon(), deorbit(), descend(), save()
    elif phase == "insert":
        print("cut off" if insert(*map(float, args)) else "not yet: call again")
    elif phase in ("check", "save"):
        globals()[phase](*args)
    else:
        globals()[phase]()
