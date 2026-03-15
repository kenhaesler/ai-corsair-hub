<script lang="ts">
  import { configStore, loadConfig, saveCurrentConfig } from '../../lib/stores/config.svelte';
  import { sensors } from '../../lib/stores/sensors.svelte';
  import type { FanGroupConfig } from '../../lib/types';
  import FanGroupCard from './FanGroupCard.svelte';
  import { onMount } from 'svelte';

  onMount(() => { loadConfig(); });

  let selectedIndex = $state(0);

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

  // Live duty for each group
  function groupDuty(name: string): number | undefined {
    return sensors.snapshot?.group_duties.find(g => g.name === name)?.duty_percent;
  }

  // Clamp selected index
  $effect(() => {
    if (configStore.config && selectedIndex >= configStore.config.fan_groups.length) {
      selectedIndex = Math.max(0, configStore.config.fan_groups.length - 1);
    }
  });
</script>

<div class="fan-groups">
  {#if configStore.error}
    <div class="banner error" style="margin: 16px 16px 0">{configStore.error}</div>
  {/if}

  {#if configStore.config}
    {#if configStore.config.fan_groups.length === 0}
      <div class="empty-state card" style="margin: 16px">
        <p>No fan groups configured. Edit config.toml to add fan groups.</p>
      </div>
    {:else}
      <div class="master-detail">
        <!-- Sidebar -->
        <nav class="sidebar">
          {#each configStore.config.fan_groups as group, i}
            {@const duty = groupDuty(group.name)}
            <button
              class="sidebar-item"
              class:active={selectedIndex === i}
              onclick={() => { selectedIndex = i; }}
            >
              <span class="dot" style="background: {duty !== undefined ? 'var(--success)' : 'var(--text-muted)'}"></span>
              <div class="sidebar-info">
                <span class="sidebar-name">{group.name}</span>
                <span class="sidebar-mode">{group.mode.type}</span>
              </div>
              {#if duty !== undefined}
                <span class="sidebar-duty tabular-nums">{Math.round(duty)}%</span>
              {/if}
            </button>
          {/each}
        </nav>

        <!-- Detail panel -->
        <div class="detail">
          {#if configStore.config.fan_groups[selectedIndex]}
            <FanGroupCard
              group={configStore.config.fan_groups[selectedIndex]}
              currentTemp={cpuTemp}
              onchange={(g) => updateGroup(selectedIndex, g)}
              expanded
            />
          {/if}

          <div class="actions">
            <button class="primary" onclick={saveCurrentConfig} disabled={configStore.saving}>
              {configStore.saving ? 'Saving...' : 'Save Configuration'}
            </button>
          </div>
        </div>
      </div>
    {/if}
  {:else}
    <div class="loading">Loading configuration...</div>
  {/if}
</div>

<style>
  .fan-groups {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .master-detail {
    flex: 1;
    display: flex;
    overflow: hidden;
  }
  .sidebar {
    width: 200px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 12px 8px;
    border-right: 1px solid var(--border);
    overflow-y: auto;
  }
  .sidebar-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius);
    cursor: pointer;
    text-align: left;
    transition: background 0.15s, border-color 0.15s;
  }
  .sidebar-item:hover {
    background: var(--bg-card-hover);
    border-color: transparent;
  }
  .sidebar-item.active {
    background: var(--bg-card-active);
    border-color: var(--accent-dim);
    box-shadow: var(--shadow-glow);
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .sidebar-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .sidebar-name {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sidebar-mode {
    font-size: 9px;
    color: var(--text-muted);
    text-transform: capitalize;
  }
  .sidebar-duty {
    font-size: 13px;
    font-weight: 600;
    color: var(--accent);
    flex-shrink: 0;
  }
  .detail {
    flex: 1;
    display: flex;
    flex-direction: column;
    padding: 16px;
    overflow-y: auto;
    min-width: 0;
  }
  .actions {
    margin-top: 16px;
    display: flex;
    justify-content: flex-end;
    flex-shrink: 0;
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
