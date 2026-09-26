# Windows runs on `backhouse`

The owner's Windows PC (`ssh backhouse`, Tailscale; OpenSSH with cmd.exe as the remote shell) is where the game is checked on Windows at the end of big sessions (process.md). CI builds and tests on Windows but has no GPU.

**Machine:** Windows 11 Pro, i7-14700K, 64 GB, RTX 4070 Ti SUPER. VS 2022 Community with the VC tools (rustc finds `link.exe`), git, and rustup (installed 2026-09-26, `%USERPROFILE%\.cargo`). The checkout is `C:\Users\Ben\sunscatter` (public repo, https).

All compiling happens on backhouse; run long commands with `run_in_background` and read logs.

## Update, test, build

```
ssh backhouse "cd /d %USERPROFILE%\sunscatter && git fetch -q && git reset --hard origin/main"
ssh backhouse "cd /d %USERPROFILE%\sunscatter && %USERPROFILE%\.cargo\bin\cargo test -p sim > %USERPROFILE%\sim_test.log 2>&1"
ssh backhouse "cd /d %USERPROFILE%\sunscatter && %USERPROFILE%\.cargo\bin\cargo build -p game --release > %USERPROFILE%\game_build.log 2>&1"
ssh backhouse 'powershell -NoProfile -Command "Get-Content C:\Users\Ben\game_build.log -Tail 20"'
```

A clean release build takes about 6 minutes.

## Demo for screenshots (plain SSH)

Plain SSH (session 0) renders fine offscreen (Vulkan on the RTX). Note `&&` with no space before it, so the values carry no trailing space:

```
ssh backhouse "rmdir /s /q C:\Users\Ben\sunscatter_demo 2>nul & set SUNSCATTER_DEMO=C:\Users\Ben\sunscatter_demo&& set SUNSCATTER_DEMO_OFFSCREEN=1&& set SUNSCATTER_DEMO_NO_BENCH=1&& set SUNSCATTER_HOME=C:\Users\Ben\sunscatter_demo\home&& cd /d C:\Users\Ben\sunscatter && target\release\game.exe > C:\Users\Ben\sunscatter_demo.log 2>&1"
scp "backhouse:sunscatter_demo/03_pad_medium.png" <local dir>/   # one file per scp call
ssh backhouse "type C:\Users\Ben\sunscatter_demo.log" | sed 's/\x1b\[[0-9;]*m//g' | grep "demo perf"
```

## Demo for performance numbers (interactive session)

Frame times over plain SSH are 3–4x worse (session 0). For perf or benchmark numbers run in the logged-in console session with a scheduled task (check `ssh backhouse "query user"` shows `ben / console / Active`). `C:\Users\Ben\run_demo.cmd` (CRLF line endings) sets the same variables, runs `target\release\game.exe >> C:\Users\Ben\sunscatter_demo.log 2>&1`, and appends `EXIT %ERRORLEVEL%`.

```
ssh backhouse "schtasks /create /tn SunscatterDemo /tr C:\Users\Ben\run_demo.cmd /sc once /st 00:00 /ru ben /it /f && schtasks /run /tn SunscatterDemo"
# poll the log until it contains EXIT, then:
ssh backhouse "schtasks /delete /tn SunscatterDemo /f"
```

## Results, 2026-09-26 (commit 2134bd8)

- `cargo test -p sim`: 72 tests pass, including the bit-exact golden tests (determinism macOS = Windows).
- The full demo runs; screenshots match macOS.
- Minimal tier, 11 vessels, interactive session: 359 fps at 1x, 366 at 1000x, 236 at 1,000,000x (worst frame 15.5 ms vs 8 ms on the M2 Pro: the one gap to watch).

The game finds `data/` through the compile-time `CARGO_MANIFEST_DIR`, so the exe runs only from the checkout that built it.

## Results, 2026-09-26 (commit fa08a52)

After the terrain overhaul and the night-side fix: `cargo test -p sim` passes (72 tests), the release build takes ~20 s incrementally, and the demo's pad, pad_top and earth_night views match macOS (textured ground, dark night side).
