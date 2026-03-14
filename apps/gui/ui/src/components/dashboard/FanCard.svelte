<script lang="ts">
  import { Tween } from 'svelte/motion';
  import { cubicOut } from 'svelte/easing';

  interface Props {
    name: string;
    duty: number;
  }

  let { name, duty }: Props = $props();

  const tweened = new Tween(0, { duration: 400, easing: cubicOut });
  $effect(() => { tweened.target = duty; });
</script>

<div class="fan-card">
  <span class="name">{name}</span>
  <div class="bar-container">
    <div
      class="bar-fill"
      style="width: {tweened.current}%; background: var(--accent)"
    ></div>
  </div>
  <span class="value tabular-nums">{Math.round(tweened.current)}%</span>
</div>

<style>
  .fan-card {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .name {
    width: 100px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
    flex-shrink: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
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
    width: 36px;
    text-align: right;
    font-size: 13px;
    font-weight: 500;
    flex-shrink: 0;
  }
</style>
