<!--
  Ambient: the one light source of the "technical luminous" world.
  - Graphite canvas, hairline 120px columns (fading down), faint stars, grain.
  - A soft nebula low-left (and a dim counter-glow top-right) whose colour IS
    the system state: silver-blue at idle → violet under load → orange when hot,
    red in emergency. Driven by display.thermalT through a slow spring, so it
    follows the time-scrub too.
  - A few vertical light streaks fall slowly; their speed follows the mean fan
    duty (airflow you can see). Transform-only, paused when hidden, off under
    reduced motion.
  Publishes the state colour as --sc on :root for every component.
-->
<script lang="ts">
  import { Spring } from 'svelte/motion';
  import { display, scrub } from '../../lib/stores/scrub.svelte';
  import { sensors } from '../../lib/stores/sensors.svelte';
  import { pageVisibility, reducedMotion, springs } from '../../lib/motion.svelte';

  const t = new Spring(display.thermalT, { ...springs.ambient });
  $effect(() => {
    const target = sensors.snapshot?.emergency && !scrub.active ? 1 : display.thermalT;
    const preset = scrub.active ? springs.ambientScrub : springs.ambient;
    t.stiffness = preset.stiffness;
    t.damping = preset.damping;
    t.set(target, { instant: reducedMotion.current });
  });

  // silver-blue → violet → orange (OKLab mix in CSS keeps it smooth)
  const STOPS: [number, [number, number, number]][] = [
    [0, [169, 193, 230]],
    [0.5, [143, 134, 255]],
    [1, [255, 116, 72]],
  ];
  function mix(x: number): string {
    const k = Math.max(0, Math.min(1, x));
    const i = k <= 0.5 ? 0 : 1;
    const [a, ca] = STOPS[i];
    const [b, cb] = STOPS[i + 1];
    const f = (k - a) / (b - a);
    const c = ca.map((v, j) => Math.round(v + (cb[j] - v) * f));
    return `rgb(${c.join(' ')})`;
  }
  const sc = $derived(sensors.snapshot?.emergency && !scrub.active ? 'rgb(255 90 69)' : mix(t.current));
  $effect(() => {
    document.documentElement.style.setProperty('--sc', sc);
  });

  const meanDuty = $derived.by(() => {
    const g = display.snapshot?.group_duties ?? [];
    return g.length ? g.reduce((a, d) => a + d.duty_percent, 0) / g.length : 30;
  });
  // 12 s per fall at rest, 4.5 s at full airflow
  const fall = $derived(`${(12 - (Math.min(100, meanDuty) / 100) * 7.5).toFixed(1)}s`);
  const paused = $derived(!pageVisibility.visible || reducedMotion.current);

  const STREAKS = [
    { x: '21%', d: '-1s' },
    { x: '43%', d: '-4.6s' },
    { x: '50%', d: '-2.3s' },
    { x: '58.5%', d: '-6.1s' },
    { x: '79%', d: '-3.4s' },
    { x: '87.5%', d: '-7.7s' },
  ];
  const grain =
    "url(\"data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='200' height='200'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='.85' numOctaves='2' stitchTiles='stitch'/><feColorMatrix values='0 0 0 0 1  0 0 0 0 1  0 0 0 0 1  0 0 0 .6 0'/></filter><rect width='100%' height='100%' filter='url(%23n)'/></svg>\")";
</script>

<div class="ambient" class:paused aria-hidden="true" style:--fall={fall}>
  <div class="nebula main"></div>
  <div class="nebula counter"></div>
  <div class="stars"></div>
  <div class="columns"></div>
  <div class="streaks">
    {#each STREAKS as s (s.x)}
      <i style:left={s.x} style:animation-delay={s.d}></i>
    {/each}
  </div>
  <div class="grain" style:background-image={grain}></div>
</div>

<style>
  .ambient {
    position: fixed;
    inset: 0;
    z-index: 0;
    pointer-events: none;
    background: var(--bg-0);
    overflow: hidden;
  }
  .nebula {
    position: absolute;
    border-radius: 50%;
    filter: blur(30px);
    transition: background 1.2s ease;
  }
  .nebula.main {
    left: -18vw;
    bottom: -46vh;
    width: 78vw;
    height: 100vh;
    opacity: 0.55;
    background: radial-gradient(
      closest-side,
      color-mix(in oklab, var(--sc) 55%, transparent),
      color-mix(in oklab, var(--sc) 14%, transparent) 55%,
      transparent
    );
  }
  .nebula.counter {
    right: -30vw;
    top: -60vh;
    width: 64vw;
    height: 90vh;
    opacity: 0.18;
    background: radial-gradient(closest-side, color-mix(in oklab, var(--sc) 55%, transparent), transparent);
  }
  .stars {
    position: absolute;
    inset: 0;
    opacity: 0.65;
    background-image:
      radial-gradient(1px 1px at 12% 22%, #fff8, transparent),
      radial-gradient(1px 1px at 31% 64%, #fff5, transparent),
      radial-gradient(1px 1px at 58% 12%, #fff6, transparent),
      radial-gradient(1px 1px at 77% 38%, #fff4, transparent),
      radial-gradient(1px 1px at 88% 81%, #fff6, transparent),
      radial-gradient(1px 1px at 44% 88%, #fff4, transparent),
      radial-gradient(1px 1px at 6% 76%, #fff5, transparent),
      radial-gradient(1px 1px at 67% 70%, #fff3, transparent),
      radial-gradient(1.4px 1.4px at 93% 16%, #fff7, transparent),
      radial-gradient(1px 1px at 22% 46%, #fff3, transparent);
  }
  .columns {
    position: absolute;
    inset: 0;
    background: repeating-linear-gradient(90deg, transparent 0 119px, rgba(255, 255, 255, 0.03) 119px 120px);
    -webkit-mask: linear-gradient(180deg, #000, transparent 85%);
    mask: linear-gradient(180deg, #000, transparent 85%);
  }
  .streaks i {
    position: absolute;
    top: -30%;
    width: 1px;
    height: 240px;
    background: linear-gradient(180deg, transparent, color-mix(in oklab, var(--sc) 70%, #fff));
    opacity: 0.4;
    animation: fall var(--fall, 9s) linear infinite;
    will-change: transform;
  }
  .streaks i::after {
    content: '';
    position: absolute;
    bottom: -2px;
    left: -1px;
    width: 3px;
    height: 3px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 0 8px 2px color-mix(in oklab, var(--sc) 80%, #fff);
  }
  @keyframes fall {
    to {
      transform: translateY(140vh);
    }
  }
  .paused .streaks i {
    animation-play-state: paused;
  }
  .grain {
    position: absolute;
    inset: 0;
    opacity: 0.045;
    background-size: 200px 200px;
    mix-blend-mode: overlay;
  }
  @media (prefers-reduced-motion: reduce) {
    .streaks {
      display: none;
    }
  }
</style>
