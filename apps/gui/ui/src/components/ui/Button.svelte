<!--
  Button. Variants: primary (accent fill), secondary (raised material),
  plain (accent text), destructive (red fill). Sizes sm (26px) / md (32px).
  Press scales to .97; `loading` shows a spinner and disables.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';
  import type { IconComponent } from '../../lib/icon';
  import Spinner from './Spinner.svelte';

  interface Props extends Omit<HTMLButtonAttributes, 'children'> {
    variant?: 'primary' | 'secondary' | 'plain' | 'destructive';
    size?: 'sm' | 'md';
    icon?: IconComponent;
    loading?: boolean;
    /** stretch to fill the container width */
    block?: boolean;
    children?: Snippet;
  }

  let {
    variant = 'secondary',
    size = 'md',
    icon: Icon,
    loading = false,
    block = false,
    disabled = false,
    type = 'button',
    children,
    class: klass = '',
    ...rest
  }: Props = $props();
</script>

<button
  {type}
  class="btn {variant} {size} {klass}"
  class:block
  class:loading
  disabled={disabled || loading}
  aria-busy={loading || undefined}
  {...rest}
>
  {#if loading}
    <Spinner size={size === 'sm' ? 12 : 14} />
  {:else if Icon}
    <Icon size={size === 'sm' ? 14 : 16} strokeWidth={2} />
  {/if}
  {#if children}<span class="label">{@render children()}</span>{/if}
</button>

<style>
  .btn {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    border: 0;
    border-radius: var(--r-control);
    font-weight: 600;
    letter-spacing: -0.003em;
    white-space: nowrap;
    cursor: pointer;
    user-select: none;
    transition:
      transform var(--dur-press) ease-out,
      background-color var(--dur-fast) ease-out,
      box-shadow var(--dur-fast) ease-out,
      color var(--dur-fast) ease-out,
      opacity var(--dur-fast) ease-out;
  }
  .md {
    height: 32px;
    padding: 0 14px;
    font-size: 13px;
  }
  .sm {
    height: 26px;
    padding: 0 10px;
    font-size: 12px;
    border-radius: 8px;
  }
  .block {
    width: 100%;
  }
  .btn:active:not(:disabled) {
    transform: scale(0.97);
  }
  .btn:focus-visible {
    box-shadow: var(--focus-ring);
  }
  .btn:disabled {
    cursor: default;
    opacity: 0.42;
  }
  .btn.loading:disabled {
    opacity: 0.75;
  }

  .primary {
    background: var(--accent);
    color: var(--accent-ink);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.28),
      0 1px 2px rgb(0 0 0 / 0.35),
      0 6px 16px -8px rgba(124, 156, 255, 0.55);
  }
  .primary:hover:not(:disabled) {
    background: var(--accent-strong);
  }
  .primary:focus-visible {
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.28),
      var(--focus-ring);
  }

  .secondary {
    background: var(--mat-raised);
    color: var(--t1);
    box-shadow:
      inset 0 0 0 1px var(--hair),
      var(--hair-inner);
  }
  .secondary:hover:not(:disabled) {
    background: var(--mat-raised-hover);
  }
  .secondary:active:not(:disabled) {
    background: var(--mat-pressed);
  }
  .secondary:focus-visible {
    box-shadow:
      inset 0 0 0 1px var(--hair-strong),
      var(--focus-ring);
  }

  .plain {
    background: transparent;
    color: var(--accent-strong);
  }
  .plain.md {
    padding: 0 10px;
  }
  .plain.sm {
    padding: 0 8px;
  }
  .plain:hover:not(:disabled) {
    background: var(--accent-softer);
  }

  .destructive {
    background: #d93a30;
    color: #fff;
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.2),
      0 1px 2px rgb(0 0 0 / 0.35),
      0 6px 16px -8px rgba(255, 69, 58, 0.5);
  }
  .destructive:hover:not(:disabled) {
    background: #c9342b;
  }
  .destructive:focus-visible {
    box-shadow: 0 0 0 3px rgba(255, 69, 58, 0.45);
  }
  .label {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
</style>
