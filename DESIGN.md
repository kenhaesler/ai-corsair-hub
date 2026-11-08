---
name: Corsair Hub
description: Technical luminous — a precise engineering instrument on graphite, lit by one state-coloured light.
colors:
  graphite: "#08090b"
  panel-top: "rgba(16, 18, 22, 0.9)"
  panel-bottom: "rgba(12, 13, 16, 0.9)"
  glass: "rgba(12, 13, 16, 0.8)"
  hairline: "rgba(255, 255, 255, 0.07)"
  hairline-strong: "rgba(255, 255, 255, 0.12)"
  text-primary: "#eef0f4"
  text-secondary: "#a7acb6"
  text-tertiary: "#7b808a"
  control: "#eceff4"
  control-ink: "#0b0c0f"
  state-light-cool: "#a9c1e6"
  heat-cold: "#7fb4ff"
  heat-mint: "#8fe0c8"
  heat-amber: "#ffc36b"
  heat-red: "#ff6a55"
  series-cpu: "#5aa9ff"
  series-gpu: "#34d399"
  series-psu-vrm: "#fbbf24"
  series-psu-case: "#fb7185"
  series-water: "#5ee1e6"
  series-power: "#ffd60a"
  ok: "#32d74b"
  warn: "#ff9f0a"
  error: "#ff453a"
typography:
  display:
    fontFamily: "Inter Tight Variable, Inter Tight, Segoe UI Variable Text, system-ui, sans-serif"
    fontSize: "50px"
    fontWeight: 300
    lineHeight: 1.04
    letterSpacing: "-0.035em"
  figure:
    fontFamily: "Inter Tight Variable, Inter Tight, system-ui, sans-serif"
    fontSize: "40px"
    fontWeight: 300
    lineHeight: 1
    letterSpacing: "-0.04em"
    fontFeature: "'cv11', 'ss01', 'tnum'"
  title:
    fontFamily: "Inter Tight Variable, Inter Tight, system-ui, sans-serif"
    fontSize: "22px"
    fontWeight: 600
    lineHeight: "28px"
    letterSpacing: "-0.017em"
  body:
    fontFamily: "Inter Tight Variable, Inter Tight, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: "20px"
  label:
    fontFamily: "Geist Mono Variable, Geist Mono, Cascadia Mono, ui-monospace, monospace"
    fontSize: "11px"
    fontWeight: 500
    lineHeight: "14px"
    letterSpacing: "0.16em"
rounded:
  chip: "6px"
  control: "10px"
  row: "14px"
  panel: "16px"
  sheet: "20px"
  pill: "999px"
spacing:
  xs: "4px"
  sm: "8px"
  md: "12px"
  lg: "16px"
  xl: "20px"
  xxl: "24px"
  section: "40px"
  gutter: "48px"
components:
  panel:
    backgroundColor: "{colors.panel-top}"
    rounded: "{rounded.panel}"
  panel-header:
    typography: "{typography.label}"
    textColor: "{colors.text-secondary}"
    height: "44px"
    padding: "0 18px"
  button-primary:
    backgroundColor: "{colors.control}"
    textColor: "{colors.control-ink}"
    rounded: "{rounded.control}"
    height: "32px"
    padding: "0 14px"
  button-secondary:
    backgroundColor: "rgba(255, 255, 255, 0.07)"
    textColor: "{colors.text-primary}"
    rounded: "{rounded.control}"
    height: "32px"
    padding: "0 14px"
  toolbar:
    backgroundColor: "{colors.glass}"
    rounded: "14px"
    height: "52px"
  chip:
    backgroundColor: "rgba(255, 255, 255, 0.05)"
    textColor: "{colors.text-secondary}"
    rounded: "{rounded.pill}"
    height: "26px"
    padding: "0 10px"
  tag:
    typography: "{typography.label}"
    rounded: "{rounded.chip}"
    padding: "4px 7px"
---

# Corsair Hub

## Overview

Corsair Hub is an instrument, not a dashboard. The direction is **technical luminous**: a precise
engineering aesthetic (hairline grids, square ports, mono micro-labels, light-weight figures) lit by a
single disciplined light source whose colour is the system's thermal state. It was approved by Ken on
2026-11-07 against `prototypes/spec-direction.html`, built from his own references (a technical-spec
tabs panel, an orthogonal security schematic, a star-field hero with falling light streaks).

