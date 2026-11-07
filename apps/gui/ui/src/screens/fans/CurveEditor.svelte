<!--
  CurveEditor (inventory 2.3), the centrepiece of Fan Control.

  - axes 20–100 °C × 0–100 %, gridlines/labels temps 30–90 (10), duty 20–100 (20)
  - zone shading green <70, amber 70–85, red >85 (+ a zone strip on the axis)
  - smooth monotone curve (2 points = straight line) with gradient under-fill;
    dashed flat extensions show the clamp beyond the end points
  - draggable handles: integer temp/duty, live re-sort, the drag follows the
    point, highlighted while dragging, role=slider + aria-label
    "Curve point N: T°C → D%", arrow-key nudging (Shift = 5), Delete removes
  - hover tooltip "T°C → D%"; click empty plot adds a point; right-click a
    handle removes it (min 2)
  - live CPU temp dashed line (display.snapshot via the parent, so it follows
    the time-scrub), ±hysteresis band, live operating point dot + "N%"
  - validation (role=status) with the exact error/warning copy
  - additions: numeric entry for the selected point, undo, sensor-mix marker,
    axis guides while dragging, add-point ghost, spring morphs (presets: PresetChips)
-->
<script lang="ts">
  import { tick, untrack } from 'svelte';
  import { Spring } from 'svelte/motion';
  import Undo2 from 'lucide-svelte/icons/undo-2';
  import Trash2 from 'lucide-svelte/icons/trash-2';
  import CircleAlert from 'lucide-svelte/icons/circle-alert';
  import TriangleAlert from 'lucide-svelte/icons/triangle-alert';
  import NumberField from '../../components/ui/NumberField.svelte';
  import IconButton from '../../components/ui/IconButton.svelte';
  import { followSpring, reducedMotion, springs } from '../../lib/motion.svelte';
  import { monotonePath } from '../../lib/charts/monotone';
  import type { CurvePoint } from '../../lib/types';
  import { clampN, curveIssues } from './fanModes';

  interface Props {
    points: CurvePoint[];
    hysteresis: number;
    color?: string;
    /** live CPU temperature (°C), from display.snapshot */
    currentTemp: number | null;
    /** live sensor-mix temperature (°C): where the control loop actually reads */
    mixTemp?: number | null;
    height?: number;
    canUndo?: boolean;
    onchange: (points: CurvePoint[]) => void;
    oncheckpoint?: (key: string) => void;
    onundo?: () => void;
  }
  let {
    points,
    hysteresis,
    color = 'var(--accent)',
    currentTemp,
    mixTemp = null,
    height = 380,
    canUndo = false,
    onchange,
    oncheckpoint,
    onundo,
  }: Props = $props();

  const T0 = 20;
  const T1 = 100;
  const ML = 46;
  const MR = 18;
  const MT = 18;
  const MB = 30;
  const TEMP_TICKS = [30, 40, 50, 60, 70, 80, 90];
  const DUTY_TICKS = [20, 40, 60, 80, 100];

  let W = $state(800);
  const H = $derived(height);
  const pw = $derived(Math.max(10, W - ML - MR));
  const ph = $derived(Math.max(10, H - MT - MB));
  const x = (t: number) => ML + ((t - T0) / (T1 - T0)) * pw;
  const y = (d: number) => MT + ph - (d / 100) * ph;
  const invT = (px: number) => T0 + ((px - ML) / pw) * (T1 - T0);
  const invD = (py: number) => ((MT + ph - py) / ph) * 100;

  const uid = `ce-${Math.random().toString(36).slice(2, 8)}`;

  // ---------------------------------------------------------------------
  // Monotone cubic evaluation (Fritsch–Carlson; same maths as the drawn path)
  // ---------------------------------------------------------------------
  function tangents(p: CurvePoint[]): number[] {
    const n = p.length;
    const m = new Array<number>(n).fill(0);
    if (n < 2) return m;
    const d: number[] = [];
    for (let i = 0; i < n - 1; i++) {
      const dx = p[i + 1].temp - p[i].temp;
      d.push(dx === 0 ? 0 : (p[i + 1].duty - p[i].duty) / dx);
    }
    m[0] = d[0];
    m[n - 1] = d[n - 2];
    for (let i = 1; i < n - 1; i++) m[i] = d[i - 1] * d[i] <= 0 ? 0 : (d[i - 1] + d[i]) / 2;
    for (let i = 0; i < n - 1; i++) {
      if (d[i] === 0) {
        m[i] = 0;
        m[i + 1] = 0;
        continue;
      }
      const a = m[i] / d[i];
      const b = m[i + 1] / d[i];
      const s = a * a + b * b;
      if (s > 9) {
        const k = 3 / Math.sqrt(s);
        m[i] = k * a * d[i];
        m[i + 1] = k * b * d[i];
      }
    }
    return m;
  }
  function curveAt(p: CurvePoint[], t: number): number {
    if (p.length === 0) return 0;
    if (t <= p[0].temp) return p[0].duty;
    const last = p[p.length - 1];
    if (t >= last.temp) return last.duty;
    const m = tangents(p);
    for (let i = 0; i < p.length - 1; i++) {
      const a = p[i];
      const b = p[i + 1];
      if (t >= a.temp && t <= b.temp) {
        const h = b.temp - a.temp;
        if (h <= 0) return b.duty;
        if (p.length === 2) return a.duty + ((b.duty - a.duty) * (t - a.temp)) / h;
        const s = (t - a.temp) / h;
        const h00 = 2 * s ** 3 - 3 * s ** 2 + 1;
        const h10 = s ** 3 - 2 * s ** 2 + s;
        const h01 = -2 * s ** 3 + 3 * s ** 2;
        const h11 = s ** 3 - s ** 2;
        return h00 * a.duty + h10 * h * m[i] + h01 * b.duty + h11 * h * m[i + 1];
      }
    }
    return last.duty;
  }

  // ---------------------------------------------------------------------
  // Displayed points spring toward the config (instant while dragging)
  // ---------------------------------------------------------------------
  const plain = (p: CurvePoint[]) => p.map((q) => ({ temp: q.temp, duty: q.duty }));
  const shown = new Spring<CurvePoint[]>(
    untrack(() => plain(points)),
    { ...springs.tab },
  );
  let instantNext = false;
  $effect(() => {
    const target = plain(points);
    const instant = instantNext || untrack(() => reducedMotion.current);
    instantNext = false;
    const cur = untrack(() => shown.current);
    if (instant) {
      shown.set(target, { instant: true });
      return;
    }
    if (cur.length !== target.length) {
      shown.set(
        target.map((p) => ({ temp: p.temp, duty: curveAt(cur, p.temp) })),
        { instant: true },
      );
    }
    shown.set(target);
  });

  const sp = $derived(shown.current);
  const pathPts = $derived(sp.map((p) => [x(p.temp), y(p.duty)] as [number, number]));
  const linePath = $derived(monotonePath(pathPts));
  const fillPath = $derived.by(() => {
    if (pathPts.length < 2) return '';
    const first = pathPts[0];
    const last = pathPts[pathPts.length - 1];
    const base = y(0);
    return `M${x(T0)},${first[1]}L${first[0]},${first[1]}${linePath.replace(/^M[^C L]+/, '')}L${x(T1)},${last[1]}L${x(T1)},${base}L${x(T0)},${base}Z`;
  });

  // ---------------------------------------------------------------------
  // Live marker
  // ---------------------------------------------------------------------
  const tempSpring = followSpring(() => currentTemp ?? 0, springs.bar);
  const hasLive = $derived(currentTemp !== null && points.length >= 2);
  const liveT = $derived(clampN(tempSpring.current, T0, T1));
  const liveDutyShown = $derived(curveAt(sp, liveT));
  const mixSpring = followSpring(() => mixTemp ?? 0, springs.bar);
  const showMix = $derived(mixTemp !== null && currentTemp !== null && Math.abs(mixTemp - currentTemp) >= 0.5 && points.length >= 2);
  const mixT = $derived(clampN(mixSpring.current, T0, T1));
  const liveDutyLabel = $derived(currentTemp !== null ? Math.round(curveAt(points, currentTemp)) : 0);

  // ---------------------------------------------------------------------
  // Interaction state
  // ---------------------------------------------------------------------
  let svg = $state<SVGSVGElement>();
  const handles: SVGGElement[] = $state([]);
  let dragging = $state<number | null>(null);
  let selected = $state<number | null>(null);
  let hovered = $state<number | null>(null);
  let ghost = $state<{ temp: number; duty: number } | null>(null);
  let dragMoved = false;
  let dragRef: CurvePoint | null = null;

  $effect(() => {
    if (selected !== null && selected >= points.length) selected = null;
  });

  const issues = $derived(curveIssues(points));
  const dupTemps = $derived.by(() => {
    const s = new Set<number>();
    const seen = new Set<number>();
    for (const p of points) {
      if (seen.has(p.temp)) s.add(p.temp);
      seen.add(p.temp);
    }
    return s;
  });

  function local(e: PointerEvent | MouseEvent) {
    const r = svg!.getBoundingClientRect();
    return { px: e.clientX - r.left, py: e.clientY - r.top };
  }
  const inPlot = (px: number, py: number) => px >= ML && px <= ML + pw && py >= MT && py <= MT + ph;
  const toPoint = (px: number, py: number) => ({
    temp: Math.round(clampN(invT(px), T0, T1)),
    duty: Math.round(clampN(invD(py), 0, 100)),
  });

  function commit(next: CurvePoint[], focusRef: CurvePoint | null, instant = true) {
    const sorted = [...next].sort((a, b) => a.temp - b.temp);
    const idx = focusRef ? sorted.indexOf(focusRef) : -1;
    instantNext = instant;
    onchange(sorted.map((p) => ({ temp: p.temp, duty: p.duty })));
    return idx;
  }

  function onHandleDown(e: PointerEvent, i: number) {
    if (e.button !== 0) return;
    e.stopPropagation();
    e.preventDefault();
    oncheckpoint?.(`drag-${Date.now()}`);
    dragging = i;
    selected = i;
    dragMoved = false;
    dragRef = { ...points[i] };
    ghost = null;
    svg?.setPointerCapture(e.pointerId);
    handles[i]?.focus({ preventScroll: true });
  }

  function onMove(e: PointerEvent) {
    const { px, py } = local(e);
    if (dragging === null) {
      const overHandle = (e.target as Element).closest?.('.handle');
      ghost = !overHandle && inPlot(px, py) ? toPoint(px, py) : null;
      return;
    }
    const p = toPoint(px, py);
    if (dragRef && p.temp === dragRef.temp && p.duty === dragRef.duty) return;
    dragMoved = true;
    const ref = { ...p };
    const next = points.map((q, j) => (j === dragging ? ref : { temp: q.temp, duty: q.duty }));
    const idx = commit(next, ref);
    dragRef = ref;
    dragging = idx;
    selected = idx;
  }

  async function onUp(e: PointerEvent) {
    if (dragging === null) return;
    const idx = dragging;
    dragging = null;
    if (svg?.hasPointerCapture(e.pointerId)) svg.releasePointerCapture(e.pointerId);
    await tick();
    handles[idx]?.focus({ preventScroll: true });
  }

  function onPlotClick(e: MouseEvent) {
    if (dragMoved) {
      dragMoved = false;
      return;
    }
    if ((e.target as Element).closest('.handle')) return;
    const { px, py } = local(e);
    if (!inPlot(px, py)) return;
    const p = toPoint(px, py);
    oncheckpoint?.(`add-${Date.now()}`);
    const ref = { ...p };
    const idx = commit([...points.map((q) => ({ ...q })), ref], ref);
    selected = idx;
    ghost = null;
  }

  function remove(i: number) {
    if (points.length <= 2) return;
    oncheckpoint?.(`remove-${Date.now()}`);
    commit(
      points.filter((_, j) => j !== i).map((q) => ({ ...q })),
      null,
    );
    selected = null;
    hovered = null;
  }

  function onContext(e: MouseEvent, i: number) {
    e.preventDefault();
    remove(i);
  }

  async function onHandleKey(e: KeyboardEvent, i: number) {
    const big = e.shiftKey ? 5 : 1;
    let dt = 0;
    let dd = 0;
    if (e.key === 'ArrowLeft') dt = -big;
    else if (e.key === 'ArrowRight') dt = big;
    else if (e.key === 'ArrowUp') dd = big;
    else if (e.key === 'ArrowDown') dd = -big;
    else if (e.key === 'Delete' || e.key === 'Backspace') {
      e.preventDefault();
      remove(i);
      return;
    } else if (e.key === 'Escape' && selected !== null) {
      selected = null;
      (e.currentTarget as SVGElement).blur();
      e.preventDefault();
      return;
    } else return;
    e.preventDefault();
    oncheckpoint?.('nudge');
    const cur = points[i];
    const ref = { temp: clampN(cur.temp + dt, T0, T1), duty: clampN(cur.duty + dd, 0, 100) };
    const idx = commit(
      points.map((q, j) => (j === i ? ref : { ...q })),
      ref,
    );
    selected = idx;
    await tick();
    handles[idx]?.focus({ preventScroll: true });
  }

  // Numeric entry for the selected point (springs into place)
  function setSelected(field: 'temp' | 'duty', v: number) {
    if (selected === null) return;
    const cur = points[selected];
    if (!cur || cur[field] === v) return;
    oncheckpoint?.(`num-${Date.now()}`);
    const ref = { ...cur, [field]: v };
    const idx = commit(
      points.map((q, j) => (j === selected ? ref : { ...q })),
      ref,
      false,
    );
    selected = idx;
  }

  const tipPoint = $derived.by(() => {
    // the selected point's values live in the numeric entry; the tooltip
    // covers hover previews of other points and the point being dragged
    const i = dragging ?? (hovered !== selected ? hovered : null);
    if (i === null || !points[i] || !sp[i]) return null;
    return { i, temp: points[i].temp, duty: points[i].duty, px: x(sp[i].temp), py: y(sp[i].duty) };
  });
  const guide = $derived.by(() => {
    const i = dragging ?? selected;
    if (i === null || !sp[i] || !points[i]) return null;
    return { px: x(sp[i].temp), py: y(sp[i].duty), temp: points[i].temp, duty: points[i].duty, active: dragging !== null };
  });

