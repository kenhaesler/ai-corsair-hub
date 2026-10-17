<script lang="ts">
  import type { FlowConfig, FlowDirection } from '../../lib/types';

  interface Props {
    flow: FlowConfig | null;
    deviceCount: number;
    onchange: (flow: FlowConfig | null) => void;
  }

  let { flow, deviceCount, onchange }: Props = $props();

  // Local drafts so the sliders respond snappily; we push up on release.
  // Defaulted values used when the user first enables flow from `null`.
  const DEFAULT_DELAY_MS = 80;
  const DEFAULT_DIRECTION: FlowDirection = 'Forward';

  let draftDelay = $state(flow?.delay_per_device_ms ?? DEFAULT_DELAY_MS);
  let draftDirection = $state<FlowDirection>(flow?.direction ?? DEFAULT_DIRECTION);

  // Re-sync the drafts whenever the parent hands us a new `flow` value
  // (e.g. after a preset apply, or when switching zones).
  $effect(() => {
    if (flow) {
      draftDelay = flow.delay_per_device_ms;
      draftDirection = flow.direction;
    }
  });

  const enabled = $derived(flow !== null);

  function toggle(on: boolean) {
    onchange(
      on
        ? { delay_per_device_ms: draftDelay, direction: draftDirection }
        : null,
    );
  }

  function commitDelay(v: number) {
    draftDelay = v;
    if (flow) onchange({ delay_per_device_ms: v, direction: draftDirection });
  }

  function commitDirection(v: FlowDirection) {
    draftDirection = v;
    if (flow) onchange({ delay_per_device_ms: draftDelay, direction: v });
  }

  // Total span (ms from first device to last) — useful so the user can
  // see at a glance whether the flow will feel perceptible or just like
  // a single-frame shimmer.
  const totalSpanMs = $derived.by(() => {
    if (!enabled || deviceCount <= 1) return 0;
    switch (draftDirection) {
      case 'Forward':
      case 'Reverse':
        return draftDelay * (deviceCount - 1);
      case 'CenterOut':
      case 'EdgeIn':
        return draftDelay * Math.floor((deviceCount - 1) / 2);
    }
  });

  const DIRECTIONS: { id: FlowDirection; label: string; hint: string }[] = [
    { id: 'Forward', label: 'Forward', hint: 'First device → last' },
    { id: 'Reverse', label: 'Reverse', hint: 'Last device → first' },
    { id: 'CenterOut', label: 'Center out', hint: 'Middle devices → edges' },
    { id: 'EdgeIn', label: 'Edge in', hint: 'Edges → middle devices' },
  ];
</script>

<section class="flow-editor">
  <header>
    <span class="section-title">Cross-Device Flow</span>
    <label class="toggle">
      <input
        type="checkbox"
        checked={enabled}
        onchange={(e) => toggle((e.currentTarget as HTMLInputElement).checked)}
      />
      <span>{enabled ? 'On' : 'Off'}</span>
    </label>
  </header>

  {#if !enabled}
    <p class="hint">
      Animate effects across the {deviceCount} device{deviceCount === 1 ? '' : 's'} in this zone with a staggered time offset — fire sweeps, rainbow cascades, etc.
    </p>
  {:else}
    <div class="row">
      <span class="field-label">Direction</span>
      <div class="segmented">
        {#each DIRECTIONS as dir}
          <button
            class="seg-btn"
            class:active={draftDirection === dir.id}
            title={dir.hint}
            onclick={() => commitDirection(dir.id)}
          >{dir.label}</button>
        {/each}
      </div>
    </div>

    <div class="row">
      <span class="field-label">Delay per device</span>
      <div class="slider-row">
        <input
          type="range"
          min="0"
          max="500"
          step="10"
          value={draftDelay}
          oninput={(e) => (draftDelay = Number((e.currentTarget as HTMLInputElement).value))}
          onchange={(e) => commitDelay(Number((e.currentTarget as HTMLInputElement).value))}
        />
        <span class="tabular-nums">{draftDelay} ms</span>
      </div>
    </div>

    {#if totalSpanMs > 0}
      <p class="meta">Total span: {totalSpanMs} ms across {deviceCount} devices</p>
    {/if}
  {/if}
</section>

<style>
  .flow-editor {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px;
    background: var(--bg-card-inset);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
  }
  .section-title {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 1px;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .toggle {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: var(--text-secondary);
    cursor: pointer;
  }
  .toggle input {
    accent-color: var(--accent);
  }
  .hint {
    margin: 0;
    font-size: 11px;
    line-height: 1.4;
    color: var(--text-muted);
  }
  .row {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .field-label {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--text-muted);
  }
  .segmented {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 4px;
  }
  .seg-btn {
    padding: 4px 6px;
    font-size: 11px;
    background: transparent;
    color: var(--text-secondary);
    border: 1px solid var(--border);
    border-radius: 4px;
    cursor: pointer;
    transition: border-color 0.1s, color 0.1s, background 0.1s;
  }
  .seg-btn:hover {
    color: var(--text-primary);
    border-color: var(--border-bright);
  }
  .seg-btn.active {
    background: var(--accent);
    color: var(--bg-base);
    border-color: var(--accent);
    font-weight: 600;
  }
  .slider-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .slider-row input[type='range'] {
    flex: 1;
    accent-color: var(--accent);
  }
  .slider-row span {
    width: 60px;
    text-align: right;
    font-size: 12px;
    font-weight: 500;
  }
  .meta {
    margin: 0;
    font-size: 10px;
    font-family: var(--font-mono);
    color: var(--text-muted);
  }
</style>
