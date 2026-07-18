<script lang="ts">
  // Expose acoustic-filter tuning in the UI: hysteresis (°C deadband)
  // plus asymmetric ramp rates. Asymmetric rates make fans feel quieter
  // to the ear than their rise suggests — ramp-down of 40% of ramp-up
  // is the historical default used when ramp_down_rate was absent.

  interface Props {
    hysteresis: number;
    rampUp: number;
    /** null = legacy symmetric behavior (ramp_rate * 0.4). */
    rampDown: number | null;
    onchange: (values: { hysteresis: number; rampUp: number; rampDown: number | null }) => void;
  }

  let { hysteresis, rampUp, rampDown, onchange }: Props = $props();

  // Defer the actual value commit until the slider is released —
  // `oninput` fires on every pixel. We mirror into local drafts so
  // the live label reads back correctly. Drafts are seeded from props
  // inside a $effect so the Svelte 5 reactivity graph tracks them.
  let draftHyst = $state(0);
  let draftUp = $state(5);
  let draftDown = $state(2);
  let asymmetric = $state(false);

  $effect(() => {
    draftHyst = hysteresis;
  });
  $effect(() => {
    draftUp = rampUp;
  });
  $effect(() => {
    if (rampDown != null) {
      draftDown = rampDown;
      asymmetric = true;
    } else {
      // Legacy: show what the backend will compute so the UI isn't
      // lying to the user.
      draftDown = Math.round(rampUp * 0.4 * 10) / 10;
      asymmetric = false;
    }
  });

  function commitHyst(v: number) {
    draftHyst = v;
    onchange({ hysteresis: v, rampUp: draftUp, rampDown: asymmetric ? draftDown : null });
  }
  function commitUp(v: number) {
    draftUp = v;
    onchange({ hysteresis: draftHyst, rampUp: v, rampDown: asymmetric ? draftDown : null });
  }
  function commitDown(v: number) {
    draftDown = v;
    onchange({ hysteresis: draftHyst, rampUp: draftUp, rampDown: v });
  }
  function toggleAsymmetric(e: Event) {
    const on = (e.currentTarget as HTMLInputElement).checked;
    asymmetric = on;
    onchange({
      hysteresis: draftHyst,
      rampUp: draftUp,
      rampDown: on ? draftDown : null,
    });
  }
</script>

<details class="noise-behavior">
  <summary>
    <span class="section-title">Noise Behavior</span>
    <span class="summary-meta">hy {draftHyst}° · ↑{draftUp}%/s · ↓{asymmetric ? draftDown : (Math.round(draftUp * 0.4 * 10) / 10)}%/s</span>
  </summary>

  <div class="controls">
    <div class="param">
      <span class="field-label">Hysteresis</span>
      <div class="slider-row">
        <input
          type="range"
          min="0"
          max="8"
          step="1"
          value={draftHyst}
          oninput={(e) => (draftHyst = Number(e.currentTarget.value))}
          onchange={(e) => commitHyst(Number(e.currentTarget.value))}
        />
        <span class="tabular-nums">{draftHyst}°C</span>
      </div>
      <p class="hint">Temperature changes smaller than this are ignored. Larger = quieter, smaller = faster response.</p>
    </div>

    <div class="param">
      <span class="field-label">Ramp Up</span>
      <div class="slider-row">
        <input
          type="range"
          min="1"
          max="20"
          step="1"
          value={draftUp}
          oninput={(e) => (draftUp = Number(e.currentTarget.value))}
          onchange={(e) => commitUp(Number(e.currentTarget.value))}
        />
        <span class="tabular-nums">{draftUp}%/s</span>
      </div>
      <p class="hint">How fast fans accelerate when temps climb.</p>
    </div>

    <div class="param">
      <label class="asymmetric-toggle">
        <input type="checkbox" checked={asymmetric} onchange={toggleAsymmetric} />
        <span>Asymmetric ramp-down</span>
      </label>
      <div class="slider-row" class:disabled={!asymmetric}>
        <input
          type="range"
          min="0.5"
          max="20"
          step="0.5"
          value={draftDown}
          disabled={!asymmetric}
          oninput={(e) => (draftDown = Number(e.currentTarget.value))}
          onchange={(e) => commitDown(Number(e.currentTarget.value))}
        />
        <span class="tabular-nums">{draftDown}%/s</span>
      </div>
      <p class="hint">
        {#if asymmetric}
          How fast fans decelerate. Typically slower than ramp-up so fall-off is inaudible.
        {:else}
          Symmetric mode: backend uses ramp-up × 0.4 automatically. Enable to tune independently.
        {/if}
      </p>
    </div>
  </div>
</details>

<style>
  .noise-behavior {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-card-inset);
    margin-top: 12px;
  }
  .noise-behavior > summary {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    padding: 8px 12px;
    cursor: pointer;
    user-select: none;
  }
  .noise-behavior > summary:hover {
    background: var(--bg-card-hover);
  }
  .noise-behavior[open] > summary {
    border-bottom: 1px solid var(--border);
  }
  .section-title {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 1px;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .summary-meta {
    font-size: 10px;
    font-family: var(--font-mono);
    color: var(--text-muted);
  }
  .controls {
    padding: 12px;
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 16px;
  }
  .field-label {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--text-muted);
  }
  .slider-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 4px;
  }
  .slider-row.disabled {
    opacity: 0.5;
  }
  .slider-row span {
    width: 56px;
    text-align: right;
    font-size: 13px;
    font-weight: 500;
    font-variant-numeric: tabular-nums;
  }
  .slider-row input[type='range'] {
    flex: 1;
    accent-color: var(--accent);
  }
  .hint {
    margin: 4px 0 0;
    font-size: 10px;
    color: var(--text-muted);
    line-height: 1.3;
  }
  .asymmetric-toggle {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--text-muted);
    cursor: pointer;
  }
  .asymmetric-toggle input {
    accent-color: var(--accent);
  }
</style>
