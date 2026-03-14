<script lang="ts">
  import type { CurvePoint } from '../../lib/types';

  interface Props {
    points: CurvePoint[];
    currentTemp?: number;
    onchange: (points: CurvePoint[]) => void;
  }

  let { points, currentTemp, onchange }: Props = $props();

  let svg: SVGSVGElement;
  let dragging: number | null = $state(null);

  // SVG viewBox: 0-100 on both axes
  // X = temperature (20-100), Y = duty (0-100, inverted for SVG)
  const TEMP_MIN = 20;
  const TEMP_MAX = 100;
  const DUTY_MIN = 0;
  const DUTY_MAX = 100;
  const PAD = 30; // padding for labels
  const SIZE = 200; // SVG content area

  function tempToX(temp: number): number {
    return PAD + ((temp - TEMP_MIN) / (TEMP_MAX - TEMP_MIN)) * SIZE;
  }

  function dutyToY(duty: number): number {
    return PAD + SIZE - ((duty - DUTY_MIN) / (DUTY_MAX - DUTY_MIN)) * SIZE;
  }

  function xToTemp(x: number): number {
    return TEMP_MIN + ((x - PAD) / SIZE) * (TEMP_MAX - TEMP_MIN);
  }

  function yToDuty(y: number): number {
    return DUTY_MIN + ((PAD + SIZE - y) / SIZE) * (DUTY_MAX - DUTY_MIN);
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
    // Sort by temperature
    newPoints.sort((a, b) => a.temp - b.temp);
    // Find new index of moved point
    dragging = newPoints.findIndex(p => p.temp === temp && p.duty === duty);
    onchange(newPoints);
  }

  function onPointerUp() {
    dragging = null;
  }

  // Path data for the curve line
  const pathD = $derived(
    points
      .map((p, i) => `${i === 0 ? 'M' : 'L'} ${tempToX(p.temp)} ${dutyToY(p.duty)}`)
      .join(' ')
  );

  // Grid lines
  const tempGridLines = [30, 40, 50, 60, 70, 80, 90];
  const dutyGridLines = [20, 40, 60, 80, 100];
</script>

<svg
  bind:this={svg}
  viewBox="0 0 {PAD * 2 + SIZE} {PAD * 2 + SIZE}"
  class="curve-svg"
  role="application"
  aria-label="Fan curve editor"
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
>
  <!-- Grid -->
  {#each tempGridLines as t}
    <line
      x1={tempToX(t)} y1={PAD} x2={tempToX(t)} y2={PAD + SIZE}
      stroke="rgba(255,255,255,0.05)" stroke-width="0.5"
    />
    <text x={tempToX(t)} y={PAD + SIZE + 14} fill="rgba(255,255,255,0.3)"
      font-size="8" text-anchor="middle">{t}°</text>
  {/each}
  {#each dutyGridLines as d}
    <line
      x1={PAD} y1={dutyToY(d)} x2={PAD + SIZE} y2={dutyToY(d)}
      stroke="rgba(255,255,255,0.05)" stroke-width="0.5"
    />
    <text x={PAD - 4} y={dutyToY(d) + 3} fill="rgba(255,255,255,0.3)"
      font-size="8" text-anchor="end">{d}%</text>
  {/each}

  <!-- Current temperature indicator -->
  {#if currentTemp !== undefined}
    <line
      x1={tempToX(currentTemp)} y1={PAD} x2={tempToX(currentTemp)} y2={PAD + SIZE}
      stroke="var(--accent)" stroke-width="1" stroke-dasharray="4 4" opacity="0.5"
    />
  {/if}

  <!-- Curve fill -->
  <path
    d="{pathD} L {tempToX(points[points.length - 1]?.temp ?? TEMP_MAX)} {dutyToY(0)} L {tempToX(points[0]?.temp ?? TEMP_MIN)} {dutyToY(0)} Z"
    fill="rgba(96, 165, 250, 0.08)"
  />

  <!-- Curve line -->
  <path d={pathD} fill="none" stroke="var(--accent)" stroke-width="2" />

  <!-- Draggable handles -->
  {#each points as point, i}
    <circle
      cx={tempToX(point.temp)}
      cy={dutyToY(point.duty)}
      r="6"
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
    />
  {/each}
</svg>

<style>
  .curve-svg {
    width: 100%;
    height: 220px;
    display: block;
  }
</style>
