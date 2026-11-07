<!--
  Live preview stage (inventory 4.5). The selected zone's devices drawn as
  light on deep black glass, from the live rgb-frame stream:
  - fans (and unknown types): a diffused LED ring (conic gradient through
    every LED), 34 dark-grey LEDs when no frame has arrived; label centred
  - devices with strips[]: one strip per segment, sliced by led_start/count,
    labelled "<name> · Strip N (count LEDs)"
  - anything else: one linear strip
  Glow is a small bloom pyramid (1/4 → 1/32 resolution, halved and then
  upsampled back, added with 'lighter'), so its cost barely grows with
  the stage size. Colours
  ease toward each 16 Hz frame at 60 fps. The loop only runs while the
  page is the active tab, the window is visible and the stage is on screen,
  and it idles once the light has settled.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import Lightbulb from 'lucide-svelte/icons/lightbulb';
  import Slider from '../../components/ui/Slider.svelte';
  import { rgb } from '../../lib/stores/rgb.svelte';
  import { configStore } from '../../lib/stores/config.svelte';
  import { sensors } from '../../lib/stores/sensors.svelte';
  import { reducedMotion } from '../../lib/motion.svelte';
  import { displayName, displayNameFromTree, isDeviceRefV2 } from '../../lib/identity';
  import type { HubSnapshot, RgbDeviceRef, RgbZoneConfig } from '../../lib/types';
  import { frameFor, ledCountOf, refKey, resolveRef } from './effects';
  import { lighting } from './state.svelte';

  interface Props {
    zone: RgbZoneConfig | null;
    hubs: HubSnapshot[];
    /** page is the active tab */
    active: boolean;
    loaded: boolean;
    onzonebrightness: (v: number, commit: boolean) => void;
  }
  let { zone, hubs, active, loaded, onzonebrightness }: Props = $props();

  // ---------------------------------------------------------------------------
  // Items and layout
  // ---------------------------------------------------------------------------
  interface Item {
    key: string;
    kind: 'fan' | 'strip';
    ref: RgbDeviceRef;
    label: string;
    /** strip slice into the device frame, or null for the whole frame */
    slice: { start: number; count: number } | null;
    /** expected LED count when no frame is present */
    expect: number;
    // layout (CSS px, relative to the canvas)
    x: number;
    y: number;
    w: number;
    h: number;
  }

  function labelFor(ref: RgbDeviceRef): string {
    if (!isDeviceRefV2(ref) || !ref.device_id) return `Ch ${ref.channel}`;
    const n = displayName(ref.device_id, { config: configStore.config, snapshot: sensors.snapshot });
    // displayName only knows fans for location; strips/adapters resolve via the tree.
    return n.startsWith('Device ') ? displayNameFromTree(ref.device_id, { config: configStore.config, hubs }) : n;
  }

  const baseItems = $derived.by((): Item[] => {
    if (!zone) return [];
    const out: Item[] = [];
    zone.devices.forEach((ref, idx) => {
      const { hub, dev } = resolveRef(ref, hubs);
      const label = labelFor(ref);
      const k = `${refKey(ref)}#${idx}`;
      const isFan = !dev || /fan/i.test(dev.device_type);
      const blank = { x: 0, y: 0, w: 0, h: 0 };
      if (isFan) {
        out.push({ key: k, kind: 'fan', ref, label, slice: null, expect: 34, ...blank });
      } else if (dev?.strips && dev.strips.length > 0) {
        for (const seg of dev.strips) {
          out.push({
            key: `${k}:s${seg.index}`,
            kind: 'strip',
            ref,
            label: `${label} · Strip ${seg.index + 1} (${seg.led_count} LEDs)`,
            slice: { start: seg.led_start, count: seg.led_count },
            expect: seg.led_count,
            ...blank,
          });
        }
      } else {
        const n = dev && hub ? ledCountOf(hub.serial, dev, rgb.frames) : null;
        out.push({
          key: k,
          kind: 'strip',
          ref,
          label: n === 0 ? `${label} · no LEDs reported` : label,
          slice: null,
          expect: n ?? 0,
          ...blank,
        });
      }
    });
    return out;
  });

  let width = $state(0);
  const PAD_X = 32;
  /** empty margin inside the canvas so bloom never meets a canvas edge */
  const GP = 44;
  const FAN_GAP = 24;
  const BLOCK_GAP = 36;
  const LABEL_H = 26; // strip label below the bar

  const layout = $derived.by(() => {
    const W = Math.max(0, width - PAD_X * 2);
    const items = baseItems.map((i) => ({ ...i }));
    const fans = items.filter((i) => i.kind === 'fan');
    const strips = items.filter((i) => i.kind === 'strip');
    let y = GP;
    if (fans.length > 0 && W > 0) {
      let rows = 1;
      let size = 0;
      for (rows = 1; rows <= 4; rows++) {
        const n = Math.ceil(fans.length / rows);
        size = Math.min(112, (W - FAN_GAP * (n - 1)) / n);
        if (size >= 96 || rows === 4) break;
      }
      size = Math.floor(size);
      const perRow = Math.ceil(fans.length / rows);
      for (let r = 0; r < rows; r++) {
        const row = fans.slice(r * perRow, (r + 1) * perRow);
        const rowW = row.length * size + (row.length - 1) * FAN_GAP;
        let x = PAD_X + (W - rowW) / 2;
        for (const f of row) {
          Object.assign(f, { x, y, w: size, h: size });
          x += size + FAN_GAP;
        }
        y += size + (r < rows - 1 ? FAN_GAP : 0);
      }
    }
    if (strips.length > 0 && W > 0) {
      const solo = fans.length === 0;
      const sh = solo ? 18 : 14;
      const rowH = sh + LABEL_H + (solo ? 18 : 10);
      if (!solo) y += BLOCK_GAP;
      const cols = !solo && strips.length > 1 && W >= 2 * 360 + 32 ? 2 : 1;
      const colW = cols === 2 ? (W - 32) / 2 : Math.min(W, solo ? 820 : 720);
      const total = cols * colW + (cols - 1) * 32;
      const x0 = PAD_X + (W - total) / 2;
      strips.forEach((st, i) => {
        const c = i % cols;
        const r = Math.floor(i / cols);
        Object.assign(st, { x: x0 + c * (colW + 32), y: y + r * rowH, w: colW, h: sh });
      });
      const rowsN = Math.ceil(strips.length / cols);
      y += (rowsN - 1) * rowH + sh + LABEL_H;
    }
    return { items, height: Math.max(0, y - GP) };
  });

  const canvasH = $derived(Math.max(96, layout.height) + GP * 2);

  // ---------------------------------------------------------------------------
  // Rendering
  // ---------------------------------------------------------------------------
  let host: HTMLDivElement;
  let canvas: HTMLCanvasElement;
  let onScreen = $state(true);
  const DARK = 30; // "no frame" LED grey
  /** current eased colours per item key (0..255 floats) */
  const cur = new Map<string, Float32Array>();
  let lastFrames: unknown = null;
  let settled = false;
  let glow = $state('rgba(124,156,255,0)');

  function targetFor(it: Item, out: Float32Array) {
    const f = frameFor(it.ref, rgb.frames);
    const leds = f?.leds ?? null;
    const n = out.length / 3;
    for (let i = 0; i < n; i++) {
      const src = leds ? leds[(it.slice?.start ?? 0) + i] : null;
      out[i * 3] = src ? src[0] : DARK;
      out[i * 3 + 1] = src ? src[1] : DARK;
      out[i * 3 + 2] = src ? src[2] : DARK;
    }
  }

  function ledCount(it: Item): number {
    if (it.slice) return it.slice.count;
    const f = frameFor(it.ref, rgb.frames);
    return f ? f.leds.length : it.expect;
  }

  const tgt = new Map<string, Float32Array>();

  function step(dt: number): boolean {
    const k = reducedMotion.current ? 1 : 1 - Math.exp(-dt * 14);
    let moving = false;
    for (const it of layout.items) {
      const n = ledCount(it);
      let c = cur.get(it.key);
      let t = tgt.get(it.key);
      if (!c || c.length !== n * 3) {
        c = new Float32Array(n * 3).fill(DARK);
        cur.set(it.key, c);
        moving = true;
      }
      if (!t || t.length !== n * 3) {
        t = new Float32Array(n * 3);
        tgt.set(it.key, t);
      }
      targetFor(it, t);
      for (let i = 0; i < c.length; i++) {
        const d = t[i] - c[i];
        if (d > 0.4 || d < -0.4) {
          c[i] += d * k;
          moving = true;
        } else c[i] = t[i];
      }
    }
    return moving;
  }

  const css = (c: Float32Array, i: number, a = 1) =>
    `rgba(${c[i * 3] | 0},${c[i * 3 + 1] | 0},${c[i * 3 + 2] | 0},${a})`;

  /** Light emitters only (rings, strip bars). Used at full res and for the bloom passes. */
  function drawEmitters(ctx: CanvasRenderingContext2D, s: number, core: boolean) {
    for (const it of layout.items) {
      const c = cur.get(it.key);
      if (!c || c.length === 0) continue;
      const n = c.length / 3;
      if (it.kind === 'fan') {
        const cx = (it.x + it.w / 2) * s;
        const cy = (it.y + it.h / 2) * s;
        const ringR = it.w * 0.4 * s;
        const g = ctx.createConicGradient(-Math.PI / 2, cx, cy);
        for (let i = 0; i < n; i++) g.addColorStop(i / n, css(c, i));
        g.addColorStop(1, css(c, 0));
        ctx.strokeStyle = g;
        ctx.lineWidth = it.w * (core ? 0.016 : 0.06) * s;
        ctx.beginPath();
        ctx.arc(cx, cy, ringR, 0, Math.PI * 2);
        ctx.stroke();
      } else {
        const x = it.x * s;
        const y = (it.y + it.h / 2) * s;
        const w = it.w * s;
        if (n === 0) continue;
        const g = ctx.createLinearGradient(x, 0, x + w, 0);
        for (let i = 0; i < n; i++) g.addColorStop(n === 1 ? 0.5 : i / (n - 1), css(c, i));
        ctx.fillStyle = g;
        const hh = (core ? it.h * 0.12 : it.h * 0.38) * s;
        ctx.beginPath();
        ctx.roundRect(x + 4 * s, y - hh / 2, w - 8 * s, hh, hh / 2);
        ctx.fill();
      }
    }
  }

  function drawBodies(ctx: CanvasRenderingContext2D, avg: [number, number, number]) {
    for (const it of layout.items) {
      if (it.kind === 'fan') {
        const cx = it.x + it.w / 2;
        const cy = it.y + it.h / 2;
        const R = it.w / 2;
        // frame
        const body = ctx.createRadialGradient(cx, cy - R * 0.3, R * 0.1, cx, cy, R);
        body.addColorStop(0, '#101219');
        body.addColorStop(1, '#07080b');
        ctx.fillStyle = body;
        ctx.beginPath();
        ctx.arc(cx, cy, R, 0, Math.PI * 2);
        ctx.fill();
        ctx.strokeStyle = 'rgba(255,255,255,0.07)';
        ctx.lineWidth = 1;
        ctx.stroke();
        // diffuser channel
        ctx.strokeStyle = 'rgba(0,0,0,0.55)';
        ctx.lineWidth = it.w * 0.1;
        ctx.beginPath();
        ctx.arc(cx, cy, it.w * 0.4, 0, Math.PI * 2);
        ctx.stroke();
        // blades, faintly lit by the ring
        const c = cur.get(it.key);
        let lr = avg[0],
          lg = avg[1],
          lb = avg[2];
        if (c && c.length) {
          lr = lg = lb = 0;
          const n = c.length / 3;
          for (let i = 0; i < n; i++) {
            lr += c[i * 3];
            lg += c[i * 3 + 1];
            lb += c[i * 3 + 2];
          }
          lr /= n;
          lg /= n;
          lb /= n;
        }
        const r0 = it.w * 0.17;
        const r1 = it.w * 0.335;
        const bladeFill = ctx.createRadialGradient(cx, cy, r0, cx, cy, r1);
        bladeFill.addColorStop(0, `rgba(${lr | 0},${lg | 0},${lb | 0},0.03)`);
        bladeFill.addColorStop(1, `rgba(${lr | 0},${lg | 0},${lb | 0},0.13)`);
        ctx.fillStyle = bladeFill;
        for (let b = 0; b < 9; b++) {
          const a = (b / 9) * Math.PI * 2;
          const sweep = 0.55;
          ctx.beginPath();
          ctx.moveTo(cx + Math.cos(a) * r0, cy + Math.sin(a) * r0);
          ctx.quadraticCurveTo(
            cx + Math.cos(a + sweep * 0.35) * r1 * 0.78,
            cy + Math.sin(a + sweep * 0.35) * r1 * 0.78,
            cx + Math.cos(a + sweep) * r1,
            cy + Math.sin(a + sweep) * r1,
          );
          ctx.arc(cx, cy, r1, a + sweep, a + sweep + 0.36);
          ctx.quadraticCurveTo(
            cx + Math.cos(a + 0.36) * r1 * 0.6,
            cy + Math.sin(a + 0.36) * r1 * 0.6,
            cx + Math.cos(a + 0.42) * r0,
            cy + Math.sin(a + 0.42) * r0,
          );
          ctx.closePath();
          ctx.fill();
        }
        if (hl(it)) {
          ctx.strokeStyle = 'rgba(200,214,255,0.55)';
          ctx.lineWidth = 1.5;
          ctx.beginPath();
          ctx.arc(cx, cy, R + 5, 0, Math.PI * 2);
          ctx.stroke();
        }
        // hub
        ctx.fillStyle = '#090a0e';
        ctx.beginPath();
        ctx.arc(cx, cy, it.w * 0.2, 0, Math.PI * 2);
        ctx.fill();
        ctx.strokeStyle = 'rgba(255,255,255,0.06)';
        ctx.lineWidth = 1;
        ctx.stroke();
      } else {
        // strip housing
        ctx.fillStyle = '#08090c';
        ctx.beginPath();
        ctx.roundRect(it.x, it.y, it.w, it.h, it.h / 2);
        ctx.fill();
        ctx.strokeStyle = 'rgba(255,255,255,0.08)';
        ctx.lineWidth = 1;
        ctx.stroke();
        if (hl(it)) {
          ctx.strokeStyle = 'rgba(200,214,255,0.55)';
          ctx.lineWidth = 1.5;
          ctx.beginPath();
          ctx.roundRect(it.x - 5, it.y - 5, it.w + 10, it.h + 10, (it.h + 10) / 2);
          ctx.stroke();
        }
      }
    }
  }

  let ctx: CanvasRenderingContext2D | null = null;
  // Bloom pyramid: emitters at 1/4, successively halved (each halving is a
  // cheap box blur), then upsampled back through the levels. No ctx.filter.
  const LEVELS = [4, 8, 16, 32];
  const down = LEVELS.map(() => document.createElement('canvas'));
  const up = LEVELS.map(() => document.createElement('canvas'));
  const dctx = down.map((c) => c.getContext('2d')!);
  const uctx = up.map((c) => c.getContext('2d')!);
  let dpr = 1;
  let cssW = 0;
  let cssH = 0;

  function sizeCanvases() {
    if (!canvas) return;
    dpr = Math.min(2, window.devicePixelRatio || 1);
    cssW = width;
    cssH = canvasH;
    canvas.width = Math.max(1, Math.round(cssW * dpr));
    canvas.height = Math.max(1, Math.round(cssH * dpr));
    LEVELS.forEach((f, i) => {
      down[i].width = up[i].width = Math.max(1, Math.ceil(cssW / f));
      down[i].height = up[i].height = Math.max(1, Math.ceil(cssH / f));
      dctx[i].imageSmoothingQuality = uctx[i].imageSmoothingQuality = 'high';
    });
    ctx = canvas.getContext('2d');
    settled = false;
  }

  let avgTick = 0;
  function render() {
    if (!ctx) return;
    // average colour of the zone (drives the glass tint)
    let ar = 0,
      ag = 0,
      ab = 0,
      an = 0;
    for (const c of cur.values()) {
      for (let i = 0; i < c.length; i += 3) {
        ar += c[i];
        ag += c[i + 1];
        ab += c[i + 2];
        an++;
      }
    }
    const avg: [number, number, number] = an ? [ar / an, ag / an, ab / an] : [0, 0, 0];
    if (++avgTick % 20 === 1) {
      const lum = (avg[0] + avg[1] + avg[2]) / 765;
      glow = `rgba(${avg[0] | 0},${avg[1] | 0},${avg[2] | 0},${(0.1 + lum * 0.22).toFixed(3)})`;
    }

    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.globalCompositeOperation = 'source-over';
    ctx.globalAlpha = 1;
    ctx.filter = 'none';
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    drawBodies(ctx, avg);

    // bloom pyramid
    dctx[0].clearRect(0, 0, down[0].width, down[0].height);
    drawEmitters(dctx[0], 0.25, false);
    for (let i = 1; i < LEVELS.length; i++) {
      dctx[i].clearRect(0, 0, down[i].width, down[i].height);
      dctx[i].drawImage(down[i - 1], 0, 0, down[i].width, down[i].height);
    }
    // upsample: up[i] = down[i] + up[i+1] scaled up (weights favour the wide levels)
    const L = LEVELS.length - 1;
    uctx[L].clearRect(0, 0, up[L].width, up[L].height);
    uctx[L].drawImage(down[L], 0, 0);
    for (let i = L - 1; i >= 1; i--) {
      const u = uctx[i];
      u.globalCompositeOperation = 'source-over';
      u.globalAlpha = 1;
      u.clearRect(0, 0, up[i].width, up[i].height);
      u.drawImage(up[i + 1], 0, 0, up[i].width, up[i].height);
      u.globalCompositeOperation = 'lighter';
      u.globalAlpha = 0.7;
      u.drawImage(down[i], 0, 0);
    }

    ctx.imageSmoothingEnabled = true;
    ctx.imageSmoothingQuality = 'high';
    ctx.globalCompositeOperation = 'lighter';
    ctx.globalAlpha = 0.8;
    ctx.drawImage(up[1], 0, 0, up[1].width * LEVELS[1], up[1].height * LEVELS[1]);
    ctx.globalAlpha = 0.32;
    ctx.drawImage(down[0], 0, 0, down[0].width * LEVELS[0], down[0].height * LEVELS[0]);
    // the diffuser itself, then a hot core
    ctx.globalAlpha = 0.86;
    drawEmitters(ctx, 1, false);
    ctx.globalAlpha = 0.38;
    drawEmitters(ctx, 1, true);
    ctx.globalAlpha = 1;
    ctx.globalCompositeOperation = 'source-over';
  }

  // run gating
  const running = $derived(active && onScreen && layout.items.length > 0 && width > 0);

  $effect(() => {
    // resize canvases when the layout box changes
    void width;
    void canvasH;
    sizeCanvases();
    render();
  });

  $effect(() => {
    // new structure: force a redraw
    void layout;
    settled = false;
  });

  $effect(() => {
    if (!running) return;
    let id = 0;
    let last = performance.now();
    const frame = (now: number) => {
      const dt = Math.min(0.1, (now - last) / 1000);
      last = now;
      if (!document.hidden) {
        const fresh = rgb.frames !== lastFrames;
        lastFrames = rgb.frames;
        const moving = step(dt);
        if (moving || fresh || !settled) {
          render();
          settled = !moving;
        }
      }
      id = requestAnimationFrame(frame);
    };
    const onVis = () => {
      cancelAnimationFrame(id);
      if (!document.hidden) {
        last = performance.now();
        id = requestAnimationFrame(frame);
      }
    };
    document.addEventListener('visibilitychange', onVis);
    id = requestAnimationFrame(frame);
    return () => {
      cancelAnimationFrame(id);
      document.removeEventListener('visibilitychange', onVis);
    };
  });

  onMount(() => {
    const ro = new ResizeObserver(() => (width = host.clientWidth));
    ro.observe(host);
    width = host.clientWidth;
    const io = new IntersectionObserver((e) => (onScreen = e[0]?.isIntersecting ?? true));
    io.observe(host);
    return () => {
      ro.disconnect();
      io.disconnect();
    };
  });

  // ---------------------------------------------------------------------------
  // Header copy
  // ---------------------------------------------------------------------------
  const lit = $derived(rgb.config?.enabled ?? false);
  const hl = (it: Item) => lighting.hoverKey !== null && refKey(it.ref) === lighting.hoverKey;
  $effect(() => {
    void lighting.hoverKey;
    settled = false;
  });
  let zb = $state(100);
  $effect(() => {
    zb = zone?.brightness ?? 100;
  });
