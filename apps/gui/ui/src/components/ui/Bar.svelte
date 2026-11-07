<!--
  Bar: horizontal bar that eases to its target on a spring (transform-only,
  compositor friendly). Exposed as role="meter".
  <Bar value={temp} max={100} color={thermalColor(temp)} label="CPU temperature" />
-->
<script lang="ts">
  import { followSpring, springs } from '../../lib/motion.svelte';

  interface Props {
    value: number;
    min?: number;
    max?: number;
    color?: string;
    height?: number;
    label: string;
    /** human-readable value for assistive tech */
    valueText?: string;
    glow?: boolean;
  }
  let {
    value,
    min = 0,
    max = 100,
    color = 'var(--accent)',
    height = 6,
    label,
    valueText,
    glow = false,
  }: Props = $props();

  const frac = $derived(Math.min(1, Math.max(0, (value - min) / Math.max(1e-9, max - min))));
  const s = followSpring(() => frac, springs.bar);
</script>

<div
  class="bar"
  style:height="{height}px"
  role="meter"
  aria-label={label}
  aria-valuemin={min}
  aria-valuemax={max}
  aria-valuenow={Math.round(value * 10) / 10}
  aria-valuetext={valueText}
>
  <div
    class="fill"
    class:glow
    style:transform="scaleX({Math.max(0, Math.min(1, s.current))})"
    style:background={color}
    style:--c={color}
  ></div>
</div>

<style>
  .bar {
    position: relative;
    width: 100%;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.08);
    overflow: hidden;
  }
  .fill {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    transform-origin: left center;
    will-change: transform;
    transition: background-color 600ms ease-out;
  }
  .fill.glow {
    box-shadow: 0 0 12px -2px var(--c);
  }
</style>
