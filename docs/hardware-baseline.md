# Hardware baseline procedure (PR 0, G05)

Operator procedure for recording the physical baseline before any production
fix is released. **Status: the physical check is pending until the operator
performs it.** Nothing in this repository has verified LED placement on real
hardware; the automated tests only prove the bytes the worker hands to the hub.

Baseline evidence is machine-local and is **not** committed (it contains
device serials, a config copy and photos).

## Procedure

1. **Build or install the baseline app.** Use the revision that is the
   baseline for the fix train (for example `cargo build -p corsair-hub-gui --release`
   from the baseline commit, or the installed release). Keep this executable
   and the matching config; they are the rollback target.
2. **Run only one hardware-controlling application.** Close iCUE. Do not run
   `corsair-scanner` or `corsair-rgb-test` while the GUI is running. Start the
   baseline app and make sure RGB is **enabled with hardware output** (the
   diagnostic refuses otherwise, because it could not restore prior state).
3. **Run the LED marker diagnostic.** Devices -> **LED markers** (60 s; the
   buttons are disabled while it runs). Every LED slot on both hubs, including
   devices that no lighting zone references, shows:
   - **white** = first LED of the slot,
   - **green** = second LED (direction; slots with 3 or more LEDs),
   - **red** = last LED of the slot,
   - every other LED = that device's palette colour (see the table in the UI).

   Colours are scaled by port power protection exactly like normal rendering
   (the UI shows the factor). With roughly 400+ LEDs the factor is 0.33 or 0.10,
   so dim markers are expected, not a fault.

   Verify per device, using the on-screen table (channel, type, LEDs, count
   source, LED range):
   - white first LED, green second LED, red last LED;
   - **both ends** of every strip and adapter chain light as expected
     (a dark tail or missing end means the effective count is wrong);
   - **neighbouring devices are not shifted** (the red end of one device must
     not appear on the next device's first LED);
   - both hubs, including channel 15 (documented 80-position adapter chain);
   - devices listed as "no slot (skipped)" really have no LEDs.

   Photograph each hub's devices while the markers are visible.

   **Extra modes** (mode selector next to the button; same 60 s countdown, all
   restore prior state when done). Use them when the default markers leave a
   boundary in doubt:
   - **Markers (black body)**: same white/green/red markers, but every other
     LED is off, so a stray lit LED beyond a slot end is easy to spot.
   - **Ruler**: count decade marks along the device. **White** = LED 0,
     **blue** = every 5th LED, **yellow** = every 10th LED, **red** = last LED
     of the slot. Counting the marks to the red LED shows where the slot really
     ends; compare with the LED range in the UI table.
   - **Walk** (pick a device and a step time, 100-2000 ms, default 400): a
     single **white** LED steps along the slot and the **red** last LED stays
     lit. The UI shows "LED i / n" (computed locally, no polling); compare it
     with the LED you see lit.

   Procedure for hub 22DE channel 15 (80 positions): run **Ruler**; the 8th
   yellow mark is LED 70 and the red LED must be LED 79, with nothing lit past
   it and nothing shifted onto the next slot. Then run **Walk** on that device
   and confirm the stepping LED matches "LED i / n" at the start, middle and
   end. For one QX fan (34 LEDs): **Ruler** shows yellow at LED 10, 20, 30, blue
   at 5, 15, 25, and red at LED 33; **Walk** should step round the fan once per
   34 steps with the red LED fixed at the end.
4. **Record the baseline.** From the repository root in Windows PowerShell:
   `.\scripts\record-baseline.ps1` (add `-ExePath <exe>` if the executable is
   not auto-detected). It writes `%LOCALAPPDATA%\corsair-hub\baselines\<timestamp>\`
   with `baseline.json` (git commit and dirty state, executable path/size/version/SHA-256,
   config SHA-256, Windows build, `rustc -V`), a copy of `config.toml` and the
   newest `led-manifest-*.json` from `%APPDATA%\corsair-hub\diagnostics\`.
   It does not touch USB devices or start the app. If it warns that no manifest
   exists, run step 3 first.
5. **Store the photos** in that same baseline folder.
6. **Fill in the checklist** below (copy this table into the baseline folder
   and mark each row). The rows mirror the "Hardware regression and rollback
   gate" in `docs/fix-implementation-plan.md`.

For a candidate build, repeat steps 2-4 with the same config and topology and
compare `led-manifest-*.json` and the photos against the baseline. The
manifest is deterministic for a given topology and config.

## Checklist

| Scenario | Required observation | Pass | Fail | Pending |
|---|---|---|---|---|
| Baseline and candidate at same config/topology | Same device IDs, effective counts, first/last LED markers, full strip coverage, neighbour boundaries, brightness and colours | | | X |
| RGB on/off and verified FPS settings | Fan cadence/ramp stays correct; no new flicker, shifts or USB error pattern | | | X |
| Fan edit, rename/name-clear, preset and lighting save | No addressing change, overrides preserved, settings survive restart | | | X |
| Quit/relaunch | Restoration completes or reports bounded failure; addressing correct after init | | | X |
| Hub unavailable at startup / controlled reconnect / resume | Available cooling continues, reappearing device resolves by ID, mapping/counts/layouts reconcile | | | X |
| Sensor unavailable/stale | Defined fallback/emergency latch; no unrelated pump slowdown; RGB worker responsive | | | X |
| Mixed operation soak (30+ minutes) | Frame/control timing, I/O failures, topology events and config revision recorded | | | X |

Stop/revert criteria are in the plan: unexpected channel offset, dark strip
tail, colour on the wrong hub/device, changed effective count without a
deliberate change, new sustained flicker, or a missed cooling schedule.
Do not unplug a running pump to simulate USB loss.
