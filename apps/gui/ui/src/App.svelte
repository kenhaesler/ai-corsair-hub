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

  type Tab = 'dashboard' | 'fans' | 'devices' | 'lighting' | 'settings';
  let activeTab = $state<Tab>('dashboard');

  onMount(() => {
    initSensorListener();
    initRgbListener();
  });
</script>

<Titlebar />
<TabBar active={activeTab} onchange={(tab) => { activeTab = tab; }} />

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
