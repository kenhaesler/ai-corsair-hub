<!--
  AnimatedNumber: spring-interpolated, tabular number.
  <AnimatedNumber value={45.23} decimals={1} />°C
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import { followSpring, springs, type SpringPreset } from '../../lib/motion.svelte';

  interface Props {
    value: number;
    decimals?: number;
    preset?: SpringPreset;
    /** thousands separators */
    group?: boolean;
    class?: string;
  }
  let { value, decimals = 0, preset = springs.number, group = false, class: klass = '' }: Props = $props();

  // the spring preset is fixed at creation
  const s = followSpring(() => value, untrack(() => preset));
  const fmt = (v: number) =>
    group
      ? v.toLocaleString('en-US', { minimumFractionDigits: decimals, maximumFractionDigits: decimals })
      : v.toFixed(decimals);
  // Avoid "-0.0" flicker while springing through zero.
  const text = $derived(fmt(Math.abs(s.current) < 0.5 * 10 ** -decimals ? 0 : s.current));
</script>

<span class="an num {klass}">{text}</span>

<style>
  .an {
    font-variant-numeric: tabular-nums;
    font-feature-settings: 'cv11', 'ss01', 'tnum';
    white-space: nowrap;
  }
</style>
