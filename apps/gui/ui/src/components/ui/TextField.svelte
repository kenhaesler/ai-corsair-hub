<!--
  TextField: single-line text input. `oncommit` fires on Enter and blur
  (Escape reverts to the value at focus). `mono` for ids/paths.
-->
<script lang="ts">
  import type { HTMLInputAttributes } from 'svelte/elements';

  interface Props extends Omit<HTMLInputAttributes, 'value' | 'size'> {
    value: string;
    label: string;
    showLabel?: boolean;
    mono?: boolean;
    size?: 'sm' | 'md';
    invalid?: boolean;
    oncommit?: (v: string) => void;
  }
  let {
    value = $bindable(),
    label,
    showLabel = false,
    mono = false,
    size = 'md',
    invalid = false,
    oncommit,
    id = `tf-${Math.random().toString(36).slice(2, 8)}`,
    class: klass = '',
    ...rest
  }: Props = $props();

  let atFocus = '';
</script>

<div class="tf {size} {klass}">
  {#if showLabel}<label class="lab" for={id}>{label}</label>{/if}
  <input
    {id}
    type="text"
    autocomplete="off"
    spellcheck="false"
    class:mono
    aria-label={showLabel ? undefined : label}
    aria-invalid={invalid || undefined}
    bind:value
    onfocus={() => (atFocus = value)}
    onblur={() => {
      if (value !== atFocus) oncommit?.(value);
    }}
    onkeydown={(e) => {
      if (e.key === 'Enter') {
        oncommit?.(value);
        atFocus = value;
      } else if (e.key === 'Escape') {
        value = atFocus;
        (e.currentTarget as HTMLInputElement).blur();
      }
    }}
    {...rest}
  />
</div>

<style>
  .tf {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .lab {
    font-size: 12px;
    font-weight: 500;
    color: var(--t2);
  }
  input {
    width: 100%;
    border: 0;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.22);
    box-shadow: inset 0 0 0 1px var(--hair-strong);
    color: var(--t1);
    transition: box-shadow var(--dur-fast) ease-out;
  }
  .md input {
    height: 30px;
    padding: 0 10px;
    font-size: 13px;
  }
  .sm input {
    height: 26px;
    padding: 0 8px;
    font-size: 12px;
  }
  input::placeholder {
    color: var(--t3);
  }
  input:hover:not(:disabled) {
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.22);
  }
  input:focus-visible {
    box-shadow:
      inset 0 0 0 1px var(--accent),
      var(--focus-ring);
  }
  input[aria-invalid='true'] {
    box-shadow: inset 0 0 0 1px var(--error);
  }
  input:disabled {
    opacity: 0.45;
  }
  .mono {
    font-family: var(--font-mono);
    font-size: 12px;
  }
</style>
