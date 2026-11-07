<!--
  TabStrip (inventory 0.2): the five areas, icon + label, a sliding
  selection pill on a spring. Ctrl+1…5 are bound in App. Arrow keys move
  between tabs (roving tabindex).
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import HeartPulse from 'lucide-svelte/icons/heart-pulse';
  import Fan from 'lucide-svelte/icons/fan';
  import Microchip from 'lucide-svelte/icons/microchip';
  import Lightbulb from 'lucide-svelte/icons/lightbulb';
  import Settings from 'lucide-svelte/icons/settings';
  import { ui, type Tab } from '../../lib/stores/ui.svelte';
  import { configStore } from '../../lib/stores/config.svelte';
  import { createSpring, springTo, springs } from '../../lib/motion.svelte';

  const tabs: { id: Tab; label: string; icon: typeof Fan; key: string }[] = [
    { id: 'dashboard', label: 'Dashboard', icon: HeartPulse, key: '1' },
    { id: 'fans', label: 'Fan Control', icon: Fan, key: '2' },
    { id: 'devices', label: 'Devices', icon: Microchip, key: '3' },
    { id: 'lighting', label: 'Lighting', icon: Lightbulb, key: '4' },
    { id: 'settings', label: 'Settings', icon: Settings, key: '5' },
  ];

  let root: HTMLDivElement;
  const btns = $state<Record<string, HTMLButtonElement>>({});
  const pill = createSpring({ x: 0, w: 0 }, springs.tab);
  let ready = $state(false);

  function measure(instant = false) {
    const b = btns[ui.activeTab];
    if (!b) return;
    springTo(pill, { x: b.offsetLeft, w: b.offsetWidth }, instant || !ready);
    ready = true;
  }
  $effect(() => {
    void ui.activeTab;
    measure();
  });
  onMount(() => {
    measure(true);
    const ro = new ResizeObserver(() => measure(true));
    ro.observe(root);
    document.fonts?.ready.then(() => measure(true));
    return () => ro.disconnect();
  });

  function onKey(e: KeyboardEvent) {
    const i = tabs.findIndex((t) => t.id === ui.activeTab);
    let j = i;
    if (e.key === 'ArrowRight') j = (i + 1) % tabs.length;
    else if (e.key === 'ArrowLeft') j = (i - 1 + tabs.length) % tabs.length;
    else if (e.key === 'Home') j = 0;
    else if (e.key === 'End') j = tabs.length - 1;
    else return;
    e.preventDefault();
    ui.goTab(tabs[j].id);
    btns[tabs[j].id]?.focus();
  }
</script>

<div class="strip" role="tablist" aria-label="Areas" tabindex="-1" bind:this={root} onkeydown={onKey}>
  <span
    class="pill"
    class:ready
    style:transform="translateX({pill.current.x}px)"
    style:width="{pill.current.w}px"
    aria-hidden="true"
  ></span>
  {#each tabs as t (t.id)}
    {@const Icon = t.icon}
    <button
      type="button"
      role="tab"
      id="tab-{t.id}"
      aria-selected={ui.activeTab === t.id}
      aria-controls="page-{t.id}"
      tabindex={ui.activeTab === t.id ? 0 : -1}
      class="tab"
      class:sel={ui.activeTab === t.id}
      title="{t.label} (Ctrl+{t.key})"
      bind:this={btns[t.id]}
      onclick={() => ui.goTab(t.id)}
    >
      <Icon size={15} strokeWidth={2} />
      <span>{t.label}</span>
      {#if t.id === 'fans' && configStore.dirty}
        <span class="dirty" aria-label="unsaved changes"></span>
      {/if}
    </button>
  {/each}
</div>

<style>
  .strip {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 2px;
    padding: 3px;
    border-radius: 11px;
    background: rgba(255, 255, 255, 0.05);
    box-shadow:
      inset 0 0 0 1px var(--hair),
      var(--hair-inner);
    isolation: isolate;
    -webkit-app-region: no-drag;
  }
  .pill {
    position: absolute;
    left: 0;
    top: 3px;
    bottom: 3px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.13);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.09),
      0 1px 2px rgb(0 0 0 / 0.45),
      0 4px 12px -4px rgb(0 0 0 / 0.5);
    z-index: -1;
    opacity: 0;
    will-change: transform, width;
  }
  .pill.ready {
    opacity: 1;
  }
  .tab {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 30px;
    padding: 0 13px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--t2);
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    white-space: nowrap;
    transition:
      color var(--dur-fast) ease-out,
      transform var(--dur-press) ease-out;
  }
  .tab:hover {
    color: var(--t1);
  }
  .tab:active {
    transform: scale(0.97);
  }
  .tab.sel {
    color: var(--t1);
    font-weight: 600;
  }
  .tab.sel :global(svg) {
    color: var(--accent-strong);
  }
  .dirty {
    position: absolute;
    top: 6px;
    right: 6px;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--warn);
  }
</style>
