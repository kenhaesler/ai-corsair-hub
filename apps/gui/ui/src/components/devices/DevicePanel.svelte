<script lang="ts">
  import {
    getDevices,
    rgbIdentifyChannels,
    rgbMarkerDiagnostic,
    onDeviceTopology,
    onLinkSubdevicesChanged,
    type ChannelIdentity,
    type LedMarkerReport,
  } from '../../lib/api';
  import type { DetectedDevice, DeviceTree, LinkSubdevicesChanged, TopologyEvent } from '../../lib/types';
  import HubDetail from './HubDetail.svelte';
  import { loadConfig, configStore } from '../../lib/stores/config.svelte';
  import { onMount } from 'svelte';

  let devices = $state<DeviceTree | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(true);

  // Channel-identify diagnostic state. When active, every enumerated
  // RGB device is painted a unique colour for IDENTIFY_SECS seconds;
  // we render a legend here so the user can correlate what they
  // physically see with the channel numbers the firmware reports.
  const IDENTIFY_SECS = 20;
  let identifyMap = $state<ChannelIdentity[] | null>(null);
  let identifyRunning = $state(false);
  let identifyDeadline = $state<number | null>(null);
  let identifyRemaining = $state(0);

  $effect(() => {
    if (!identifyRunning || identifyDeadline == null) return;
    const tick = () => {
      const left = Math.max(0, Math.ceil((identifyDeadline! - Date.now()) / 1000));
      identifyRemaining = left;
      if (left <= 0) {
        identifyRunning = false;
        identifyDeadline = null;
      }
    };
    tick();
    const handle = setInterval(tick, 500);
    return () => clearInterval(handle);
  });

  // LED marker diagnostic (G05). Same countdown UX as Identify; the report
  // carries the expected slot layout so the operator can verify each LED
  // range against what is physically lit.
  const MARKER_SECS = 60;
  let markerReport = $state<LedMarkerReport | null>(null);
  let markerRunning = $state(false);
  let markerDeadline = $state<number | null>(null);
  let markerRemaining = $state(0);
  const diagnosticBusy = $derived(identifyRunning || markerRunning);

  $effect(() => {
    if (!markerRunning || markerDeadline == null) return;
    const tick = () => {
      const left = Math.max(0, Math.ceil((markerDeadline! - Date.now()) / 1000));
      markerRemaining = left;
      if (left <= 0) {
        markerRunning = false;
        markerDeadline = null;
      }
    };
    tick();
    const handle = setInterval(tick, 500);
    return () => clearInterval(handle);
  });

  async function runMarkers() {
    if (diagnosticBusy) return;
    try {
      const report = await rgbMarkerDiagnostic(MARKER_SECS);
      markerReport = report;
      markerRunning = true;
      markerDeadline = Date.now() + report.duration_secs * 1000;
      markerRemaining = report.duration_secs;
    } catch (e) {
      error = `LED markers failed: ${String(e)}`;
    }
  }

  async function runIdentify() {
    if (diagnosticBusy) return;
    try {
      const mapping = await rgbIdentifyChannels(IDENTIFY_SECS);
      identifyMap = mapping;
      markerRunning = false; // Identify cancels the marker diagnostic
      markerDeadline = null;
      identifyRunning = true;
      identifyDeadline = Date.now() + IDENTIFY_SECS * 1000;
      identifyRemaining = IDENTIFY_SECS;
    } catch (e) {
      error = `Identify failed: ${String(e)}`;
    }
  }

  onMount(async () => {
    // Config is needed so HubDetail can render user-set friendly names in
    // the Name column. loadConfig is a no-op if the config is already in
    // the store.
    if (!configStore.config) {
      await loadConfig();
    }
    await refresh();
  });

  // Hotplug / resume notices (diagnostic only; never touches addressing or config).
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
    refreshTimer = setTimeout(refresh, REFRESH_DEBOUNCE_MS);
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

  function plural(n: number, word: string): string {
    return `${n} ${word}${n === 1 ? '' : 's'}`;
  }

  function onSubdevices(e: LinkSubdevicesChanged) {
    const parts: string[] = [];
    const added = e.added?.length ?? 0;
    const removed = e.removed?.length ?? 0;
    const moved = e.moved?.length ?? 0;
    if (added) parts.push(`${plural(added, 'device')} added`);
    if (removed) parts.push(`${plural(removed, 'device')} removed`);
    if (moved) parts.push(`${plural(moved, 'device')} moved`);
    if (parts.length === 0) return;
    showNotice(`Hub ${shortSerial(e.hub_serial ?? '')}: ${parts.join(', ')}`, 'info');
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
    };
  });

  const SUPPORT_LABEL: Record<string, string> = {
    supported: 'Supported',
    detect_only: 'Detected - no driver',
    unknown: 'Unknown',
  };
  const STATUS_LABEL: Record<string, string> = {
    not_ready: 'Not ready',
    quarantined: 'Quarantined',
    backoff: 'Retrying',
  };
  const supportLabel = (d: DetectedDevice) => SUPPORT_LABEL[d.support] ?? 'Unknown';
  const pidHex = (d: DetectedDevice) =>
    `0x${d.pid.toString(16).toUpperCase().padStart(4, '0')}`;

  async function refresh() {
    loading = true;
    error = null;
    try {
      devices = await getDevices();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function shortSerial(s: string): string {
    return s.length > 8 ? s.slice(-4) : s;
  }

  /** Group identify rows by hub serial for legend rendering. */
  function groupByHub(rows: ChannelIdentity[]): { serial: string; rows: ChannelIdentity[] }[] {
    const byHub = new Map<string, ChannelIdentity[]>();
    for (const r of rows) {
      if (!byHub.has(r.hub_serial)) byHub.set(r.hub_serial, []);
      byHub.get(r.hub_serial)!.push(r);
    }
    return Array.from(byHub.entries()).map(([serial, rows]) => ({
      serial,
      rows: rows.slice().sort((a, b) => a.channel - b.channel),
    }));
  }
</script>

<div class="device-panel scroll-area">
  <div class="header-row">
    <h3 class="section-title">USB Devices</h3>
    <div class="header-actions">
      <button class="identify-btn" onclick={runIdentify} disabled={diagnosticBusy}>
        {identifyRunning ? `Identifying… ${identifyRemaining}s` : 'Identify Channels'}
      </button>
      <button class="identify-btn" onclick={runMarkers} disabled={diagnosticBusy}>
        {markerRunning ? `LED markers… ${markerRemaining}s` : 'LED markers'}
      </button>
      <button onclick={refresh} disabled={loading}>
        {loading ? 'Scanning...' : 'Refresh'}
      </button>
    </div>
  </div>

  {#if error}
    <div class="banner error">{error}</div>
  {/if}

  <div class="notice-region" role="status" aria-live="polite">
    {#if notice}
      <div
        class="banner"
        class:success={notice.tone === 'success'}
        class:notice-info={notice.tone === 'info'}
      >
        {notice.text}
      </div>
    {/if}
  </div>

  {#if identifyMap && identifyRunning}
    <div class="identify-legend">
      <p class="legend-intro">
        Every device now lights up a unique colour for {IDENTIFY_SECS} seconds.
        Look at your rig and match each physical fan/strip to the colour below
        — that tells you which channel number the firmware actually assigned.
      </p>
      {#each groupByHub(identifyMap) as hubGroup}
        <div class="legend-hub">
          <span class="legend-hub-title">Hub {shortSerial(hubGroup.serial)}</span>
          <div class="legend-rows">
            {#each hubGroup.rows as row}
              <div class="legend-row">
                <span
                  class="legend-swatch"
                  style="background: rgb({row.rgb[0]},{row.rgb[1]},{row.rgb[2]})"
                ></span>
                <span class="legend-ch">Ch {row.channel}</span>
                <span class="legend-type">{row.device_type}</span>
                <span class="legend-color-name">{row.color_name}</span>
              </div>
            {/each}
          </div>
        </div>
      {/each}
    </div>
  {/if}

  {#if markerReport && markerRunning}
    <div class="identify-legend">
      <p class="legend-intro">
        Every LED slot shows a marker for {markerReport.duration_secs} seconds:
        {#each markerReport.legend.slice(0, 3) as entry, i}
          <span
            class="legend-swatch inline-swatch"
            style="background: rgb({entry.rgb[0]},{entry.rgb[1]},{entry.rgb[2]})"
          ></span>
          {entry.label} = {entry.meaning}{i < 2 ? ';' : '.'}
        {/each}
        All other LEDs use the device's palette colour. Colours are scaled by port
        power protection (factor {markerReport.power_factor}, {markerReport.total_leds} LEDs
        total), exactly like normal rendering.
      </p>
      {#each markerReport.hubs as hub}
        <div class="legend-hub">
          <span class="legend-hub-title">
            Hub {shortSerial(hub.hub_serial)} · fw {hub.firmware} · {hub.total_leds} LEDs
            {#if !hub.healthy}(unhealthy — not written){/if}
          </span>
          <table class="marker-table">
            <thead>
              <tr>
                <th>Ch</th>
                <th>Device</th>
                <th>Type</th>
                <th>LEDs</th>
                <th>Count source</th>
                <th>LED range</th>
              </tr>
            </thead>
            <tbody>
              {#each hub.slots as slot}
                <tr>
                  <td class="mono">{slot.channel}</td>
                  <td>
                    <span
                      class="legend-swatch inline-swatch"
                      style="background: rgb({slot.body_rgb[0]},{slot.body_rgb[1]},{slot.body_rgb[2]})"
                    ></span>
                    {slot.name ?? slot.device_id.slice(-6)}
                  </td>
                  <td>{slot.device_type}</td>
                  <td class="mono">{slot.led_count}</td>
                  <td>{slot.count_source}</td>
                  <td class="mono">{slot.start_led}–{slot.end_led - 1}</td>
                </tr>
              {/each}
              {#each hub.skipped_zero_count as skipped}
                <tr class="skipped">
                  <td class="mono">{skipped.channel}</td>
                  <td>{skipped.name ?? skipped.device_id.slice(-6)}</td>
                  <td>{skipped.device_type}</td>
                  <td class="mono">0</td>
                  <td>{skipped.count_source}</td>
                  <td>no slot (skipped)</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/each}
      {#if markerReport.file_path}
        <p class="legend-intro marker-file">Manifest saved to <span class="mono">{markerReport.file_path}</span></p>
      {:else if markerReport.file_error}
        <p class="legend-intro marker-file">Manifest export failed: {markerReport.file_error}</p>
      {/if}
    </div>
  {/if}

  {#if devices}
    {#each devices.hubs as hub}
      <div class="card" style="margin-bottom: 12px">
        <HubDetail {hub} />
      </div>
    {/each}

    {#if devices.psu}
      <div class="card">
        <h4 class="psu-title">Corsair HX1500i</h4>
        <div class="psu-info">
          <span class="label">Serial</span>
          <span class="mono">{devices.psu.serial}</span>
        </div>
      </div>
    {/if}

    {#if devices.detected && devices.detected.length > 0}
      <section class="card detected" aria-labelledby="detected-title">
        <h4 id="detected-title" class="detected-title">All Corsair devices</h4>
        <ul class="detected-list">
          {#each devices.detected as d (d.key)}
            <li class="detected-row">
              <span class="detected-name">{d.name}</span>
              <span class="mono detected-meta">PID {pidHex(d)}</span>
              {#if d.serial}
                <span class="mono detected-meta">S/N {d.serial}</span>
              {/if}
              <span class="badge badge-{d.support}">{supportLabel(d)}</span>
              {#if d.status !== 'ok'}
                <span class="badge badge-status">{STATUS_LABEL[d.status] ?? d.status}</span>
              {/if}
              {#if d.bootloader}
                <span class="badge badge-status">Bootloader</span>
              {/if}
            </li>
          {/each}
        </ul>
        {#if (devices.excluded_count ?? 0) > 0}
          <p class="excluded-note">
            {devices.excluded_count} interfaces filtered (Bluetooth / driver problem)
          </p>
        {/if}
      </section>
    {/if}

    {#if devices.hubs.length === 0 && !devices.psu}
      <div class="card empty">
        <p>No Corsair devices detected on USB bus.</p>
      </div>
    {/if}
  {:else if !loading}
    <div class="card empty">
      <p>Unable to scan devices.</p>
    </div>
  {/if}
</div>

<style>
  .device-panel {
    display: flex;
    flex-direction: column;
  }
  .header-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
  }
  .section-title {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--text-secondary);
    margin: 0;
  }
  .psu-title {
    margin: 0 0 8px 0;
    font-size: 14px;
    font-weight: 600;
  }
  .psu-info {
    display: flex;
    gap: 8px;
    font-size: 12px;
  }
  .label {
    color: var(--text-muted);
  }
  .empty {
    text-align: center;
    padding: 40px;
    color: var(--text-secondary);
  }
  .notice-region .banner {
    margin-bottom: 12px;
  }
  .banner.notice-info {
    background: var(--bg-card-inset);
    border-color: var(--accent-dim);
    color: var(--text-secondary);
  }
  .detected {
    margin-top: 12px;
  }
  .detected-title {
    margin: 0 0 8px 0;
    font-size: 14px;
    font-weight: 600;
  }
  .detected-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .detected-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 10px;
    font-size: 12px;
  }
  .detected-name {
    font-weight: 600;
    color: var(--text-primary);
  }
  .detected-meta {
    font-family: var(--font-mono);
    color: var(--text-muted);
    font-size: 11px;
  }
  .badge {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    padding: 1px 6px;
    border-radius: 4px;
    border: 1px solid var(--text-muted);
    color: var(--text-muted);
  }
  .badge-supported {
    color: var(--success);
    border-color: var(--success);
  }
  .badge-detect_only,
  .badge-status {
    color: var(--warning);
    border-color: var(--warning);
  }
  .excluded-note {
    margin: 10px 0 0;
    font-size: 11px;
    color: var(--text-muted);
  }
  .header-actions {
    display: flex;
    gap: 6px;
  }
  .identify-btn {
    background: var(--accent);
    color: var(--bg-base);
    border: 1px solid var(--accent);
    font-weight: 600;
  }
  .identify-btn:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .identify-legend {
    margin-bottom: 16px;
    padding: 12px 16px;
    background: var(--bg-card-inset);
    border: 1px solid var(--accent-dim);
    border-radius: var(--radius);
  }
  .legend-intro {
    margin: 0 0 12px;
    font-size: 12px;
    color: var(--text-secondary);
    line-height: 1.4;
  }
  .legend-hub {
    margin-bottom: 10px;
  }
  .legend-hub:last-child {
    margin-bottom: 0;
  }
  .legend-hub-title {
    display: block;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    color: var(--text-muted);
    margin-bottom: 6px;
  }
  .legend-rows {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 4px 16px;
  }
  .legend-row {
    display: grid;
    grid-template-columns: 18px 40px 1fr auto;
    gap: 8px;
    align-items: center;
    font-size: 12px;
    color: var(--text-primary);
  }
  .legend-swatch {
    width: 18px;
    height: 18px;
    border-radius: 4px;
    border: 1px solid rgba(255, 255, 255, 0.2);
  }
  .legend-ch {
    font-weight: 600;
    font-family: var(--font-mono);
  }
  .legend-type {
    color: var(--text-muted);
    font-size: 11px;
  }
  .inline-swatch {
    display: inline-block;
    width: 12px;
    height: 12px;
    vertical-align: middle;
  }
  .marker-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
    color: var(--text-primary);
  }
  .marker-table th {
    text-align: left;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    color: var(--text-muted);
    padding: 2px 8px 4px 0;
  }
  .marker-table td {
    padding: 2px 8px 2px 0;
  }
  .marker-table .mono {
    font-family: var(--font-mono);
  }
  .marker-table .skipped {
    color: var(--text-muted);
  }
  .marker-file {
    margin: 10px 0 0;
    word-break: break-all;
  }
  .legend-color-name {
    color: var(--text-secondary);
    font-size: 11px;
    text-align: right;
  }
</style>
