<!--
  Devices (inventory 3). Large-title page "USB Devices" (the tab stays
  "Devices"): a grouped Diagnostics inset (Identify, LED markers, Walk), live
  notices, the Identify intro and the marker legend (while running), one
  section per hub (daisy-chain strip + channel table with inline rename; the
  diagnostics' per-device results show up in these tables), the PSU, and
  "All Corsair devices". Refresh sits in the toolbar.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { slide } from 'svelte/transition';
  import { expoOut } from 'svelte/easing';
  import RefreshCw from 'lucide-svelte/icons/refresh-cw';
  import Unplug from 'lucide-svelte/icons/unplug';
  import LargeTitlePage from '../../components/shell/LargeTitlePage.svelte';
  import ToolbarActions from '../../components/shell/ToolbarActions.svelte';
  import Banner from '../../components/ui/Banner.svelte';
  import Button from '../../components/ui/Button.svelte';
  import EmptyState from '../../components/ui/EmptyState.svelte';
  import Skeleton from '../../components/ui/Skeleton.svelte';
  import { getDevices, onDeviceTopology, onLinkSubdevicesChanged } from '../../lib/api';
  import type { DeviceTree, LinkSubdevicesChanged, TopologyEvent } from '../../lib/types';
  import { configStore, loadConfig } from '../../lib/stores/config.svelte';
  import { shortHubSerial } from '../../lib/identity';
  import { reducedMotion } from '../../lib/motion.svelte';
  import { Diagnostics } from './diagnostics.svelte';
  import DiagnosticsGroup from './DiagnosticsGroup.svelte';
  import IdentifyNote from './IdentifyNote.svelte';
  import MarkerLegend from './MarkerLegend.svelte';
  import Notice from './Notice.svelte';
  import HubSection from './HubSection.svelte';
  import PsuCard from './PsuCard.svelte';
  import DetectedList from './DetectedList.svelte';

  let devices = $state<DeviceTree | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(true);

  const diag = new Diagnostics((m) => (error = m));

  const walkDevices = $derived(
    (devices?.hubs ?? []).flatMap((h) =>
      h.devices.map((d) => ({
        id: d.device_id,
        label: `Hub ${shortHubSerial(h.serial)} Ch ${d.channel} · ${d.device_type}`,
      })),
    ),
  );

  // --- scanning ------------------------------------------------------------
  // Overlapping fetches can resolve out of order; only the newest is applied.
  let refreshSeq = 0;

  /** `silent` (event-driven) refreshes do not set `loading` or touch `error`. */
  async function refresh(opts: { silent?: boolean } = {}) {
    const silent = opts.silent === true;
    const seq = ++refreshSeq;
    if (!silent) {
      loading = true;
      error = null;
    }
    try {
      const tree = await getDevices();
      if (seq === refreshSeq) devices = tree;
    } catch (e) {
      if (!silent && seq === refreshSeq) error = String(e);
    } finally {
      if (seq === refreshSeq) loading = false;
    }
  }

  onMount(async () => {
    // Config supplies the user-set names for the Name column.
    if (!configStore.config) await loadConfig();
    await refresh();
  });

  // --- live notices (diagnostic only) -------------------------------------
  const NOTICE_MS = 6000;
  const REFRESH_DEBOUNCE_MS = 500;
  let notice = $state<{ text: string; tone: 'info' | 'success' | 'warning' } | null>(null);
  let noticeTimer: ReturnType<typeof setTimeout> | undefined;
  let refreshTimer: ReturnType<typeof setTimeout> | undefined;

  function showNotice(text: string, tone: 'info' | 'success' | 'warning') {
    notice = { text, tone };
    clearTimeout(noticeTimer);
    noticeTimer = setTimeout(() => (notice = null), NOTICE_MS);
  }

  function scheduleRefresh() {
    clearTimeout(refreshTimer);
    refreshTimer = setTimeout(() => refresh({ silent: true }), REFRESH_DEBOUNCE_MS);
  }

  function onTopology(e: TopologyEvent) {
    const name = e.device?.name ?? 'Corsair device';
    switch (e.kind) {
      case 'added':
        showNotice(`${name} connected`, 'success');
        break;
      case 'removed':
        showNotice(`${name} disconnected`, 'warning');
        break;
      case 'changed':
        showNotice(`${name} changed`, 'info');
        break;
      case 'suspending':
        showNotice('System suspending', 'info');
        break;
      case 'resumed':
        showNotice('System resumed - re-checking devices', 'info');
        break;
      default:
        return;
    }
    scheduleRefresh();
  }

  const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;

  function onSubdevices(e: LinkSubdevicesChanged) {
    const parts: string[] = [];
    const added = e.added?.length ?? 0;
    const removed = e.removed?.length ?? 0;
    const moved = e.moved?.length ?? 0;
    if (added) parts.push(`${plural(added, 'device')} added`);
    if (removed) parts.push(`${plural(removed, 'device')} removed`);
    if (moved) parts.push(`${plural(moved, 'device')} moved`);
    if (parts.length === 0) return;
    showNotice(`Hub ${shortHubSerial(e.hub_serial ?? '')}: ${parts.join(', ')}`, 'info');
    scheduleRefresh();
  }

  onMount(() => {
    let disposed = false;
    const unlisten: (() => void)[] = [];
    for (const sub of [onDeviceTopology(onTopology), onLinkSubdevicesChanged(onSubdevices)]) {
      sub.then((fn) => (disposed ? fn() : unlisten.push(fn))).catch(() => {});
    }
    return () => {
      disposed = true;
      unlisten.forEach((fn) => fn());
      clearTimeout(noticeTimer);
      clearTimeout(refreshTimer);
      diag.dispose();
    };
  });

  const slideOpts = $derived({ duration: reducedMotion.current ? 0 : 320, easing: expoOut });
</script>

<ToolbarActions>
  <Button size="sm" icon={RefreshCw} loading={loading} onclick={() => refresh()}>
    {loading ? 'Scanning...' : 'Refresh'}
  </Button>
</ToolbarActions>

<LargeTitlePage title="USB Devices" maxWidth={1440}>
  {#if error}
    <div class="gap" transition:slide={slideOpts}>
      <Banner tone="error" ondismiss={() => (error = null)}>{error}</Banner>
    </div>
  {/if}

  <div class="notice-region" role="status" aria-live="polite">
    {#if notice}
      <div class="gap" transition:slide={slideOpts}>
        <Notice text={notice.text} tone={notice.tone} />
      </div>
    {/if}
  </div>

  <section aria-label="Devices">
    <DiagnosticsGroup {diag} {walkDevices} />

    {#if diag.identifyMap && diag.identifyRunning}
      <div transition:slide={slideOpts}>
        <IdentifyNote />
      </div>
    {/if}
    {#if diag.markerReport && diag.markerRunning}
      <div transition:slide={slideOpts}>
        <MarkerLegend report={diag.markerReport} walkLed={diag.walkLed} />
      </div>
    {/if}

    {#if devices}
      <div class="hubs">
        {#each devices.hubs as hub (hub.serial)}
          <HubSection {hub} {diag} />
        {/each}
      </div>

      {#if devices.psu}
        <div class="psu"><PsuCard psu={devices.psu} /></div>
      {/if}

      {#if devices.hubs.length === 0 && !devices.psu}
        <div class="empty">
          <EmptyState icon={Unplug} title="No Corsair devices detected on USB bus." message="Connect an iCUE LINK hub or a supported power supply, then refresh." />
        </div>
      {/if}
    {:else if loading}
      <div class="skeletons" aria-hidden="true">
        <Skeleton height="22px" width="220px" />
        <Skeleton height="148px" radius="14px" />
      </div>
    {:else}
      <div class="empty">
        <EmptyState icon={Unplug} title="Unable to scan devices." />
      </div>
    {/if}
  </section>

  {#if devices?.detected && devices.detected.length > 0}
    <section class="section" aria-labelledby="all-h">
      <div class="section-head">
        <h2 id="all-h" class="t-title2">All Corsair devices</h2>
      </div>
      <DetectedList
        devices={devices.detected}
        excluded={devices.excluded_count ?? 0}
        hubSerials={devices.hubs.map((h) => h.serial)}
        psuSerial={devices.psu?.serial ?? null}
      />
    </section>
  {/if}
</LargeTitlePage>

<style>
  .gap {
    padding-bottom: var(--sp-3);
  }
  .hubs {
    display: flex;
    flex-direction: column;
    gap: var(--sp-8);
    margin-top: var(--sp-8);
  }
  .psu {
    margin-top: var(--sp-8);
  }
  .empty {
    margin-top: var(--sp-6);
    border-radius: var(--r-panel);
    background: var(--panel-bg);
    box-shadow: var(--panel-shadow);
  }
  .skeletons {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    margin-top: var(--sp-8);
  }
</style>
