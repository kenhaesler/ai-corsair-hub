<script lang="ts">
  import { onMount, type Component } from 'svelte';
  import Titlebar from './components/shell/Titlebar.svelte';
  import Toolbar from './components/shell/Toolbar.svelte';
  import StatusBar from './components/shell/StatusBar.svelte';
  import Ambient from './components/shell/Ambient.svelte';
  import Toasts from './components/shell/Toasts.svelte';
  import PageHost from './components/shell/PageHost.svelte';
  import Dashboard from './screens/dashboard/Dashboard.svelte';
  import { initSensorListener } from './lib/stores/sensors.svelte';
  import { loadConfig } from './lib/stores/config.svelte';
  import { ui, TABS, type Tab } from './lib/stores/ui.svelte';
  import { scrub } from './lib/stores/scrub.svelte';
  import { getStartupConfigIssue } from './lib/api';

  // Non-dashboard screens are code-split and mounted on first visit, then
  // kept mounted so their local state survives tab switches.
  const loaders: Record<Exclude<Tab, 'dashboard'>, () => Promise<{ default: Component }>> = {
    fans: () => import('./screens/fans/FanControl.svelte'),
    devices: () => import('./screens/devices/Devices.svelte'),
    lighting: () => import('./screens/lighting/Lighting.svelte'),
    settings: () => import('./screens/settings/Settings.svelte'),
  };
  const modules = $state<Partial<Record<Tab, Component>>>({});

  async function ensure(tab: Tab) {
    if (tab === 'dashboard' || modules[tab]) return;
    const m = await loaders[tab]();
    modules[tab] = m.default;
  }
  $effect(() => {
    ensure(ui.activeTab);
  });

  onMount(() => {
    initSensorListener();
    loadConfig();
    getStartupConfigIssue()
      .then((issue) => (ui.configIssue = issue))
      .catch(() => {});

    // Warm the other screens' chunks once idle (no mount, just fetch).
    const idle = (cb: () => void) =>
      'requestIdleCallback' in window ? window.requestIdleCallback(cb, { timeout: 3000 }) : setTimeout(cb, 1500);
    idle(() => Object.values(loaders).forEach((l) => l().catch(() => {})));
  });

  function onKey(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;
    if (mod && !e.shiftKey && !e.altKey && /^[1-5]$/.test(e.key)) {
      e.preventDefault();
      ui.goTab(TABS[Number(e.key) - 1]);
      return;
    }
    if (e.key === 'Escape' && !e.defaultPrevented) {
      if (scrub.active) {
        scrub.release();
        return;
      }
      const back = ui.chrome[ui.activeTab].back;
      if (back) {
        e.preventDefault();
        back.onback();
      }
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<Ambient />
<div class="app">
  <Titlebar />
  <div class="stage">
    <Toolbar />
    <PageHost tab="dashboard"><Dashboard /></PageHost>
    {#each ['fans', 'devices', 'lighting', 'settings'] as const as tab (tab)}
      {@const Screen = modules[tab]}
      <PageHost {tab}>
        {#if Screen}<Screen />{/if}
      </PageHost>
    {/each}
  </div>
  <StatusBar />
</div>
<Toasts />

<style>
  .app {
    position: relative;
    z-index: 1;
    height: 100%;
    display: grid;
    /* status bar row collapses when the active tab hides it */
    grid-template-rows: var(--titlebar-h) minmax(0, 1fr) auto;
  }
  .stage {
    position: relative;
    min-height: 0;
    overflow: hidden;
  }
</style>
