<!--
  Banner: inline alert row (role="alert"). Tones warning / error / success /
  info. Optional `title` (bold lead), `actions` snippet, `ondismiss`.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import TriangleAlert from 'lucide-svelte/icons/triangle-alert';
  import CircleAlert from 'lucide-svelte/icons/circle-alert';
  import CircleCheck from 'lucide-svelte/icons/circle-check';
  import Info from 'lucide-svelte/icons/info';
  import X from 'lucide-svelte/icons/x';

  interface Props {
    tone?: 'warning' | 'error' | 'success' | 'info';
    title?: string;
    actions?: Snippet;
    ondismiss?: () => void;
    dismissLabel?: string;
    children: Snippet;
  }
  let { tone = 'warning', title, actions, ondismiss, dismissLabel = 'Dismiss', children }: Props = $props();
  const Icon = $derived(
    tone === 'error' ? CircleAlert : tone === 'success' ? CircleCheck : tone === 'info' ? Info : TriangleAlert,
  );
</script>

<div class="banner {tone}" role="alert">
  <span class="ic" aria-hidden="true"><Icon size={16} strokeWidth={2.25} /></span>
  <div class="text">
    {#if title}<strong class="title">{title}</strong>{/if}
    {@render children()}
  </div>
  {#if actions}<div class="actions">{@render actions()}</div>{/if}
  {#if ondismiss}
    <button type="button" class="x" aria-label={dismissLabel} title={dismissLabel} onclick={ondismiss}>
      <X size={14} strokeWidth={2.25} />
    </button>
  {/if}
</div>

<style>
  .banner {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 12px 14px;
    border-radius: var(--r-row);
    font-size: 13px;
    line-height: 19px;
    box-shadow: var(--sh-1);
  }
  .ic {
    flex: none;
    display: grid;
    place-items: center;
    height: 19px;
  }
  .text {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .title {
    font-weight: 600;
    margin-right: 4px;
  }
  .actions {
    flex: none;
    display: flex;
    gap: 8px;
    align-items: center;
    margin: -4px 0;
  }
  .x {
    flex: none;
    width: 24px;
    height: 24px;
    margin: -3px -4px -3px 0;
    display: grid;
    place-items: center;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: inherit;
    opacity: 0.8;
    cursor: pointer;
  }
  .x:hover {
    opacity: 1;
    background: rgba(255, 255, 255, 0.08);
  }
  .warning {
    background: var(--warn-surface);
    box-shadow:
      inset 0 0 0 1px var(--warn-hair),
      var(--sh-1);
    color: var(--warn-text);
  }
  .warning .ic {
    color: var(--warn);
  }
  .error {
    background: var(--error-surface);
    box-shadow:
      inset 0 0 0 1px var(--error-hair),
      var(--sh-1);
    color: var(--error-text);
  }
  .error .ic {
    color: var(--error);
  }
  .success {
    background: var(--success-surface);
    box-shadow:
      inset 0 0 0 1px var(--success-hair),
      var(--sh-1);
    color: var(--success-text);
  }
  .success .ic {
    color: var(--success);
  }
  .info {
    background: var(--info-surface);
    box-shadow:
      inset 0 0 0 1px var(--info-hair),
      var(--sh-1);
    color: var(--info-text);
  }
  .info .ic {
    color: var(--accent);
  }
  .banner :global(code) {
    font-size: 12px;
    padding: 1px 5px;
    border-radius: 4px;
    background: rgba(0, 0, 0, 0.25);
    color: inherit;
  }
</style>