Mode: **Operate**. Users glance at it on a second monitor, tune under load, and show it off. It must read
in a second and cost the machine nothing. The shipping window is 1440×900; the minimum is 1000×700.
There is one theme, dark, because the product lives beside games on a dark desk.

The one signature element is the **luminous bar over a ghost operating envelope**: a value glows in its own
heat colour, a lit white cap marks the reading, and a hairline box shows the range the control loop is
allowed to use. The one authored motion is **light that moves with the system**: ambient streaks fall at
fan-duty speed, pulses travel the control path at load speed, and the state light shifts hue as the rig
warms.

## Colors

- **Canvas** is graphite `#08090b`, never pure black. Panels are a near-black gradient with a repeating
  77px hairline column grid; hairlines are white at 7% (12% for panel edges).
- **Controls are white on graphite.** Primary buttons, toggles, sliders and the selected segmented pill
  are `#eceff4` with graphite ink. No accent colour is spent on chrome.
- **Colour means something.** It appears in exactly three places:
  1. **Data**: every live value glows in its own heat colour, from cold blue through mint and amber to red
     (`heatColor` in `apps/gui/ui/src/screens/dashboard/overview.ts`). Duty and power map onto the same ramp.
  2. **The state light** `--sc`: set on `:root` by `Ambient.svelte` from the thermal state (silver-blue →
     violet → orange, red in emergency). The nebula, tab underlines, headline emphasis, live markers and
     active preset tile all take their tint from it.
  3. **Series colours** (CPU blue, GPU green, PSU amber/rose, water cyan, power yellow) for chart lines
     and their legends only.
- Status greens/ambers/reds are reserved for live dots, banners and validation.
- Text: `#eef0f4` primary, `#a7acb6` secondary, `#7b808a` tertiary. Tertiary is for micro-labels and
  hints, never body copy. All body text meets 4.5:1 on the panel.

## Typography

- **Inter Tight** carries everything human: the state headline (50px, weight 300), page titles (40px,
  300), figures (64/40/30/28px, weight 300, tight tracking, tabular numerals) and body (14px).
- **Geist Mono**, uppercase, 10.5–11px, tracked 0.16em, is the instrument's engraving: panel headers,
  axis ticks, table heads, status tags, chain labels, RPM readouts. It is for measurement and labels,
  never for prose.
- Figures are light, not bold. Emphasis comes from size, glow and colour, never from weight above 600.
- Units sit beside figures in a smaller tertiary size (`45.1 °C`, `268 W`).

## Layout

- Frameless window: 32px titlebar, a **floating glass toolbar** (52px, inset 24px, 8px from the top)
  holding the tab strip and the page's actions, pages scrolling beneath it. Content fades out as it
  passes under the toolbar.
- One frame rule: operational pages (Summary, Fan Control, Devices, Lighting, Detail) run full bleed up to
  1440px; Settings is a 960px form. Gutters are 48px (32px under 1180px).
- Large titles get 32px above them; sections are 40px apart; panel interiors use an 18px inset.
- Summary is two panels side by side (readings 1.38fr, control path 1fr, 452px tall) over a five-cell
  figures row. Below 1180px the two panels stack at full width.
- Fan Control is a group strip (one panel of cells, selected cell underlined) over the editor panels for
  the selected group. Cells flex from 200px down to 148px; the mode label hides below 168px; "New group"
  stays pinned at the right edge when the strip scrolls.
- Every list uses 44px rows with inset hairline separators.

## Elevation & Depth

- Depth is tonal and lit, not shadowed. Panels carry a 1px hairline inset and a long soft drop
  (`0 24px 48px -32px rgb(0 0 0 / .9)`) that reads as the panel resting on the canvas.
- The toolbar and sheets are glass: `blur(16px) saturate(140%)` over 80–90% graphite, with a 1px hairline
  and a faint top highlight.
- Light is the depth cue: lit caps, glows under bars (`0 0 22px -4px` of the value colour), text-shadow
  behind figures, the state light's nebula behind everything.
