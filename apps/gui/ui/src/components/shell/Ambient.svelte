<!--
  Ambient: Weather-style background light.
  - Cool (idle): glacier-blue/violet light pooling from the top, behind the
    large title, plus a dimmer violet counter-glow low on the opposite side.
  - Warm (load): the same geometry in muted amber / ember.
  The two sets cross-fade (equal-power) by the display's thermal state t,
  smoothed by a very slow spring (~20 s); while scrubbing it follows the past
  thermal state quickly. Only opacity and transform animate (compositor-only).
  Peak alpha stays low so text keeps AA contrast (backdrop luminance < 0.03).
  Grain overlay at 3% stops banding. Drift pauses when hidden and is off
  under reduced motion.
-->
<script lang="ts">
  import { Spring } from 'svelte/motion';
  import { display, scrub } from '../../lib/stores/scrub.svelte';
  import { pageVisibility, reducedMotion, springs } from '../../lib/motion.svelte';

  const t = new Spring(display.thermalT, { ...springs.ambient });

  $effect(() => {
    const target = display.thermalT;
    const preset = scrub.active ? springs.ambientScrub : springs.ambient;
    t.stiffness = preset.stiffness;
    t.damping = preset.damping;
    t.set(target, { instant: reducedMotion.current });
  });

  // Remap thermal t so idle (~45 °C) stays fully cool and sustained load
  // (~60 °C, t ≈ .5) already reads clearly amber.
  const smooth = (e0: number, e1: number, x: number) => {
    const k = Math.max(0, Math.min(1, (x - e0) / (e1 - e0)));
    return k * k * (3 - 2 * k);
  };
  const warm = $derived(smooth(0.2, 0.62, t.current));
  // equal-power cross-fade so the mid state is not dimmer than either end
  const coolA = $derived(Math.cos((warm * Math.PI) / 2));
  const warmA = $derived(Math.sin((warm * Math.PI) / 2));
  const paused = $derived(!pageVisibility.visible || reducedMotion.current);

  const grain =
    "url(\"data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='160' height='160'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='.9' numOctaves='2' stitchTiles='stitch'/><feColorMatrix values='0 0 0 0 1  0 0 0 0 1  0 0 0 0 1  0 0 0 .55 0'/></filter><rect width='100%' height='100%' filter='url(%23n)'/></svg>\")";
</script>

<div class="ambient" class:paused aria-hidden="true">
  <div class="set cool" style:opacity={coolA}>
    <div class="pool"></div>
    <div class="counter"></div>
  </div>
  <div class="set warm" style:opacity={warmA}>
    <div class="pool"></div>
    <div class="counter"></div>
  </div>
  <div class="vignette"></div>
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
  .set {
    position: absolute;
    inset: 0;
    will-change: opacity;
  }
  .pool,
  .counter {
    position: absolute;
    will-change: transform;
  }
  /* light pooling from the top, centred behind the large title */
  .pool {
    left: -10%;
    right: -10%;
    top: -38vh;
    height: 110vh;
    animation: drift-pool 70s ease-in-out infinite alternate;
  }
  /* dimmer counter-glow, low on the opposite side */
  .counter {
    right: -22vw;
    bottom: -40vh;
    width: 80vw;
    height: 90vh;
    animation: drift-counter 90s ease-in-out infinite alternate;
  }
  .cool .pool {
    background: radial-gradient(
      50% 50% at 42% 50%,
      hsl(222 78% 54% / 0.34) 0%,
      hsl(240 66% 50% / 0.2) 34%,
      hsl(258 60% 44% / 0.08) 62%,
      transparent 100%
    );
  }
  .cool .counter {
    background: radial-gradient(
      50% 50% at 50% 50%,
      hsl(258 58% 46% / 0.2) 0%,
      hsl(250 55% 40% / 0.08) 50%,
      transparent 100%
    );
  }
  .warm .pool {
    background: radial-gradient(
      50% 50% at 42% 50%,
      hsl(34 80% 50% / 0.3) 0%,
      hsl(26 74% 45% / 0.17) 34%,
      hsl(16 64% 40% / 0.07) 62%,
      transparent 100%
    );
  }
  .warm .counter {
    background: radial-gradient(
      50% 50% at 50% 50%,
      hsl(14 70% 42% / 0.2) 0%,
      hsl(20 60% 36% / 0.08) 50%,
      transparent 100%
    );
  }
  .paused .pool,
  .paused .counter {
    animation-play-state: paused;
  }
  @keyframes drift-pool {
    from {
      transform: translate3d(-3vw, 0, 0) scale(1);
    }
    to {
      transform: translate3d(4vw, 2vh, 0) scale(1.06);
    }
  }
  @keyframes drift-counter {
    from {
      transform: translate3d(0, 0, 0) scale(1.04);
    }
    to {
      transform: translate3d(-6vw, -4vh, 0) scale(0.96);
    }
  }
  .vignette {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.02) 0%, transparent 14%),
      radial-gradient(140% 90% at 50% 50%, transparent 55%, rgba(0, 0, 0, 0.35) 100%);
  }
  .grain {
    position: absolute;
    inset: 0;
    opacity: 0.03;
    background-size: 160px 160px;
    mix-blend-mode: overlay;
  }
  @media (prefers-reduced-motion: reduce) {
    .pool,
    .counter {
      animation: none !important;
    }
  }
</style>
