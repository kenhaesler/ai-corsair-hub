<script lang="ts">
  type Tab = 'dashboard' | 'fans' | 'devices' | 'lighting' | 'settings';

  interface Props {
    active: Tab;
    onchange: (tab: Tab) => void;
  }

  let { active, onchange }: Props = $props();

  const tabs: { id: Tab; label: string; icon: string }[] = [
    { id: 'dashboard', label: 'Dashboard', icon: 'M3 3h7v7H3V3zm11 0h7v7h-7V3zM3 14h7v7H3v-7zm11 0h7v7h-7v-7z' },
    { id: 'fans', label: 'Fan Control', icon: 'M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10 10-4.5 10-10S17.5 2 12 2zm0 4c1.1 0 2 .9 2 2s-.9 2-2 2-2-.9-2-2 .9-2 2-2zm-5 8.5c0-1.4 2.2-2.5 5-2.5s5 1.1 5 2.5' },
    { id: 'devices', label: 'Devices', icon: 'M7 2h10a2 2 0 012 2v16a2 2 0 01-2 2H7a2 2 0 01-2-2V4a2 2 0 012-2zm1 4v4h8V6H8zm4 8a1.5 1.5 0 100 3 1.5 1.5 0 000-3z' },
    { id: 'lighting', label: 'Lighting', icon: 'M9 21c0 .5.4 1 1 1h4c.6 0 1-.5 1-1v-1H9v1zm3-19C8.1 2 5 5.1 5 9c0 2.4 1.2 4.5 3 5.7V17c0 .5.4 1 1 1h6c.6 0 1-.5 1-1v-2.3c1.8-1.3 3-3.4 3-5.7 0-3.9-3.1-7-7-7z' },
    { id: 'settings', label: 'Settings', icon: 'M12 15.5A3.5 3.5 0 018.5 12 3.5 3.5 0 0112 8.5a3.5 3.5 0 013.5 3.5 3.5 3.5 0 01-3.5 3.5zm7.43-2.53c.04-.32.07-.64.07-.97s-.03-.66-.07-1l2.11-1.63c.19-.15.24-.42.12-.64l-2-3.46c-.12-.22-.39-.3-.61-.22l-2.49 1c-.52-.4-1.08-.73-1.69-.98l-.38-2.65A.49.49 0 0014 2h-4c-.25 0-.46.18-.49.42l-.38 2.65c-.61.25-1.17.59-1.69.98l-2.49-1c-.23-.09-.49 0-.61.22l-2 3.46c-.13.22-.07.49.12.64L4.57 11c-.04.34-.07.67-.07 1s.03.65.07.97l-2.11 1.66c-.19.15-.25.42-.12.64l2 3.46c.12.22.39.3.61.22l2.49-1.01c.52.4 1.08.73 1.69.98l.38 2.65c.03.24.24.42.49.42h4c.25 0 .46-.18.49-.42l.38-2.65c.61-.25 1.17-.58 1.69-.98l2.49 1.01c.22.08.49 0 .61-.22l2-3.46c.12-.22.07-.49-.12-.64l-2.11-1.66z' },
  ];
</script>

<nav class="tabbar">
  {#each tabs as tab}
    <button
      class="tab"
      class:active={active === tab.id}
      onclick={() => onchange(tab.id)}
    >
      <svg viewBox="0 0 24 24" class="tab-icon" fill="currentColor">
        <path d={tab.icon} />
      </svg>
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
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 16px;
    font-size: 12px;
    font-weight: 500;
    color: var(--text-secondary);
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    cursor: pointer;
    transition: color 0.15s, border-color 0.15s, background 0.15s;
    border-radius: 0;
    position: relative;
  }
  .tab:hover {
    color: var(--text-primary);
    background: rgba(255,255,255,0.03);
    border-color: transparent;
    border-bottom-color: rgba(255,255,255,0.1);
  }
  .tab.active {
    color: var(--accent);
    border-bottom-color: var(--accent);
    background: rgba(96, 165, 250, 0.05);
  }
  .tab-icon {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
  }
</style>
