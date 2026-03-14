<script lang="ts">
  import { onMount } from 'svelte';
  import { initSensorListener } from './lib/stores/sensors.svelte';
  import Titlebar from './components/layout/Titlebar.svelte';
  import TabBar from './components/layout/TabBar.svelte';
  import Dashboard from './components/dashboard/Dashboard.svelte';
  import FanGroups from './components/fans/FanGroups.svelte';
  import DevicePanel from './components/devices/DevicePanel.svelte';
  import Settings from './components/settings/Settings.svelte';

  type Tab = 'dashboard' | 'fans' | 'devices' | 'settings';
  let activeTab = $state<Tab>('dashboard');

  onMount(() => {
    initSensorListener();
  });
</script>

<Titlebar />
<TabBar active={activeTab} onchange={(tab) => { activeTab = tab; }} />

<main class="content">
  {#if activeTab === 'dashboard'}
    <Dashboard />
  {:else if activeTab === 'fans'}
    <FanGroups />
  {:else if activeTab === 'devices'}
    <DevicePanel />
  {:else if activeTab === 'settings'}
    <Settings />
  {/if}
</main>

<style>
  .content {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
</style>