</script>

<div class="ce" style:--gc={color}>
  <div class="plot" bind:clientWidth={W} style:height="{H}px">
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
    <svg
      bind:this={svg}
      width={W}
      height={H}
      viewBox="0 0 {W} {H}"
      role="application"
      aria-label="Fan curve editor — click to add points, right-click to remove"
      class:dragging={dragging !== null}
      onpointermove={onMove}
      onpointerup={onUp}
      onpointercancel={onUp}
      onpointerleave={() => (ghost = null)}
      onpointerdown={() => (dragMoved = false)}
      onclick={onPlotClick}
    >
      <defs>
        <linearGradient id="{uid}-fill" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stop-color={color} stop-opacity="0.34" />
          <stop offset="70%" stop-color={color} stop-opacity="0.07" />
          <stop offset="100%" stop-color={color} stop-opacity="0.01" />
        </linearGradient>
        <linearGradient id="{uid}-zone" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stop-color="#fff" stop-opacity="1" />
          <stop offset="100%" stop-color="#fff" stop-opacity="0.35" />
        </linearGradient>
        <mask id="{uid}-zmask"><rect x={ML} y={MT} width={pw} height={ph} fill="url(#{uid}-zone)" /></mask>
        <clipPath id="{uid}-clip"><rect x={ML} y={MT - 2} width={pw} height={ph + 4} /></clipPath>
      </defs>

      <!-- plot surface + zones -->
      <rect class="surface" x={ML} y={MT} width={pw} height={ph} rx="6" />
      <g mask="url(#{uid}-zmask)">
        <rect class="z-ok" x={ML} y={MT} width={x(70) - ML} height={ph} />
        <rect class="z-warm" x={x(70)} y={MT} width={x(85) - x(70)} height={ph} />
        <rect class="z-hot" x={x(85)} y={MT} width={ML + pw - x(85)} height={ph} />
      </g>
      <rect class="zs-ok" x={ML} y={MT + ph - 2} width={x(70) - ML} height="2" />
      <rect class="zs-warm" x={x(70)} y={MT + ph - 2} width={x(85) - x(70)} height="2" />
      <rect class="zs-hot" x={x(85)} y={MT + ph - 2} width={ML + pw - x(85)} height="2" />

      <text class="zl zl-warm" x={x(70) + 8} y={MT + ph - 9}>Warm</text>
      <text class="zl zl-hot" x={x(85) + 8} y={MT + ph - 9}>Hot</text>

      <!-- grid -->
      {#each TEMP_TICKS as t (t)}
        <line class="grid" x1={x(t)} x2={x(t)} y1={MT} y2={MT + ph} />
        <text class="ax ax-x" x={x(t)} y={MT + ph + 19}>{t}°</text>
      {/each}
      {#each DUTY_TICKS as d (d)}
        <line class="grid" x1={ML} x2={ML + pw} y1={y(d)} y2={y(d)} />
        <text class="ax ax-y" x={ML - 10} y={y(d) + 4}>{d}%</text>
      {/each}
      <line class="base" x1={ML} x2={ML + pw} y1={MT + ph} y2={MT + ph} />

      <!-- hysteresis band + live temperature -->
      {#if hasLive}
        {@const lo = clampN(liveT - hysteresis, T0, T1)}
        {@const hi = clampN(liveT + hysteresis, T0, T1)}
        {#if hysteresis > 0}
          <rect class="hyst" x={x(lo)} y={MT} width={Math.max(0, x(hi) - x(lo))} height={ph} />
          <line class="hyst-edge" x1={x(lo)} x2={x(lo)} y1={MT} y2={MT + ph} />
          <line class="hyst-edge" x1={x(hi)} x2={x(hi)} y1={MT} y2={MT + ph} />
        {/if}
        <line class="live-line" x1={x(liveT)} x2={x(liveT)} y1={MT} y2={MT + ph} />
      {/if}

      <!-- curve -->
      <g clip-path="url(#{uid}-clip)">
        <path class="fill" d={fillPath} fill="url(#{uid}-fill)" />
        {#if pathPts.length >= 2}
          <line class="ext" x1={x(T0)} x2={pathPts[0][0]} y1={pathPts[0][1]} y2={pathPts[0][1]} />
          <line
            class="ext"
            x1={pathPts[pathPts.length - 1][0]}
            x2={x(T1)}
            y1={pathPts[pathPts.length - 1][1]}
            y2={pathPts[pathPts.length - 1][1]}
          />
        {/if}
        <path class="line-glow" d={linePath} />
        <path class="line" d={linePath} />
      </g>

      <!-- guides to the axes for the selected / dragged point -->
      {#if guide}
        <g class="guides" class:active={guide.active} aria-hidden="true">
          <line x1={ML} x2={guide.px} y1={guide.py} y2={guide.py} />
          <line x1={guide.px} x2={guide.px} y1={guide.py} y2={MT + ph} />
        </g>
      {/if}

      <!-- add-point ghost -->
      {#if ghost && dragging === null}
        <circle class="ghost" cx={x(ghost.temp)} cy={y(ghost.duty)} r="5" />
      {/if}

      <!-- sensor-mix marker: where the loop actually reads -->
      {#if showMix}
        {@const mx = x(mixT)}
        {@const my = y(curveAt(sp, mixT))}
        <g class="mix" aria-hidden="true">
          <line x1={mx} x2={mx} y1={my + 6} y2={MT + ph - 24} />
          <circle cx={mx} cy={my} r="4.5" />
          <g transform="translate({mx},{MT + ph - 24})">
            <rect x="-34" width="68" height="18" rx="9" />
            <text x="0" y="13">Mix {(mixTemp ?? 0).toFixed(1)}°</text>
          </g>
        </g>
      {/if}

      <!-- live operating point -->
      {#if hasLive}
        {@const lx = x(liveT)}
        {@const ly = y(liveDutyShown)}
        <g class="op" transform="translate({lx},{ly})" aria-hidden="true">
          <circle class="op-halo" r="11" />
          <circle class="op-dot" r="5" />
        </g>
        <g transform="translate({Math.min(lx + 12, ML + pw - 44)},{ly - 26 < MT + 4 ? ly + 8 : ly - 26})" aria-hidden="true">
          <rect class="op-pill" width="42" height="20" rx="10" />
          <text class="op-t" x="21" y="14">{liveDutyLabel}%</text>
        </g>
        <g transform="translate({clampN(x(liveT), ML + 34, ML + pw - 34)},{MT - 19})" aria-hidden="true">
          <rect class="cpu-pill" x="-34" y="0" width="68" height="18" rx="9" />
          <text class="cpu-t" x="0" y="13">CPU {(currentTemp ?? 0).toFixed(1)}°</text>
        </g>
      {/if}

      <!-- handles -->
      {#each sp as p, i (i)}
        {@const real = points[i]}
        {#if real}
          {@const dup = dupTemps.has(real.temp)}
          <g
            class="handle"
            class:sel={selected === i}
            class:drag={dragging === i}
            class:dup
            transform="translate({x(p.temp)},{y(p.duty)})"
            role="slider"
            tabindex="0"
            aria-label="Curve point {i + 1}: {real.temp}°C → {real.duty}%"
            aria-valuenow={real.duty}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuetext="{real.temp}°C → {real.duty}%"
            bind:this={handles[i]}
            onpointerdown={(e) => onHandleDown(e, i)}
            onpointerenter={() => (hovered = i)}
            onpointerleave={() => (hovered = null)}
            oncontextmenu={(e) => onContext(e, i)}
            onkeydown={(e) => onHandleKey(e, i)}
            onfocus={() => (selected = i)}
          >
            <circle class="hit" r="15" />
            <circle class="ring" r="11" />
            <circle class="knob" r="6" />
          </g>
        {/if}
      {/each}
    </svg>

    {#if tipPoint}
      <div
        class="tip num"
        class:below={tipPoint.py < MT + 44}
        style:left="{tipPoint.px}px"
        style:top="{tipPoint.py}px"
        aria-hidden="true"
      >
        {tipPoint.temp}°C → {tipPoint.duty}%
      </div>
    {:else if ghost}
      <div class="tip ghost-tip num" style:left="{x(ghost.temp)}px" style:top="{y(ghost.duty)}px" aria-hidden="true">
        Add {ghost.temp}°C → {ghost.duty}%
      </div>
    {/if}
  </div>

  <div class="foot">
    {#if selected !== null && points[selected]}
      <div class="entry" class:hidden-entry={dragging !== null}>
        <span class="entry-l num">Point {selected + 1}</span>
        <NumberField
          label="Temperature"
          showLabel
          value={points[selected].temp}
          min={T0}
          max={T1}
          step={1}
          unit="°C"
          width={64}
          onchange={(v) => setSelected('temp', v)}
        />
        <NumberField
          label="Duty"
          showLabel
          value={points[selected].duty}
          min={0}
          max={100}
          step={1}
          unit="%"
          width={60}
          onchange={(v) => setSelected('duty', v)}
        />
        <IconButton
          icon={Trash2}
          label={points.length <= 2 ? 'A curve needs at least 2 points' : 'Remove point'}
          size="sm"
          disabled={points.length <= 2}
          onclick={() => selected !== null && remove(selected)}
        />
      </div>
    {:else}
      <p class="hints">
        Click the plot to add a point · Right-click a point to remove it · Arrow keys nudge a focused point (Shift = 5)
      </p>
    {/if}
    <div class="foot-r">
      <IconButton icon={Undo2} label="Undo (Ctrl+Z)" size="sm" disabled={!canUndo} onclick={() => onundo?.()} />
    </div>
  </div>

  {#if issues.error || issues.warn}
    <ul class="issues" role="status">
      {#if issues.error}
        <li class="err"><CircleAlert size={14} strokeWidth={2.25} />{issues.error}</li>
      {/if}
      {#if issues.warn}
        <li class="warn"><TriangleAlert size={14} strokeWidth={2.25} />{issues.warn}</li>
      {/if}
    </ul>
  {:else}
    <div class="sr-only" role="status"></div>
  {/if}
</div>

<style>
  .ce {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .plot {
    position: relative;
    width: 100%;
    margin: 10px 0 0;
  }
  svg {
    display: block;
    overflow: visible;
    touch-action: none;
    user-select: none;
    -webkit-user-select: none;
    cursor: crosshair;
  }
  svg.dragging {
    cursor: grabbing;
  }
  svg:focus-visible {
    box-shadow: none;
  }
  .surface {
    fill: rgba(0, 0, 0, 0.16);
  }
  .z-ok {
    fill: rgba(50, 215, 75, 0.045);
  }
  .z-warm {
    fill: rgba(255, 159, 10, 0.075);
  }
  .z-hot {
    fill: rgba(255, 69, 58, 0.095);
  }
  .zl {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    pointer-events: none;
  }
  .zl-warm {
    fill: var(--warn-text);
    opacity: 0.7;
  }
  .zl-hot {
    fill: var(--error-text);
    opacity: 0.75;
  }
  .mix line {
    stroke: rgba(255, 255, 255, 0.28);
    stroke-width: 1;
    stroke-dasharray: 1 3;
  }
  .mix circle {
    fill: #0d0f14;
    stroke: var(--t1);
    stroke-width: 1.5;
  }
  .mix rect {
    fill: rgba(20, 23, 32, 0.92);
    stroke: rgba(255, 255, 255, 0.16);
  }
  .mix text {
    fill: var(--t2);
    font-size: 10.5px;
    font-weight: 600;
    text-anchor: middle;
    font-variant-numeric: tabular-nums;
    font-feature-settings: 'tnum';
  }
  .zs-ok {
    fill: var(--ok);
    opacity: 0.45;
  }
  .zs-warm {
    fill: var(--warn);
    opacity: 0.6;
  }
  .zs-hot {
    fill: var(--error);
    opacity: 0.7;
  }
  .grid {
    stroke: rgba(255, 255, 255, 0.055);
    stroke-width: 1;
    shape-rendering: crispEdges;
  }
  .base {
    stroke: rgba(255, 255, 255, 0.14);
    stroke-width: 1;
    shape-rendering: crispEdges;
  }
  .ax {
    fill: var(--t3);
    font-size: 11px;
    font-weight: 500;
    font-variant-numeric: tabular-nums;
    font-feature-settings: 'tnum';
  }
  .ax-x {
    text-anchor: middle;
  }
  .ax-y {
    text-anchor: end;
  }
  .ax {
    transition: opacity var(--dur-fast) ease-out;
  }
  .hyst {
    fill: var(--accent);
    opacity: 0.09;
  }
  .hyst-edge {
    stroke: var(--accent);
    stroke-opacity: 0.22;
    stroke-width: 1;
  }
  .live-line {
    stroke: var(--accent-strong);
    stroke-width: 1.25;
    stroke-dasharray: 4 4;
    opacity: 0.75;
  }
  .fill {
    pointer-events: none;
  }
  .ext {
    stroke: var(--gc);
    stroke-width: 1.5;
    stroke-dasharray: 2 5;
    stroke-linecap: round;
    opacity: 0.5;
    pointer-events: none;
  }
  .line {
    fill: none;
    stroke: var(--gc);
    stroke-width: 2.5;
    stroke-linecap: round;
    stroke-linejoin: round;
    pointer-events: none;
  }
  .line-glow {
    fill: none;
    stroke: var(--gc);
    stroke-width: 8;
    stroke-linecap: round;
    stroke-linejoin: round;
    opacity: 0.12;
    pointer-events: none;
  }

  .guides line {
    stroke: rgba(255, 255, 255, 0.35);
    stroke-width: 1;
    stroke-dasharray: 3 3;
    shape-rendering: crispEdges;
  }

  .ghost {
    fill: none;
    stroke: var(--gc);
    stroke-width: 1.5;
    stroke-dasharray: 2 2;
    opacity: 0.8;
    pointer-events: none;
  }

  .op-halo {
    fill: var(--accent);
    opacity: 0.18;
    transform-box: fill-box;
    transform-origin: center;
    animation: op-pulse 2.4s var(--ease-out-expo) infinite;
  }
  .op-dot {
    fill: var(--accent-strong);
    stroke: #fff;
    stroke-width: 2;
  }
  @keyframes op-pulse {
    0% {
      transform: scale(0.6);
      opacity: 0.32;
    }
    100% {
      transform: scale(1.6);
      opacity: 0;
    }
  }
  .op-pill {
    fill: var(--accent);
  }
  .op-t {
    fill: var(--accent-ink);
    font-size: 11.5px;
    font-weight: 700;
    text-anchor: middle;
    font-variant-numeric: tabular-nums;
    font-feature-settings: 'tnum';
  }
  .cpu-pill {
    fill: rgba(20, 23, 32, 0.92);
    stroke: rgba(124, 156, 255, 0.4);
  }
  .cpu-t {
    fill: var(--accent-strong);
    font-size: 10.5px;
    font-weight: 600;
    text-anchor: middle;
    font-variant-numeric: tabular-nums;
    font-feature-settings: 'tnum';
  }

  .handle {
    cursor: grab;
    outline: none;
  }
  .handle .hit {
    fill: transparent;
  }
  .handle .ring {
    fill: var(--gc);
    opacity: 0;
    transform-box: fill-box;
    transform-origin: center;
    transform: scale(0.5);
    transition:
      opacity var(--dur-fast) ease-out,
      transform 260ms var(--ease-out-expo);
  }
  .handle .knob {
    fill: #0d0f14;
    stroke: var(--gc);
    stroke-width: 2.5;
    transform-box: fill-box;
    transform-origin: center;
    transition:
      transform 260ms var(--ease-out-expo),
      fill var(--dur-fast) ease-out;
  }
  .handle:hover .knob {
    transform: scale(1.18);
  }
  .handle:hover .ring {
    opacity: 0.16;
    transform: scale(1);
  }
  .handle.sel .knob {
    fill: var(--gc);
    stroke: #fff;
    stroke-width: 2;
  }
  .handle:focus-visible .ring {
    opacity: 0.3;
    transform: scale(1.05);
    fill: var(--accent);
  }
  .handle.drag {
    cursor: grabbing;
  }
  .handle.drag .knob {
    fill: #fff;
    stroke: var(--gc);
    stroke-width: 3;
    transform: scale(1.3);
  }
  .handle.drag .ring {
    opacity: 0.24;
    transform: scale(1.25);
  }
  .handle.dup .knob {
    stroke: var(--error);
  }
  .handle.dup .ring {
    fill: var(--error);
    opacity: 0.22;
    transform: scale(1);
  }

  .tip {
    position: absolute;
    transform: translate(-50%, calc(-100% - 16px));
    padding: 4px 9px;
    border-radius: 8px;
    background: rgba(28, 31, 40, 0.92);
    -webkit-backdrop-filter: blur(12px);
    backdrop-filter: blur(12px);
    box-shadow:
      0 0 0 1px var(--hair-strong),
      0 6px 18px -6px rgb(0 0 0 / 0.7);
    font-size: 12px;
    font-weight: 600;
    color: var(--t1);
    white-space: nowrap;
    pointer-events: none;
  }
  .tip.below {
    transform: translate(-50%, 18px);
  }
  .ghost-tip {
    color: var(--t2);
    font-weight: 500;
  }

  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 50px;
    padding: 8px 14px;
    margin-top: 6px;
    box-shadow: inset 0 1px 0 var(--hair);
  }
  .entry {
    display: flex;
    align-items: center;
    gap: 16px;
  }
  .entry-l {
    font-size: 13px;
    font-weight: 600;
    color: var(--t1);
    min-width: 56px;
  }
  .hidden-entry {
    visibility: hidden;
  }
  .hints {
    font-size: 12px;
    color: var(--t3);
  }
  .foot-r {
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    margin-right: -4px;
  }
  .issues {
    list-style: none;
    margin: 0;
    padding: 0 14px 14px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .issues li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border-radius: var(--r-control);
    font-size: 12.5px;
    line-height: 17px;
  }
  .issues .err {
    background: var(--error-surface);
    box-shadow: inset 0 0 0 1px var(--error-hair);
    color: var(--error-text);
  }
  .issues .warn {
    background: var(--warn-surface);
    box-shadow: inset 0 0 0 1px var(--warn-hair);
    color: var(--warn-text);
  }
  @media (prefers-reduced-motion: reduce) {
    .op-halo {
      animation: none;
      opacity: 0.2;
    }
  }
</style>