</script>

<section class="stage" aria-label="Live preview" style:--glow={lit ? glow : 'rgba(0,0,0,0)'}>
  <div class="sheen" aria-hidden="true"></div>
  <header class="head">
    <div class="titles">
      <h2 class="t-title3 name">{zone ? zone.name : 'Live preview'}</h2>
    </div>
    {#if zone}
      <div class="zone-bright">
        <Slider
          label="Zone brightness"
          min={0}
          max={100}
          step={5}
          bind:value={zb}
          format={(v) => `${v}%`}
          oninput={(v) => onzonebrightness(v, false)}
          oncommit={(v) => onzonebrightness(v, true)}
        />
      </div>
    {/if}
  </header>

  <!-- focusable so keyboard users can reveal the device names (hover does it for pointers) -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    class="canvas-host"
    bind:this={host}
    style:height="{canvasH}px"
    role="group"
    tabindex={layout.items.length > 0 ? 0 : -1}
    aria-label="Live preview of {layout.items.length} {layout.items.length === 1 ? 'light' : 'lights'}. Hover or focus to show names."
  >
    <canvas bind:this={canvas} style:width="{width}px" style:height="{canvasH}px" aria-hidden="true"></canvas>
    {#each layout.items as it (it.key)}
      {#if it.kind === 'fan'}
        <span
          class="fan-label"
          class:hl={hl(it)}
          style:left="{it.x + it.w / 2}px"
          style:top="{it.y + it.h / 2}px"
          style:max-width="{it.w * 0.62}px"
          style:font-size="{it.w < 104 ? 10.5 : 11.5}px"
          title={it.label}
          >{#if /^Hub \S+ Ch \d+$/.test(it.label)}{it.label.replace(/ Ch \d+$/, '')}<span class="ch"
              >{it.label.match(/Ch \d+$/)?.[0]}</span
            >{:else}{it.label}{/if}</span
        >
      {:else}
        <span class="strip-label" class:hl={hl(it)} style:left="{it.x + 2}px" style:top="{it.y + it.h + 8}px" style:max-width="{it.w}px" title={it.label}
          >{it.label}</span
        >
      {/if}
    {/each}

    {#if loaded && zone && layout.items.length === 0}
      <div class="empty">
        <span class="empty-icon" aria-hidden="true"><Lightbulb size={22} strokeWidth={1.75} /></span>
        <p class="t-headline">Add devices to this zone to preview</p>
        <p class="t-callout t3">Assign hub channels from the zone settings</p>
      </div>
    {/if}
  </div>
</section>

<style>
  .stage {
    position: relative;
    isolation: isolate;
    border-radius: var(--r-sheet);
    overflow: hidden;
    background:
      radial-gradient(120% 90% at 50% 105%, var(--glow), transparent 62%),
      radial-gradient(90% 70% at 50% -10%, rgba(124, 156, 255, 0.06), transparent 60%),
      linear-gradient(180deg, #0a0b10 0%, #040507 100%);
    box-shadow:
      inset 0 0 0 1px var(--hair),
      inset 0 1px 0 rgba(255, 255, 255, 0.07),
      var(--sh-2);
  }
  /* glass sheen: a soft diagonal reflection across the top */
  .sheen {
    position: absolute;
    inset: 0;
    z-index: 2;
    pointer-events: none;
    background:
      linear-gradient(168deg, rgba(255, 255, 255, 0.055) 0%, rgba(255, 255, 255, 0.012) 28%, transparent 42%),
      linear-gradient(0deg, rgba(0, 0, 0, 0.35), transparent 22%);
  }
  .head {
    position: relative;
    z-index: 3;
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 24px;
    padding: 20px 24px 8px;
  }
  .titles {
    min-width: 0;
  }
  .name {
    color: var(--t1);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .zone-bright {
    flex: none;
    width: 220px;
  }
  .canvas-host {
    position: relative;
    z-index: 1;
    /* the canvas carries GP (44px) of empty glow margin above and below */
    margin: -32px 0 -16px;
  }
  canvas {
    position: absolute;
    left: 0;
    top: 0;
    display: block;
  }
  .fan-label {
    position: absolute;
    transform: translate(-50%, -50%);
    text-align: center;
    line-height: 1.2;
    font-weight: 500;
    color: var(--t2);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    pointer-events: auto;
    font-variant-numeric: tabular-nums;
    text-shadow: 0 1px 2px rgb(0 0 0 / 0.8);
  }
  /* names appear on demand: hovering/focusing the stage, or the matching
     row in the device list (their single always-visible home) */
  .fan-label,
  .strip-label {
    opacity: 0;
    transition: opacity var(--dur-med) var(--ease-out-expo);
  }
  .canvas-host:hover .fan-label,
  .canvas-host:hover .strip-label,
  .canvas-host:focus-visible .fan-label,
  .canvas-host:focus-visible .strip-label,
  .fan-label.hl,
  .strip-label.hl {
    opacity: 1;
  }
  .canvas-host:focus-visible {
    box-shadow: none;
    outline: none;
  }
  .fan-label .ch {
    display: block;
    font-size: 0.92em;
    font-weight: 500;
    color: var(--t3);
  }
  .strip-label {
    position: absolute;
    font-size: 12px;
    line-height: 16px;
    font-weight: 500;
    color: var(--t2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-variant-numeric: tabular-nums;
  }
  .empty {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    text-align: center;
    padding: 0 24px;
  }
  .empty .t-headline {
    color: var(--t1);
  }
  .empty-icon {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    margin-bottom: 8px;
    border-radius: 50%;
    color: var(--t2);
    background: rgba(255, 255, 255, 0.04);
    box-shadow: inset 0 0 0 1px var(--hair);
  }
</style>
