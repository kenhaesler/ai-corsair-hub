<script lang="ts">
  import { onMount } from 'svelte';
  import { initSensorListener } from './lib/stores/sensors.svelte';
  import { initRgbListener } from './lib/stores/rgb.svelte';
  import Titlebar from './components/layout/Titlebar.svelte';
  import TabBar from './components/layout/TabBar.svelte';
  import Dashboard from './components/dashboard/Dashboard.svelte';
  import FanGroups from './components/fans/FanGroups.svelte';
  import DevicePanel from './components/devices/DevicePanel.svelte';
  import Lighting from './components/lighting/Lighting.svelte';
  import Settings from './components/settings/Settings.svelte';
  import { getStartupConfigIssue } from './lib/api';
  import type { StartupConfigIssue } from './lib/types';

  type Tab = 'dashboard' | 'fans' | 'devices' | 'lighting' | 'settings';
  let activeTab = $state<Tab>('dashboard');

  let configIssue = $state<StartupConfigIssue | null>(null);
  let issueDismissed = $state(false);

  onMount(() => {
    initSensorListener();
    initRgbListener();
    getStartupConfigIssue()
      .then((issue) => { configIssue = issue; })
      .catch(() => {});
  });
</script>

<Titlebar />
<TabBar active={activeTab} onchange={(tab) => { activeTab = tab; }} />

{#if configIssue && !issueDismissed}
  <div class="banner config-issue" role="alert">
    <div class="issue-text">
      <strong>Your configuration file could not be loaded.</strong>
      {#if configIssue.preserved_path}
        It was preserved unchanged at <code>{configIssue.preserved_path}</code>.
      {:else}
        It could NOT be copied to a backup{configIssue.preserve_error ? `: ${configIssue.preserve_error}` : ''}.
      {/if}
      Error: {configIssue.error}.
      {#if configIssue.loaded === 'last_good'}
        Running the last known good configuration instead.
      {:else}
        Running default settings instead — fan groups and RGB settings are not loaded.
      {/if}
      Saving any change will write a new config.toml; the preserved file remains.
    </div>
    <button type="button" class="issue-dismiss" onclick={() => { issueDismissed = true; }}>Dismiss</button>
  </div>
{/if}

<main class="content">
  {#key activeTab}
    <div class="page-enter">
      {#if activeTab === 'dashboard'}
        <Dashboard />
      {:else if activeTab === 'fans'}
        <FanGroups />
      {:else if activeTab === 'devices'}
        <DevicePanel />
      {:else if activeTab === 'lighting'}
        <Lighting />
      {:else if activeTab === 'settings'}
        <Settings />
      {/if}
    </div>
  {/key}
</main>

<style>
  .config-issue {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    margin: 8px 16px 0;
    flex-shrink: 0;
  }
  .issue-text {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
    line-height: 1.4;
  }
  .issue-dismiss {
    flex-shrink: 0;
    background: transparent;
    color: inherit;
    border: 1px solid currentColor;
    border-radius: var(--radius);
    padding: 2px 10px;
    font: inherit;
    cursor: pointer;
  }
  .issue-dismiss:hover {
    background: rgba(251, 191, 36, 0.15);
  }
  .issue-dismiss:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .content {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
  .page-enter {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: fadeIn 0.15s ease;
  }
</style>
