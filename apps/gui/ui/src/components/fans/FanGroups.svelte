<script lang="ts">
  import { configStore, loadConfig, saveCurrentConfig } from '../../lib/stores/config.svelte';
  import { sensors } from '../../lib/stores/sensors.svelte';
  import type { FanGroupConfig } from '../../lib/types';
  import FanGroupCard from './FanGroupCard.svelte';
  import { onMount } from 'svelte';

  onMount(() => { loadConfig(); });

  function updateGroup(index: number, group: FanGroupConfig) {
    if (!configStore.config) return;
    const groups = [...configStore.config.fan_groups];
    groups[index] = group;
    configStore.config = { ...configStore.config, fan_groups: groups };
  }

  // Get current CPU temp for curve display
  const cpuTemp = $derived(
    sensors.snapshot?.temperatures.find(t => t.source === 'cpu')?.celsius
  );
</script>

<div class="fan-groups scroll-area">
  {#if configStore.error}
    <div class="banner error">{configStore.error}</div>
  {/if}

  {#if configStore.config}
    {#if configStore.config.fan_groups.length === 0}
      <div class="empty-state card">
        <p>No fan groups configured. Edit config.toml to add fan groups.</p>
      </div>
    {:else}
      <div class="groups-grid">
        {#each configStore.config.fan_groups as group, i}
          <FanGroupCard
            {group}
            currentTemp={cpuTemp}
            onchange={(g) => updateGroup(i, g)}
          />
        {/each}
      </div>

      <div class="actions">
        <button class="primary" onclick={saveCurrentConfig} disabled={configStore.saving}>
          {configStore.saving ? 'Saving...' : 'Save Configuration'}
        </button>
      </div>
    {/if}
  {:else}
    <div class="loading">Loading configuration...</div>
  {/if}
</div>

<style>
  .fan-groups {
    display: flex;
    flex-direction: column;
  }
  .groups-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(400px, 1fr));
    gap: 12px;
  }
  .actions {
    margin-top: 16px;
    display: flex;
    justify-content: flex-end;
  }
  .empty-state {
    text-align: center;
    padding: 40px;
    color: var(--text-secondary);
  }
  .loading {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 200px;
    color: var(--text-secondary);
  }
</style>
