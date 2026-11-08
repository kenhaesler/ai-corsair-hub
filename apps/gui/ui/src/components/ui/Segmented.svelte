<!--
  Segmented control with a sliding pill on a spring. Keyboard: ←/→ (and
  Home/End) move the selection, as a radiogroup.

  <Segmented options={[{value: 60, label: '1 min'}, ...]} bind:value label="Range" />
-->
<script lang="ts" generics="T extends string | number">
  import { onMount } from 'svelte';
  import type { IconComponent } from '../../lib/icon';
  import { createSpring, springTo, springs } from '../../lib/motion.svelte';

  interface Option {
    value: T;
    label: string;
    icon?: IconComponent;
    disabled?: boolean;
  }
  interface Props {
    options: Option[];
    value: T;
    /** accessible name of the group */
    label: string;
    size?: 'sm' | 'md';
    onchange?: (v: T) => void;
    disabled?: boolean;
  }
  let { options, value = $bindable(), label, size = 'md', onchange, disabled = false }: Props = $props();

  let root: HTMLDivElement;
  const buttons = $state<HTMLButtonElement[]>([]);
  const pill = createSpring({ x: 0, w: 0 }, springs.tab);
  let ready = $state(false);

  function measure(instant = false) {
    const i = options.findIndex((o) => o.value === value);
    const b = buttons[i];
    if (!b || !root) return;
    springTo(pill, { x: b.offsetLeft, w: b.offsetWidth }, instant || !ready);
    ready = true;
  }

  $effect(() => {
    void value;
    void options.length;
    measure();
  });

  onMount(() => {
    measure(true);
    const ro = new ResizeObserver(() => measure(true));
    ro.observe(root);
    return () => ro.disconnect();
  });

  function select(v: T) {
    if (v === value || disabled) return;
    value = v;
    onchange?.(v);
  }

  function onKey(e: KeyboardEvent) {
    const enabled = options.filter((o) => !o.disabled);
    const i = enabled.findIndex((o) => o.value === value);
    let j = i;
    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') j = Math.min(enabled.length - 1, i + 1);
    else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') j = Math.max(0, i - 1);
    else if (e.key === 'Home') j = 0;
    else if (e.key === 'End') j = enabled.length - 1;
    else return;
    e.preventDefault();
    select(enabled[j].value);
    const idx = options.indexOf(enabled[j]);
    buttons[idx]?.focus();
  }
</script>

<div
  class="seg {size}"
  class:disabled
  role="radiogroup"
  aria-label={label}
  tabindex="-1"
  bind:this={root}
  onkeydown={onKey}
>
  <span
    class="pill"
    class:ready
    style:transform="translateX({pill.current.x}px)"
    style:width="{pill.current.w}px"
    aria-hidden="true"
  ></span>
  {#each options as o, i (o.value)}
    {@const Icon = o.icon}
    <button
      type="button"
      role="radio"
      aria-checked={o.value === value}
      tabindex={o.value === value || (i === 0 && !options.some((x) => x.value === value)) ? 0 : -1}
      class="opt"
      class:sel={o.value === value}
      disabled={disabled || o.disabled}
      bind:this={buttons[i]}
      onclick={() => select(o.value)}
    >
      {#if Icon}<Icon size={14} strokeWidth={2} />{/if}
      <span>{o.label}</span>
    </button>
  {/each}
</div>

<style>
  .seg {
    position: relative;
    display: inline-flex;
    padding: 2px;
    border-radius: 9px;
    background: rgba(255, 255, 255, 0.06);
    box-shadow: inset 0 0 0 1px var(--hair);
    isolation: isolate;
  }
  .pill {
    position: absolute;
    top: 2px;
    bottom: 2px;
    left: 0;
    border-radius: 7px;
    background: rgba(255, 255, 255, 0.14);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.08),
      0 1px 2px rgb(0 0 0 / 0.4),
      0 2px 8px -2px rgb(0 0 0 / 0.4);
    z-index: -1;
    opacity: 0;
  }
  .pill.ready {
    opacity: 1;
  }
  .opt {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    border: 0;
    background: transparent;
    color: var(--t2);
    font-weight: 500;
    border-radius: 7px;
    cursor: pointer;
    white-space: nowrap;
    transition: color var(--dur-fast) ease-out;
  }
  .md .opt {
    height: 26px;
    padding: 0 12px;
    font-size: 12.5px;
  }
  .sm .opt {
    height: 22px;
    padding: 0 9px;
    font-size: 12px;
  }
  .opt:hover:not(:disabled) {
    color: var(--t1);
  }
  .opt.sel {
    color: var(--t1);
    font-weight: 600;
  }
  .opt:focus-visible {
    box-shadow: var(--focus-ring);
  }
  .opt:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .disabled {
    opacity: 0.6;
  }
</style>
