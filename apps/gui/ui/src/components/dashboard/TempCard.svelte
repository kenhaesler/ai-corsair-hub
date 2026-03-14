<script lang="ts">
  import { Tween } from 'svelte/motion';
  import { cubicOut } from 'svelte/easing';

  interface Props {
    label: string;
    value: number;
    max?: number;
  }

  let { label, value, max = 100 }: Props = $props();

  const tweened = new Tween(0, { duration: 400, easing: cubicOut });
  $effect(() => { tweened.target = value; });

  function tempColor(temp: number): string {
    if (temp < 40) return 'var(--temp-cold)';
    if (temp < 60) return `color-mix(in srgb, var(--temp-cold) ${Math.round((60 - temp) / 20 * 100)}%, var(--temp-warm))`;
    if (temp < 80) return `color-mix(in srgb, var(--temp-warm) ${Math.round((80 - temp) / 20 * 100)}%, var(--temp-hot))`;
    return 'var(--temp-hot)';
  }
</script>

<div class="temp-card">
  <span class="label">{label}</span>
  <div class="bar-container">
    <div
      class="bar-fill"
      style="width: {Math.min((tweened.current / max) * 100, 100)}%; background: {tempColor(tweened.current)}"
    ></div>
  </div>
  <span class="value tabular-nums">{tweened.current.toFixed(1)}°C</span>
</div>

<style>
  .temp-card {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .label {
    width: 64px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
    flex-shrink: 0;
  }
  .bar-container {
    flex: 1;
    height: 6px;
    background: rgba(255, 255, 255, 0.06);
    border-radius: 3px;
    overflow: hidden;
  }
  .bar-fill {
    height: 100%;
    border-radius: 3px;
    transition: width 0.4s cubic-bezier(0.25, 0.1, 0.25, 1);
  }
  .value {
    width: 56px;
    text-align: right;
    font-size: 13px;
    font-weight: 500;
    flex-shrink: 0;
  }
</style>
