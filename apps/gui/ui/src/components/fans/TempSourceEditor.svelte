<script lang="ts">
  import type { TempSourceConfig } from '../../lib/types';

  interface Props {
    source: TempSourceConfig;
    availableSensors: string[];
    onchange: (source: TempSourceConfig) => void;
  }

  let { source, availableSensors, onchange }: Props = $props();

  // Friendly labels for sensor identifiers the control loop uses.
  // Falls through to the raw id so a future sensor (water_inline, mb_t1)
  // shows up without a UI edit.
  const FRIENDLY: Record<string, string> = {
    cpu: 'CPU',
    gpu: 'GPU',
    psu_vrm: 'PSU VRM',
    psu_case: 'PSU Case',
    water_inline: 'Water',
  };

  function label(id: string): string {
    return FRIENDLY[id] ?? id;
  }

  // Union of currently-selected sensors and the available list, with
  // availability annotated — an orphan reference (sensor no longer
  // available) gets shown as disabled so the user sees it exists and
  // can remove it.
  interface Row {
    id: string;
    selected: boolean;
    weight: number;
    available: boolean;
  }

  const rows = $derived.by<Row[]>(() => {
    const out: Row[] = [];
    const seen = new Set<string>();
    // First: the sensors currently referenced by this group, in config order.
    for (let i = 0; i < source.sensors.length; i++) {
      const id = source.sensors[i];
      seen.add(id);
      out.push({
        id,
        selected: true,
        weight: source.weights[i] ?? 1,
        available: availableSensors.includes(id),
      });
    }
    // Then: sensors the machine has but this group isn't using.
    for (const id of availableSensors) {
      if (!seen.has(id)) {
        out.push({ id, selected: false, weight: 0, available: true });
      }
    }
    return out;
  });

  // Sum of active weights — shown in the header so the user sees the
  // normalization the backend will apply.
  const activeWeightSum = $derived(
    rows.filter((r) => r.selected).reduce((s, r) => s + r.weight, 0),
  );

  function toggle(id: string) {
    const idx = source.sensors.indexOf(id);
    if (idx >= 0) {
      // Don't let the user strip the last sensor — the backend rejects
      // empty temp sources (validate_temp_source).
      if (source.sensors.length <= 1) return;
      const sensors = source.sensors.filter((_, i) => i !== idx);
      const weights = source.weights.filter((_, i) => i !== idx);
      onchange({ sensors, weights });
    } else {
      onchange({
        sensors: [...source.sensors, id],
        weights: [...source.weights, 1],
      });
    }
  }

  function setWeight(id: string, weight: number) {
    const idx = source.sensors.indexOf(id);
    if (idx < 0) return;
    const weights = [...source.weights];
    weights[idx] = weight;
    onchange({ ...source, weights });
  }
</script>

<section class="temp-source">
  <header>
    <span class="section-title">Sensor Mix</span>
    <span class="meta">
      {rows.filter((r) => r.selected).length} active · Σ {activeWeightSum.toFixed(2)}
    </span>
  </header>

  <div class="chips">
    {#each rows as row}
      <button
        type="button"
        class="chip"
        class:active={row.selected}
        class:unavailable={!row.available}
        title={row.available
          ? row.selected
            ? `Weight ${row.weight.toFixed(2)} — click to remove`
            : `Click to include ${label(row.id)}`
          : `${label(row.id)} is referenced but not currently available`}
        onclick={() => toggle(row.id)}
      >
        <span class="chip-label">{label(row.id)}</span>
        {#if row.selected}
          <span class="chip-weight tabular-nums">{row.weight.toFixed(2)}</span>
        {/if}
      </button>
    {/each}
  </div>

  {#if rows.some((r) => r.selected)}
    <div class="sliders">
      {#each rows.filter((r) => r.selected) as row}
        <label class="slider-row">
          <span class="slider-label">{label(row.id)}</span>
          <input
            type="range"
            min="0"
            max="2"
            step="0.05"
            value={row.weight}
            oninput={(e) =>
              setWeight(row.id, Number((e.currentTarget as HTMLInputElement).value))}
          />
          <span class="slider-value tabular-nums">{row.weight.toFixed(2)}</span>
        </label>
      {/each}
    </div>
  {/if}

  <p class="hint">
    Raw weights; the control loop normalizes to a weighted mean.
    A radiator group often weights GPU higher; chassis fans, CPU.
  </p>
</section>

<style>
  .temp-source {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px;
    background: var(--bg-card-inset);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    margin-top: 12px;
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
  .meta {
    font-size: 10px;
    font-family: var(--font-mono);
    color: var(--text-muted);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    font-size: 11px;
    background: transparent;
    color: var(--text-secondary);
    border: 1px solid var(--border);
    border-radius: 14px;
    cursor: pointer;
    transition: background 0.1s, border-color 0.1s, color 0.1s;
  }
  .chip:hover:not(.unavailable) {
    color: var(--text-primary);
    border-color: var(--border-bright);
  }
  .chip.active {
    background: var(--bg-card-active);
    color: var(--accent);
    border-color: var(--accent-dim);
  }
  .chip.unavailable {
    opacity: 0.5;
    border-style: dashed;
  }
  .chip-label {
    font-weight: 600;
  }
  .chip-weight {
    font-size: 10px;
    font-family: var(--font-mono);
    color: var(--text-muted);
  }
  .sliders {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .slider-row {
    display: grid;
    grid-template-columns: 64px 1fr 44px;
    align-items: center;
    gap: 8px;
  }
  .slider-label {
    font-size: 11px;
    color: var(--text-secondary);
  }
  .slider-row input[type='range'] {
    accent-color: var(--accent);
  }
  .slider-value {
    text-align: right;
    font-size: 11px;
  }
  .hint {
    margin: 0;
    font-size: 10px;
    color: var(--text-muted);
    line-height: 1.4;
  }
</style>
