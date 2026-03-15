<script lang="ts">
  import type { CurvePoint } from '../../lib/types';

  interface Props {
    points: CurvePoint[];
    currentTemp?: number;
    onchange: (points: CurvePoint[]) => void;
    expanded?: boolean;
  }

  let { points, currentTemp, onchange, expanded = false }: Props = $props();

  let svg: SVGSVGElement;
  let dragging: number | null = $state(null);
  let hoveredPoint: number | null = $state(null);

  const TEMP_MIN = 20;
  const TEMP_MAX = 100;
  const DUTY_MIN = 0;
  const DUTY_MAX = 100;
  const PAD = 32;
  const W = $derived(expanded ? 560 : 360);
  const H = $derived(expanded ? 360 : 200);

  function tempToX(temp: number): number {
    return PAD + ((temp - TEMP_MIN) / (TEMP_MAX - TEMP_MIN)) * W;
  }

  function dutyToY(duty: number): number {
    return PAD + H - ((duty - DUTY_MIN) / (DUTY_MAX - DUTY_MIN)) * H;
  }

  function xToTemp(x: number): number {
    return TEMP_MIN + ((x - PAD) / W) * (TEMP_MAX - TEMP_MIN);
  }

  function yToDuty(y: number): number {
    return DUTY_MIN + ((PAD + H - y) / H) * (DUTY_MAX - DUTY_MIN);
  }

  function svgPoint(e: PointerEvent): { x: number; y: number } {
    const ctm = svg.getScreenCTM();
    if (!ctm) return { x: 0, y: 0 };
    const inv = ctm.inverse();
    const pt = svg.createSVGPoint();
    pt.x = e.clientX;
    pt.y = e.clientY;
    const svgPt = pt.matrixTransform(inv);
    return { x: svgPt.x, y: svgPt.y };
  }

  function onPointerDown(e: PointerEvent, index: number) {
    dragging = index;
    (e.target as Element).setPointerCapture(e.pointerId);
  }

  function onPointerMove(e: PointerEvent) {
    if (dragging === null) return;
    const { x, y } = svgPoint(e);
    const temp = Math.round(Math.max(TEMP_MIN, Math.min(TEMP_MAX, xToTemp(x))));
    const duty = Math.round(Math.max(DUTY_MIN, Math.min(DUTY_MAX, yToDuty(y))));

    const newPoints = [...points];
    newPoints[dragging] = { temp, duty };
    newPoints.sort((a, b) => a.temp - b.temp);
    dragging = newPoints.findIndex(p => p.temp === temp && p.duty === duty);
    onchange(newPoints);
  }

  function onPointerUp() {
    dragging = null;
  }

  // Click empty area to add a point
  function onSvgClick(e: MouseEvent) {
    if (dragging !== null) return;
    // Don't add if clicking on a handle
    if ((e.target as Element).closest('circle')) return;
    const ctm = svg.getScreenCTM();
    if (!ctm) return;
    const inv = ctm.inverse();
    const pt = svg.createSVGPoint();
    pt.x = e.clientX;
    pt.y = e.clientY;
    const svgPt = pt.matrixTransform(inv);
    const temp = Math.round(Math.max(TEMP_MIN, Math.min(TEMP_MAX, xToTemp(svgPt.x))));
    const duty = Math.round(Math.max(DUTY_MIN, Math.min(DUTY_MAX, yToDuty(svgPt.y))));
    // Only add if within plot area
    if (svgPt.x < PAD || svgPt.x > PAD + W || svgPt.y < PAD || svgPt.y > PAD + H) return;
    const newPoints = [...points, { temp, duty }];
    newPoints.sort((a, b) => a.temp - b.temp);
    onchange(newPoints);
  }

  // Right-click to remove a point
  function onPointContext(e: MouseEvent, index: number) {
    e.preventDefault();
    if (points.length <= 2) return;
    const newPoints = points.filter((_, i) => i !== index);
    onchange(newPoints);
  }

  // Monotone cubic bezier path (Fritsch-Carlson)
  const bezierPath = $derived.by(() => {
    if (points.length < 2) return '';
    const pts = points.map(p => ({ x: tempToX(p.temp), y: dutyToY(p.duty) }));
    if (pts.length === 2) {
      return `M ${pts[0].x} ${pts[0].y} L ${pts[1].x} ${pts[1].y}`;
    }
    let d = `M ${pts[0].x} ${pts[0].y}`;
    for (let i = 0; i < pts.length - 1; i++) {
      const p0 = pts[Math.max(0, i - 1)];
      const p1 = pts[i];
      const p2 = pts[i + 1];
      const p3 = pts[Math.min(pts.length - 1, i + 2)];
      const cp1x = p1.x + (p2.x - p0.x) / 6;
      const cp1y = p1.y + (p2.y - p0.y) / 6;
      const cp2x = p2.x - (p3.x - p1.x) / 6;
      const cp2y = p2.y - (p3.y - p1.y) / 6;
      d += ` C ${cp1x} ${cp1y}, ${cp2x} ${cp2y}, ${p2.x} ${p2.y}`;
    }
    return d;
  });

  // Fill path under the curve
  const fillPath = $derived.by(() => {
    const base = bezierPath;
    if (!base) return '';
    const lastPt = points[points.length - 1];
    const firstPt = points[0];
    return `${base} L ${tempToX(lastPt.temp)} ${dutyToY(0)} L ${tempToX(firstPt.temp)} ${dutyToY(0)} Z`;
  });

  // Live operating point — interpolate duty at currentTemp
  const livePoint = $derived.by(() => {
    if (currentTemp === undefined || points.length < 2) return null;
    const t = currentTemp;
    if (t <= points[0].temp) return { temp: t, duty: points[0].duty };
    if (t >= points[points.length - 1].temp) return { temp: t, duty: points[points.length - 1].duty };
    for (let i = 0; i < points.length - 1; i++) {
      if (t >= points[i].temp && t <= points[i + 1].temp) {
        const frac = (t - points[i].temp) / (points[i + 1].temp - points[i].temp);
        const duty = points[i].duty + frac * (points[i + 1].duty - points[i].duty);
        return { temp: t, duty };
      }
    }
    return null;
  });

  // Grid lines
  const tempGridLines = [30, 40, 50, 60, 70, 80, 90];
  const dutyGridLines = [20, 40, 60, 80, 100];
