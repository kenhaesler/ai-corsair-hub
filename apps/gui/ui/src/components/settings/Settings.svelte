<script lang="ts">
  import { configStore, loadConfig, saveCurrentConfig } from '../../lib/stores/config.svelte';
  import { onMount } from 'svelte';

  onMount(() => { loadConfig(); });

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

  {#if configStore.config}
    <section class="card">
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
