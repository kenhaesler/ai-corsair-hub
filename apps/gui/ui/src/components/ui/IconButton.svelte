<!--
  IconButton: square icon-only button. `label` is required (aria-label + tooltip).
  Variants: plain (transparent until hover) / raised (material). `pressed` makes it a toggle.
-->
<script lang="ts">
  import type { HTMLButtonAttributes } from 'svelte/elements';
  import type { IconComponent } from '../../lib/icon';

  interface Props extends Omit<HTMLButtonAttributes, 'children'> {
    icon: IconComponent;
    label: string;
    variant?: 'plain' | 'raised';
    size?: 'sm' | 'md';
    pressed?: boolean | undefined;
  }
  let {
    icon: Icon,
    label,
    variant = 'plain',
    size = 'md',
    pressed = undefined,
    type = 'button',
    class: klass = '',
    ...rest
  }: Props = $props();
</script>

<button
  {type}
  class="ib {variant} {size} {klass}"
  class:on={pressed}
  aria-label={label}
  title={label}
  aria-pressed={pressed}
  {...rest}
>
  <Icon size={size === 'sm' ? 14 : 16} strokeWidth={2} />
</button>

<style>
  .ib {
    display: inline-grid;
    place-items: center;
    border: 0;
    border-radius: 8px;
    color: var(--t2);
    background: transparent;
    cursor: pointer;
    flex: none;
    transition:
      transform var(--dur-press) ease-out,
      background-color var(--dur-fast) ease-out,
      color var(--dur-fast) ease-out;
  }
  .md {
    width: 30px;
    height: 30px;
  }
  .sm {
    width: 24px;
    height: 24px;
    border-radius: 6px;
  }
  .raised {
    background: var(--mat-raised);
    box-shadow: inset 0 0 0 1px var(--hair);
  }
  .ib:hover:not(:disabled) {
    background: var(--mat-raised-hover);
    color: var(--t1);
  }
  .ib:active:not(:disabled) {
    transform: scale(0.94);
  }
  .ib:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .ib.on {
    background: var(--accent-soft);
    color: var(--accent-strong);
  }
</style>
