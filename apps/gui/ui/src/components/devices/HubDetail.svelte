<script lang="ts">
  import type { HubSnapshot } from '../../lib/types';

  interface Props {
    hub: HubSnapshot;
  }

  let { hub }: Props = $props();
</script>

<div class="hub-detail">
  <div class="hub-header">
    <h4>iCUE LINK Hub</h4>
    <span class="meta mono">FW {hub.firmware}</span>
  </div>
  <div class="serial mono">{hub.serial}</div>

  <table class="device-table">
    <thead>
      <tr>
        <th>Ch</th>
        <th>Type</th>
        <th>Model</th>
        <th>ID</th>
        <th>RPM</th>
      </tr>
    </thead>
    <tbody>
      {#each hub.devices as device}
        <tr>
          <td class="tabular-nums">{device.channel}</td>
          <td>{device.device_type}</td>
          <td class="tabular-nums">0x{device.model.toString(16).padStart(2, '0').toUpperCase()}</td>
          <td class="mono">{device.device_id.slice(0, 12)}...</td>
          <td class="tabular-nums">{device.rpm ?? '—'}</td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .hub-detail {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .hub-header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
  }
  .hub-header h4 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
  }
  .meta {
    font-size: 11px;
    color: var(--text-muted);
  }
  .serial {
    font-size: 10px;
    color: var(--text-muted);
    word-break: break-all;
  }
  .device-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }
  th {
    text-align: left;
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--text-muted);
    padding: 4px 8px;
    border-bottom: 1px solid var(--border);
  }
  td {
    padding: 4px 8px;
    border-bottom: 1px solid rgba(255,255,255,0.03);
  }
</style>