- Focus is a two-ring white halo (`0 0 0 2px #08090b, 0 0 0 3px rgba(195,209,234,.7)`).

## Shapes

- Panels 16px, sheets 20px, rows 14px, controls 10px, chips and tags 6px (tags) or pill.
- Ports on the schematic are 7px squares; bars are 2–3px rectangles, never pills. Envelope boxes are
  1px hairline rectangles with 2px corners.
- Icons are Lucide at 14–16px, stroke 1.75–2.25, drawn in text colour inside 26px graphite tiles
  (`rgba(255,255,255,.05)` with a strong hairline) when they lead a row.

## Components

- **Panel** (`InsetGroup`): graphite gradient + column grid, 44px header row in mono micro-caps with an
  optional trailing action, rows separated by inset hairlines, optional footer caption.
- **Spec tabs** (`SpecPanel`): four icon + label tabs with a sliding lit underline (`--sc`, 1–2px,
  glow), a panel header naming the tab, rows of name / luminous bar / value, a mono tick axis, and an
  Edit mode for pin order and visibility.
- **Luminous bar**: 30px track at 2.4% white, ghost envelope box, gradient fill from 18% to 75% of the
  value colour with a white tip, 2px white lit cap, two drifting sparks. Transitions 1s ease-out.
- **Control path** (`ControlPath`): source pills → white control-loop hub → group pills on orthogonal
  hairline wires with square ports and travelling light pulses; each group pill shows duty and a mono
  status tag (On curve / Climbing / Easing / Holding / Fixed / Override).
- **Figures row**: five cells separated by hairlines; mono label, 40px light figure glowing in its value
  colour, one line of context. The last cell is the active fan preset with a jump button.
- **Group strip** (`GroupCapsules`): a leading **All fans** cell (average duty, combined envelope,
  "Mixed" or the shared mode) whose editor writes every change to all groups at once; then one cell per
  group with live dot + name (pencil to rename, F2), 30px light duty figure, mono mode label, 8px
  luminous bar over the group's envelope; sliding lit underline; sticky "New group".
- **Curve editor**: heat-zone plot with mono axes, group-coloured monotone curve, the sensor-mix reading
  driving the hysteresis band and operating point (CPU shown as a secondary marker when it differs),
  preset chips with curve thumbnails, numeric entry and undo.
- **Toolbar actions**: page-owned; Fan Control shows unsaved dot (words hidden under 1240px), plain Revert
  and a white primary Save; Lighting shows a master-controls pill that opens a popover.
- **Buttons**: primary white/graphite-ink, secondary raised 7% white with hairline, plain text,
  destructive red. 32px (md) / 26px (sm), radius 10px, press scales to .97.
- **Toggle**: white track with graphite knob when on; `Segmented`: sliding white pill on a spring.
- **Sheet**: centred glass dialog, spring from .96, focus trapped and handed back on close; Escape closes
  one level.
- **Status tag**: mono 10px uppercase in a 6px tinted box (light blue by default, red for alerts).

## Do's and Don'ts

- Do give every live value its own heat colour; do keep chrome white on graphite.
- Do show each datum once per screen; disclose detail on tap (Summary rows open Detail pages).
- Do use horizontal bars for temperatures and duty. Never arc or radial gauges.
- Do derive only what hardware reports (types, LED counts, chain order); names and placements are user
  settings with neutral defaults ("Hub 7A11 Ch 1", "Strip 1 · 40 LEDs").
- Do honour `prefers-reduced-motion`: sparks, streaks and pulses stop, springs settle instantly.
- Don't add an eyebrow label above a heading; mono micro-caps belong to panel headers and measurements.
- Don't tint secondary text grey on coloured surfaces; tint it from the surface hue.
- Don't introduce curvy flow lines, atmospheric gradient backgrounds, cheap 3D or themed metaphors
  (audio consoles, blueprints, retro phosphor). The schematic is orthogonal; the light is one source.
- Don't colour controls, icon tiles or sliders with series colours; colour is for data.
- Don't let any surface break at 1000×700: stack panels, shrink label columns, pin the primary action.
