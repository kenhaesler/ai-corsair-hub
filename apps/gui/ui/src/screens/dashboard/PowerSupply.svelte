<!--
  Power Supply breakdown (inventory 1.5), rendered as the last row of the
  Pinned "Power Supply" group, directly under the PSU total-output row (which
  is the single home of total W, its sentence and chart, and opens the power
  detail page). Breakdown: one pill per rail (name, W 0 dp, V 2 dp, A 1 dp)
  plus Input V and PSU fan RPM. VRM / Case temperatures are not repeated here:
  the PSU VRM / PSU CASE temperature rows are their single home.
-->
<script lang="ts">
  import { display } from '../../lib/stores/scrub.svelte';

  const psu = $derived(display.snapshot?.psu ?? null);
</script>

{#if psu}
  <div class="breakdown" aria-label="Power supply breakdown">
    <div class="rails">
      {#each psu.rails as r (r.name)}
        <div class="rail">
          <span class="rname">{r.name}</span>
          <span class="rpow num">{r.power.toFixed(0)}<span class="ru">W</span></span>
          <span class="rsub num">{r.voltage.toFixed(2)}V · {r.current.toFixed(1)}A</span>
        </div>
      {/each}
    </div>
    <dl class="chips">
      <div class="chip"><dt>Input</dt><dd class="num">{psu.input_voltage.toFixed(0)}V</dd></div>
      <div class="chip"><dt>Fan</dt><dd class="num">{Math.round(psu.fan_rpm)} RPM</dd></div>
    </dl>
  </div>
{/if}

<style>
  .breakdown {
    display: flex;
    align-items: stretch;
    gap: 12px;
    padding: 12px 20px 16px;
  }
  .rails {
    flex: 1;
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
  }
  .rail,
  .chip {
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 8px 12px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.04);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.04);
  }
  .rname,
  dt {
    font-size: 11.5px;
    font-weight: 600;
    color: var(--t3);
  }
  .rname {
    color: #ffe680;
  }
  .rpow,
  dd {
    margin: 0;
    font-size: 17px;
    line-height: 22px;
    font-weight: 600;
    letter-spacing: -0.01em;
    color: var(--t1);
  }
  .ru {
    margin-left: 2px;
    font-size: 12px;
    color: var(--t2);
  }
  .rsub {
    font-size: 11.5px;
    color: var(--t2);
  }
  .chips {
    margin: 0;
    flex: 0 0 auto;
    display: grid;
    grid-template-columns: repeat(2, minmax(96px, auto));
    gap: 8px;
  }
  .chip {
    justify-content: center;
  }
</style>
