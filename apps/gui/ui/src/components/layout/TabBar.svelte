<script lang="ts">
  type Tab = 'dashboard' | 'fans' | 'devices' | 'settings';

  interface Props {
    active: Tab;
    onchange: (tab: Tab) => void;
  }

  let { active, onchange }: Props = $props();

  const tabs: { id: Tab; label: string }[] = [
    { id: 'dashboard', label: 'Dashboard' },
    { id: 'fans', label: 'Fan Control' },
    { id: 'devices', label: 'Devices' },
    { id: 'settings', label: 'Settings' },
  ];
</script>

<nav class="tabbar">
  {#each tabs as tab}
    <button
      class="tab"
      class:active={active === tab.id}
      onclick={() => onchange(tab.id)}
    >
      {tab.label}
    </button>
  {/each}
</nav>

<style>
  .tabbar {
    display: flex;
    gap: 2px;
    padding: 0 16px;
    background: rgba(0, 0, 0, 0.05);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }
  .tab {
    padding: 8px 16px;
    font-size: 12px;
    font-weight: 500;
    color: var(--text-secondary);
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    cursor: pointer;
    transition: color 0.15s, border-color 0.15s;
    border-radius: 0;
  }
  .tab:hover {
    color: var(--text-primary);
    background: transparent;
    border-color: transparent;
    border-bottom-color: rgba(255,255,255,0.1);
  }
  .tab.active {
    color: var(--accent);
    border-bottom-color: var(--accent);
  }
</style>
