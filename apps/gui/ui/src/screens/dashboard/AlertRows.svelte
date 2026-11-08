<!--
  Dashboard alert rows (inventory 1.1): hw error (raw string), emergency,
  stale, hub lost / recovered (recovered auto-dismisses after 5 s via the
  sensors store). Emergency/stale follow the live snapshot, not the scrub.
-->
<script lang="ts">
  import { flip } from 'svelte/animate';
  import { slide } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import Banner from '../../components/ui/Banner.svelte';
  import { sensors } from '../../lib/stores/sensors.svelte';
  import { reducedMotion } from '../../lib/motion.svelte';

  const live = $derived(sensors.snapshot);
  const dur = $derived(reducedMotion.current ? 0 : 320);

  interface Row {
    key: string;
    tone: 'error' | 'warning' | 'success';
    text: string;
  }
  const rows = $derived.by(() => {
    const out: Row[] = [];
    if (sensors.hwError) out.push({ key: 'hw', tone: 'error', text: sensors.hwError });
    if (live?.pump_stall) {
      const names = (live.pumps ?? []).map((p) => p.name).join(', ');
      out.push({ key: 'pump', tone: 'error', text: `PUMP STALL: ${names || 'the pump'} has stopped — all fans at 100%. Check the loop now.` });
    } else if (live?.emergency) {
      out.push({ key: 'emergency', tone: 'error', text: 'EMERGENCY: Critical temperature detected — all fans at 100%' });
    }
    if (live?.any_stale) out.push({ key: 'stale', tone: 'warning', text: 'Sensor data stale — failsafe mode active (70%)' });
    for (const hub of sensors.hubStatus) {
      if (hub.state === 'lost')
        out.push({
          key: `hub-${hub.serial}`,
          tone: 'warning',
          text: `Hub ${hub.serial.slice(0, 8)}... lost — attempting recovery...`,
        });
      else if (hub.state === 'recovered')
        out.push({ key: `hub-${hub.serial}`, tone: 'success', text: `Hub ${hub.serial.slice(0, 8)}... recovered` });
    }
    return out;
  });
</script>

{#if rows.length}
  <div class="alerts">
    {#each rows as r (r.key + r.tone)}
      <div animate:flip={{ duration: dur }} transition:slide={{ duration: dur, easing: cubicOut }}>
        <Banner tone={r.tone}>{r.text}</Banner>
      </div>
    {/each}
  </div>
{/if}

<style>
  .alerts {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-bottom: 24px;
  }
</style>
