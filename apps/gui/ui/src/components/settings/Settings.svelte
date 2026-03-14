<script lang="ts">
  import { configStore, loadConfig, saveCurrentConfig } from '../../lib/stores/config.svelte';
  import { applyPreset } from '../../lib/api';
  import { onMount } from 'svelte';

  onMount(() => { loadConfig(); });

  let presetStatus = $state<string | null>(null);

  async function doApplyPreset(name: string) {
    try {
      await applyPreset(name);
      await loadConfig(); // refresh Fan Control tab with new curves
      presetStatus = `Applied "${name}" curve preset`;
      setTimeout(() => { presetStatus = null; }, 3000);
    } catch (e) {
      presetStatus = `Error: ${e}`;
    }
  }

  function updatePollInterval(value: number) {
    if (!configStore.config) return;
    configStore.config = { ...configStore.config, general: { ...configStore.config.general, poll_interval_ms: value } };
  }

  function updateLogLevel(value: string) {
    if (!configStore.config) return;
    configStore.config = { ...configStore.config, general: { ...configStore.config.general, log_level: value } };
  }
</script>

<div class="settings scroll-area">
  {#if configStore.error}
    <div class="banner error">{configStore.error}</div>
  {/if}

  <section class="card">
    <h3 class="section-title">Quick Presets</h3>
    <p class="preset-hint">Adaptive fan curves optimized for custom water cooling</p>
    <div class="preset-row">
      <button class="preset-btn" onclick={() => doApplyPreset('silent')}>
        <span class="preset-name">Silent</span>
        <span class="preset-desc">20-30% idle, slow ramp</span>
      </button>
      <button class="preset-btn" onclick={() => doApplyPreset('balanced')}>
        <span class="preset-name">Balanced</span>
        <span class="preset-desc">25-55% daily, smooth</span>
      </button>
      <button class="preset-btn" onclick={() => doApplyPreset('performance')}>
        <span class="preset-name">Performance</span>
        <span class="preset-desc">30-70% aggressive, fast</span>
      </button>
    </div>
    {#if presetStatus}
      <p class="status">{presetStatus}</p>
    {/if}
  </section>

  {#if configStore.config}
    <section class="card" style="margin-top: 12px">
      <h3 class="section-title">General</h3>
      <div class="field">
        <span class="field-label">Poll Interval</span>
        <div class="input-row">
          <input
            type="range" min="100" max="5000" step="100"
            value={configStore.config.general.poll_interval_ms}
            oninput={(e) => updatePollInterval(Number(e.currentTarget.value))}
          />
          <span class="tabular-nums">{configStore.config.general.poll_interval_ms}ms</span>
        </div>
      </div>
      <div class="field">
        <span class="field-label">Log Level</span>
        <select
          value={configStore.config.general.log_level}
          onchange={(e) => updateLogLevel(e.currentTarget.value)}
        >
          <option value="error">Error</option>
          <option value="warn">Warn</option>
          <option value="info">Info</option>
          <option value="debug">Debug</option>
          <option value="trace">Trace</option>
        </select>
      </div>
      <div class="actions">
        <button class="primary" onclick={saveCurrentConfig} disabled={configStore.saving}>
          {configStore.saving ? 'Saving...' : 'Save Settings'}
        </button>
      </div>
    </section>
  {/if}

  <section class="card" style="margin-top: 12px">
    <h3 class="section-title">About</h3>
    <p class="about-text">ai-corsair-hub v0.1.0</p>
    <p class="about-text">Lightweight replacement for iCUE — smart fan control for custom water cooling</p>
    <p class="about-text" style="color: var(--text-muted)">Apache 2.0 License</p>
  </section>
</div>

<style>
  .settings {
    display: flex;
    flex-direction: column;
  }
  .section-title {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--text-secondary);
    margin: 0 0 12px 0;
  }
  .preset-hint {
    font-size: 11px;
    color: var(--text-muted);
    margin: 0 0 8px 0;
  }
  .preset-row {
    display: flex;
    gap: 8px;
  }
  .preset-btn {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    flex: 1;
    padding: 8px 12px;
  }
  .preset-name {
    font-weight: 600;
    font-size: 12px;
  }
  .preset-desc {
    font-size: 10px;
    color: var(--text-muted);
  }
  .status {
    font-size: 11px;
    color: var(--success);
    margin: 8px 0 0 0;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-bottom: 12px;
  }
  .field-label {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--text-muted);
  }
  .input-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .input-row span {
    width: 56px;
    text-align: right;
    font-size: 12px;
  }
  select {
    width: 120px;
  }
  .actions {
    margin-top: 8px;
    display: flex;
    justify-content: flex-end;
  }
  .about-text {
    font-size: 12px;
    margin: 0 0 4px 0;
    color: var(--text-secondary);
  }
</style>
