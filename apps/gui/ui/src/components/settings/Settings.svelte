<script lang="ts">
  import { configStore, loadConfig, saveCurrentConfig } from '../../lib/stores/config.svelte';
  import { check } from '@tauri-apps/plugin-updater';
  import { getVersion } from '@tauri-apps/api/app';
  import { onMount } from 'svelte';

  let updateStatus = $state<'idle' | 'checking' | 'available' | 'downloading' | 'upToDate' | 'error'>('idle');
  let updateVersion = $state('');
  let updateError = $state('');
  let updateObj = $state<Awaited<ReturnType<typeof check>> | null>(null);
  let appVersion = $state('');
  let downloadedBytes = $state(0);
  let downloadTotalBytes = $state(0);

  onMount(() => {
    loadConfig();
    getVersion()
      .then(v => appVersion = v)
      .catch(() => { /* Tauri API unavailable (e.g. dev-reload race) — leave blank */ });
  });

  function humanErr(e: unknown): string {
    if (e == null) return 'Unknown error';
    if (typeof e === 'string') return e;
    if (e instanceof Error) return e.message;
    // Tauri plugin errors are plain objects — stringify structured content
    try { return JSON.stringify(e); } catch { return String(e); }
  }

  function humanBytes(n: number): string {
    if (n <= 0) return '0 B';
    const units = ['B', 'KiB', 'MiB', 'GiB'];
    let i = 0;
    let v = n;
    while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
    return `${v.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
  }

  function updatePollInterval(value: number) {
    if (!configStore.config) return;
    configStore.config = { ...configStore.config, general: { ...configStore.config.general, poll_interval_ms: value } };
  }

  function updateLogLevel(value: string) {
    if (!configStore.config) return;
    configStore.config = { ...configStore.config, general: { ...configStore.config.general, log_level: value } };
  }

  async function checkForUpdates() {
    updateStatus = 'checking';
    updateError = '';
    try {
      const update = await check();
      if (update) {
        updateVersion = update.version;
        updateObj = update;
        updateStatus = 'available';
      } else {
        updateStatus = 'upToDate';
      }
    } catch (e) {
      updateError = humanErr(e);
      updateStatus = 'error';
    }
  }

  async function downloadAndInstall() {
    if (!updateObj) return;
    updateStatus = 'downloading';
    downloadedBytes = 0;
    downloadTotalBytes = 0;
    try {
      await updateObj.downloadAndInstall((event) => {
        switch (event.event) {
          case 'Started':
            downloadTotalBytes = event.data.contentLength ?? 0;
            break;
          case 'Progress':
            downloadedBytes += event.data.chunkLength;
            break;
          case 'Finished':
            // App is about to restart; leave state as 'downloading'
            break;
        }
      });
    } catch (e) {
      updateError = humanErr(e);
      updateStatus = 'error';
    }
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
    <h3 class="section-title">Updates</h3>
    <div class="update-section" aria-live="polite">
      {#if updateStatus === 'idle'}
        <button class="primary" onclick={checkForUpdates}>Check for Updates</button>
      {:else if updateStatus === 'checking'}
        <p class="update-text">Checking for updates...</p>
      {:else if updateStatus === 'upToDate'}
        <p class="update-text success">You're up to date!</p>
        <button onclick={checkForUpdates}>Check Again</button>
      {:else if updateStatus === 'available'}
        <p class="update-text">Update available: v{updateVersion}</p>
        <button class="primary" onclick={downloadAndInstall}>Download &amp; Install</button>
      {:else if updateStatus === 'downloading'}
        <p class="update-text">
          {#if downloadTotalBytes > 0}
            Downloading: {humanBytes(downloadedBytes)} / {humanBytes(downloadTotalBytes)}
            ({Math.min(100, Math.round((downloadedBytes / downloadTotalBytes) * 100))}%)
          {:else if downloadedBytes > 0}
            Downloading: {humanBytes(downloadedBytes)}
          {:else}
            Preparing download...
          {/if}
        </p>
        {#if downloadTotalBytes > 0}
          <div class="progress-track" role="progressbar"
               aria-valuemin="0" aria-valuemax={downloadTotalBytes} aria-valuenow={downloadedBytes}>
            <div class="progress-fill"
                 style="width: {Math.min(100, (downloadedBytes / downloadTotalBytes) * 100)}%"></div>
          </div>
        {/if}
        <p class="update-text" style="color: var(--text-muted); font-size: 11px">
          The app will restart automatically when installation completes.
        </p>
      {:else if updateStatus === 'error'}
        <p class="update-text error-text">{updateError}</p>
        <button onclick={checkForUpdates}>Try Again</button>
      {/if}
    </div>
  </section>

  <section class="card" style="margin-top: 12px">
    <h3 class="section-title">About</h3>
    <p class="about-text">Corsair Hub {appVersion ? `v${appVersion}` : ''}</p>
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
  .update-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .update-text {
    font-size: 12px;
    margin: 0;
    color: var(--text-secondary);
  }
  .update-text.success {
    color: var(--accent, #4ade80);
  }
  .update-text.error-text {
    color: var(--error, #f87171);
  }
  .progress-track {
    width: 100%;
    height: 6px;
    background: rgba(255, 255, 255, 0.08);
    border-radius: 3px;
    overflow: hidden;
  }
  .progress-fill {
    height: 100%;
    background: var(--accent, #4ade80);
    transition: width 120ms linear;
  }
  .about-text {
    font-size: 12px;
    margin: 0 0 4px 0;
    color: var(--text-secondary);
  }
</style>
