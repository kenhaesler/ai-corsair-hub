<!--
  Badge / Chip. Tones: neutral, accent, success, warn, error, info.
  `dot` adds a leading status dot; `dashed` draws a dashed outline.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { IconComponent } from '../../lib/icon';

  interface Props {
    tone?: 'neutral' | 'accent' | 'success' | 'warn' | 'error' | 'info';
    dot?: boolean;
    dashed?: boolean;
    icon?: IconComponent;
    size?: 'sm' | 'md';
    title?: string;
    children: Snippet;
  }
  let { tone = 'neutral', dot = false, dashed = false, icon: Icon, size = 'sm', title, children }: Props = $props();
</script>

<span class="badge {tone} {size}" class:dashed {title}>
  {#if dot}<span class="dot" aria-hidden="true"></span>{/if}
  {#if Icon}<Icon size={size === 'sm' ? 11 : 13} strokeWidth={2.25} />{/if}
  {@render children()}
</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    border-radius: var(--r-chip);
    font-weight: 600;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }
  .sm {
    height: 20px;
    padding: 0 7px;
    font-size: 11px;
  }
  .md {
    height: 24px;
    padding: 0 9px;
    font-size: 12px;
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: currentColor;
  }
  .neutral {
    background: rgba(255, 255, 255, 0.08);
    color: var(--t2);
  }
  .accent {
    background: var(--accent-soft);
    color: var(--accent-strong);
  }
  .success {
    background: var(--success-surface);
    color: var(--success-text);
  }
  .warn {
    background: var(--warn-surface);
    color: var(--warn-text);
  }
  .error {
    background: var(--error-surface);
    color: var(--error-text);
  }
  .info {
    background: var(--info-surface);
    color: var(--info-text);
  }
  .dashed {
    background: transparent;
    box-shadow: none;
    outline: 1px dashed currentColor;
    outline-offset: -1px;
  }
</style>
