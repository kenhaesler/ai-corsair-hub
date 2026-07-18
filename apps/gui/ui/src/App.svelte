<script lang="ts">
  import { onMount } from 'svelte';
  import {
    disposeSensorListener,
    initSensorListener,
    sensors,
  } from './lib/stores/sensors.svelte';
  import {
    disposeRgbListener,
    disposeRgbSaveQueue,
    initRgbListener,
    initializeRgbConfig,
    rgb,
  } from './lib/stores/rgb.svelte';
  import { configStore, loadConfig } from './lib/stores/config.svelte';
  import Titlebar from './components/layout/Titlebar.svelte';
  import TabBar from './components/layout/TabBar.svelte';
  import Dashboard from './components/dashboard/Dashboard.svelte';
  import FanGroups from './components/fans/FanGroups.svelte';
  import DevicePanel from './components/devices/DevicePanel.svelte';
  import Lighting from './components/lighting/Lighting.svelte';
  import Settings from './components/settings/Settings.svelte';

  type Tab = 'dashboard' | 'fans' | 'devices' | 'lighting' | 'settings';
  let activeTab = $state<Tab>('dashboard');
  let retryTimer: ReturnType<typeof setTimeout> | null = null;
  let stopped = false;

  async function connectListeners() {
    const results = await Promise.allSettled([
      initSensorListener(),
      initRgbListener(),
    ]);
    if (!stopped && results.some((result) => result.status === 'rejected')) {
      retryTimer = setTimeout(() => {
        retryTimer = null;
        void connectListeners();
      }, 2000);
    }
  }

  async function initializeConfig() {
    const config = await loadConfig();
    if (config) initializeRgbConfig(config.rgb);
  }

  onMount(() => {
    void initializeConfig();
    stopped = false;
    void connectListeners();

    return () => {
      stopped = true;
      if (retryTimer) clearTimeout(retryTimer);
      retryTimer = null;
      disposeSensorListener();
      disposeRgbListener();
      disposeRgbSaveQueue();
    };
  });
</script>

<Titlebar />
<TabBar active={activeTab} onchange={(tab) => { activeTab = tab; }} />

<main class="content">
  {#if configStore.error && !configStore.config}
    <div class="banner error app-banner">
      <span>Configuration could not be loaded: {configStore.error}</span>
      <button onclick={initializeConfig} disabled={configStore.loading}>
        {configStore.loading ? 'Retrying...' : 'Retry'}
      </button>
    </div>
  {/if}
  {#if sensors.listenerError || rgb.listenerError}
    <div class="banner warning app-banner" aria-live="polite">
      Live updates disconnected. Retrying...
    </div>
  {/if}

  <div class="page" hidden={activeTab !== 'dashboard'}><Dashboard /></div>
  <div class="page" hidden={activeTab !== 'fans'}><FanGroups /></div>
  <div class="page" hidden={activeTab !== 'devices'}><DevicePanel /></div>
  <div class="page" hidden={activeTab !== 'lighting'}><Lighting /></div>
  <div class="page" hidden={activeTab !== 'settings'}><Settings /></div>
</main>

<style>
  .content {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
  .page {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: fadeIn 0.15s ease;
  }
  .page[hidden] {
    display: none;
  }
  .app-banner {
    margin: 8px 16px 0;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .app-banner button {
    padding: 3px 10px;
  }
</style>
