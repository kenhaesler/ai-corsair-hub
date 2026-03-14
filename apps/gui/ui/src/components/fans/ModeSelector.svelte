<script lang="ts">
  type Mode = 'fixed' | 'curve' | 'pid';

  interface Props {
    value: Mode;
    onchange: (mode: Mode) => void;
  }

  let { value, onchange }: Props = $props();

  const modes: { id: Mode; label: string }[] = [
    { id: 'fixed', label: 'Fixed' },
    { id: 'curve', label: 'Curve' },
    { id: 'pid', label: 'PID' },
  ];
</script>

<div class="mode-selector">
  {#each modes as mode}
    <button
      class="mode-btn"
      class:active={value === mode.id}
      onclick={() => onchange(mode.id)}
    >
      {mode.label}
    </button>
  {/each}
</div>

<style>
  .mode-selector {
    display: flex;
    background: var(--bg-input);
    border-radius: var(--radius);
    padding: 2px;
    gap: 2px;
  }
  .mode-btn {
    flex: 1;
    padding: 4px 12px;
    font-size: 11px;
    font-weight: 500;
    background: transparent;
    border: none;
    color: var(--text-secondary);
    cursor: pointer;
    border-radius: 6px;
    transition: all 0.15s ease;
  }
  .mode-btn:hover {
    color: var(--text-primary);
    background: transparent;
  }
  .mode-btn.active {
    background: var(--accent);
    color: #000;
  }
</style>