</script>

<svg
  bind:this={svg}
  viewBox="0 0 {PAD * 2 + W} {PAD * 2 + H}"
  class="curve-svg"
  role="application"
  aria-label="Fan curve editor — click to add points, right-click to remove"
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
  onclick={onSvgClick}
>
  <!-- Temperature zone shading -->
  <rect
    x={PAD} y={PAD}
    width={tempToX(70) - PAD} height={H}
    fill="rgba(52, 211, 153, 0.03)"
  />
  <rect
    x={tempToX(70)} y={PAD}
    width={tempToX(85) - tempToX(70)} height={H}
    fill="rgba(251, 191, 36, 0.03)"
  />
  <rect
    x={tempToX(85)} y={PAD}
    width={PAD + W - tempToX(85)} height={H}
    fill="rgba(248, 113, 113, 0.03)"
  />

  <!-- Grid -->
  {#each tempGridLines as t}
    <line
      x1={tempToX(t)} y1={PAD} x2={tempToX(t)} y2={PAD + H}
      stroke="rgba(255,255,255,0.05)" stroke-width="0.5"
    />
    <text x={tempToX(t)} y={PAD + H + 15} fill="rgba(255,255,255,0.3)"
      font-size="9" text-anchor="middle">{t}°</text>
  {/each}
  {#each dutyGridLines as d}
    <line
      x1={PAD} y1={dutyToY(d)} x2={PAD + W} y2={dutyToY(d)}
      stroke="rgba(255,255,255,0.05)" stroke-width="0.5"
    />
    <text x={PAD - 4} y={dutyToY(d) + 3} fill="rgba(255,255,255,0.3)"
      font-size="9" text-anchor="end">{d}%</text>
  {/each}

  <!-- Current temperature indicator line -->
  {#if currentTemp !== undefined}
    <line
      x1={tempToX(currentTemp)} y1={PAD} x2={tempToX(currentTemp)} y2={PAD + H}
      stroke="var(--accent)" stroke-width="1" stroke-dasharray="4 4" opacity="0.4"
    />
  {/if}

  <!-- Curve fill -->
  <path d={fillPath} fill="rgba(96, 165, 250, 0.08)" />

  <!-- Curve line (smooth bezier) -->
  <path d={bezierPath} fill="none" stroke="var(--accent)" stroke-width="2" />

  <!-- Live operating point -->
  {#if livePoint}
    {@const lp = livePoint}
    <circle
      cx={tempToX(lp.temp)}
      cy={dutyToY(lp.duty)}
      r="5"
      fill="var(--accent)"
      stroke="white"
      stroke-width="2"
      class="live-dot"
    />
    <text
      x={tempToX(lp.temp) + 10}
      y={dutyToY(lp.duty) + 4}
      fill="var(--accent)"
      font-size="10"
      font-weight="600"
    >{Math.round(lp.duty)}%</text>
  {/if}

  <!-- Draggable handles -->
  {#each points as point, i}
    <circle
      cx={tempToX(point.temp)}
      cy={dutyToY(point.duty)}
      r="8"
      fill={dragging === i ? 'white' : 'var(--accent)'}
      stroke="white"
      stroke-width="1.5"
      cursor="grab"
      tabindex="0"
      role="slider"
      aria-label="Curve point {i + 1}: {point.temp}°C → {point.duty}%"
      aria-valuenow={point.duty}
      aria-valuemin={DUTY_MIN}
      aria-valuemax={DUTY_MAX}
      onpointerdown={(e) => onPointerDown(e, i)}
      onpointerenter={() => { hoveredPoint = i; }}
      onpointerleave={() => { hoveredPoint = null; }}
      oncontextmenu={(e) => onPointContext(e, i)}
    />
    <!-- Tooltip on hover -->
    {#if hoveredPoint === i}
      <text
        x={tempToX(point.temp)}
        y={dutyToY(point.duty) - 14}
        fill="var(--text-primary)"
        font-size="10"
        text-anchor="middle"
        font-weight="600"
      >{point.temp}°C → {point.duty}%</text>
    {/if}
  {/each}
</svg>

<style>
  .curve-svg {
    width: 100%;
    height: 100%;
    min-height: 200px;
    display: block;
  }
  .live-dot {
    animation: pulse 2s infinite;
  }
</style>
