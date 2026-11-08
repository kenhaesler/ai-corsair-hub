<!--
  Toggle (switch). role="switch"; Space/Enter toggle. Provide `label` for the
  accessible name; set `showLabel` to render it visibly to the left.
-->
<script lang="ts">
  interface Props {
    checked: boolean;
    label: string;
    showLabel?: boolean;
    disabled?: boolean;
    size?: 'sm' | 'md';
    onchange?: (v: boolean) => void;
  }
  let { checked = $bindable(), label, showLabel = false, disabled = false, size = 'md', onchange }: Props = $props();

  function toggle() {
    if (disabled) return;
    checked = !checked;
    onchange?.(checked);
  }
</script>

<button
  type="button"
  role="switch"
  aria-checked={checked}
  aria-label={showLabel ? undefined : label}
  class="tg {size}"
  class:on={checked}
  {disabled}
  onclick={toggle}
>
  {#if showLabel}<span class="text">{label}</span>{/if}
  <span class="track" aria-hidden="true"><span class="knob"></span></span>
</button>

<style>
  .tg {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    border: 0;
    padding: 0;
    background: transparent;
    cursor: pointer;
    border-radius: 999px;
    color: var(--t1);
    font-size: 13px;
  }
  .tg:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .text {
    font-weight: 500;
  }
  .track {
    position: relative;
    display: block;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.16);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.06);
    transition: background-color 220ms var(--ease-out-expo);
  }
  .md .track {
    width: 38px;
    height: 22px;
  }
  .sm .track {
    width: 30px;
    height: 18px;
  }
  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    border-radius: 50%;
    background: #fff;
    box-shadow: var(--sh-knob);
    transition: transform 260ms var(--ease-out-expo);
  }
  .md .knob {
    width: 18px;
    height: 18px;
  }
  .sm .knob {
    width: 14px;
    height: 14px;
  }
  .on .track {
    background: var(--accent);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.2);
  }
  .on .knob {
    background: var(--accent-ink);
    box-shadow: 0 1px 2px rgb(0 0 0 / 0.5);
  }
  .md.on .knob {
    transform: translateX(16px);
  }
  .sm.on .knob {
    transform: translateX(12px);
  }
  .tg:active:not(:disabled) .track {
    filter: brightness(1.15);
  }
  .tg:focus-visible {
    box-shadow: none;
  }
  .tg:focus-visible .track {
    box-shadow: var(--focus-ring);
  }
  @media (prefers-reduced-motion: reduce) {
    .knob {
      transition-duration: 1ms;
    }
  }
</style>
