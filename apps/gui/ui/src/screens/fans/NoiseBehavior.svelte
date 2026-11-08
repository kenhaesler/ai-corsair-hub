<!--
  NoiseBehavior (inventory 2.4): collapsible, summary "hy N° · ↑X%/s · ↓Y%/s"
  (symmetric: Y = round(ramp_up × 0.4, 1 dp)), shown only while
  collapsed: when open the slider readouts are the single home of each value. Hysteresis 0–8 °C, Ramp Up
  1–20 %/s, "Asymmetric ramp-down" enabling Ramp Down 0.5–20 %/s. Labels
  update live; values commit on release.
-->
<script lang="ts">
  import Disclosure from '../../components/ui/Disclosure.svelte';
  import Slider from '../../components/ui/Slider.svelte';
  import Checkbox from '../../components/ui/Checkbox.svelte';

  interface Props {
    hysteresis: number;
    rampUp: number;
    rampDown: number | null;
    /** °C per 100 W of power rise; 0 = off */
    feedForward?: number;
    color?: string;
    open?: boolean;
    onchange: (v: { hysteresis: number; rampUp: number; rampDown: number | null; feedForward: number }) => void;
  }
  let { hysteresis, rampUp, rampDown, feedForward = 0, color = 'var(--accent)', open = $bindable(false), onchange }: Props = $props();
  let draftFf = $state(0);
  $effect(() => {
    draftFf = feedForward;
  });

  let draftHyst = $state(0);
  let draftUp = $state(5);
  let draftDown = $state(2);
  let asymmetric = $state(false);

  $effect(() => {
    draftHyst = hysteresis;
  });
  $effect(() => {
    draftUp = rampUp;
  });
  $effect(() => {
    if (rampDown != null) {
      draftDown = rampDown;
      asymmetric = true;
    } else {
      draftDown = Math.round(rampUp * 0.4 * 10) / 10;
      asymmetric = false;
    }
  });

  const symDown = $derived(Math.round(draftUp * 0.4 * 10) / 10);
  const summary = $derived(
    `hy ${draftHyst}° · ↑${draftUp}%/s · ↓${asymmetric ? draftDown : symDown}%/s${draftFf > 0 ? ` · power +${draftFf}°/100 W` : ''}`,
  );

  function emit(over: Partial<{ hysteresis: number; rampUp: number; rampDown: number | null; feedForward: number }> = {}) {
    onchange({
      hysteresis: draftHyst,
      rampUp: draftUp,
      rampDown: asymmetric ? draftDown : null,
      feedForward: draftFf,
      ...over,
    });
  }
  function toggleAsym(on: boolean) {
    asymmetric = on;
    if (on && rampDown == null) draftDown = symDown;
    emit({ rampDown: on ? draftDown : null });
  }

</script>

<div class="noise">
  <Disclosure title="Noise Behavior" subtitle={open ? undefined : summary} bind:open>
    <div class="body">
      <div class="param">
        <Slider
          label="Hysteresis"
          min={0}
          max={8}
          step={1}
          {color}
          bind:value={draftHyst}
          format={(v) => `${v}°C`}
          oncommit={(v) => emit({ hysteresis: v })}
        />
        <p class="hint">Temperature changes smaller than this are ignored. Larger = quieter, smaller = faster response.</p>
      </div>

      <div class="param">
        <Slider
          label="Ramp Up"
          min={1}
          max={20}
          step={1}
          {color}
          bind:value={draftUp}
          format={(v) => `${v}%/s`}
          oncommit={(v) => emit({ rampUp: v })}
        />
        <p class="hint">How fast fans accelerate when temps climb.</p>
      </div>

      <div class="param">
        <Slider
          label="Power anticipation"
          min={0}
          max={10}
          step={0.5}
          {color}
          bind:value={draftFf}
          format={(v) => (v > 0 ? `+${v}° / 100 W` : 'Off')}
          oncommit={(v) => emit({ feedForward: v })}
        />
        <p class="hint">
          Reads the curve a few degrees higher while CPU+GPU power sits above its one-minute average, so fans start before the coolant warms. 0 = off.
        </p>
      </div>

      <div class="param">
        <div class="asym">
          <Checkbox checked={asymmetric} onchange={toggleAsym}>Asymmetric ramp-down</Checkbox>
        </div>
        <Slider
          label="Ramp Down"
          hideLabel
          min={0.5}
          max={20}
          step={0.5}
          {color}
          disabled={!asymmetric}
          bind:value={draftDown}
          format={(v) => `${asymmetric ? v : symDown}%/s`}
          oncommit={(v) => emit({ rampDown: v })}
        />
        <p class="hint">
          {#if asymmetric}
            How fast fans decelerate. Typically slower than ramp-up so fall-off is inaudible.
          {:else}
            Symmetric mode: backend uses ramp-up × 0.4 automatically. Enable to tune independently.
          {/if}
        </p>
      </div>

    </div>
  </Disclosure>
</div>

<style>
  .noise {
    padding: 4px 18px 6px;
  }
  .noise :global(.trigger .t-headline) {
    font: 500 11px var(--font-mono);
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--t2);
  }
  .noise :global(.trigger) {
    min-height: 36px;
  }
  .body {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 20px 28px;
    padding: 8px 0 12px 24px;
  }
  .param {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .asym {
    height: 16px;
    display: flex;
    align-items: center;
    margin-bottom: -2px;
  }
  .asym :global(.cb) {
    font-size: 12px;
    font-weight: 500;
    color: var(--t2);
    gap: 8px;
  }
  .asym :global(.box) {
    width: 15px;
    height: 15px;
    border-radius: 4px;
  }
  .hint {
    font-size: 12px;
    line-height: 17px;
    color: var(--t3);
    max-width: 44ch;
  }
  @media (max-width: 1180px) {
    .body {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
</style>
