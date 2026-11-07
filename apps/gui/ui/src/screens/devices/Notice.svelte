<!--
  Live notice pill (inventory 3.2). Rendered inside an aria-live region by
  the Devices screen; the region itself is the announcer, so this carries no
  role of its own.
-->
<script lang="ts">
  import CircleCheck from 'lucide-svelte/icons/circle-check';
  import TriangleAlert from 'lucide-svelte/icons/triangle-alert';
  import Info from 'lucide-svelte/icons/info';

  interface Props {
    text: string;
    tone: 'info' | 'success' | 'warning';
  }
  let { text, tone }: Props = $props();
  const Icon = $derived(tone === 'success' ? CircleCheck : tone === 'warning' ? TriangleAlert : Info);
</script>

<div class="notice {tone}">
  <span class="ic" aria-hidden="true"><Icon size={16} strokeWidth={2.25} /></span>
  <span class="tx">{text}</span>
</div>

<style>
  .notice {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 40px;
    padding: 8px 14px;
    border-radius: var(--r-row);
    font-size: 13px;
    line-height: 18px;
    font-weight: 500;
    box-shadow: var(--sh-1);
  }
  .ic {
    display: grid;
    flex: none;
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
</style>
