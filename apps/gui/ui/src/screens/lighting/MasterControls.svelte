<!--
  Master controls for the toolbar's trailing slot (inventory 4.2): master
  brightness 0–100 "N%" (saves on input), "Hardware" → hardware_output,
  "Enabled" → setRgbEnabled then save, read-only "N FPS" badge, and a
  "Saving..." capsule that drops below the toolbar while a save is in
  flight (it takes no toolbar space, so nothing shifts).
  Windows 1600 px and wider show everything inline; narrower ones collapse into a
  summary pill ("☀ 80% · On · 30 FPS") that opens the same controls in a
  popover, Control Center style.
-->
<script lang="ts">
  import { onMount, tick } from 'svelte';
  import Sun from 'lucide-svelte/icons/sun';
  import ChevronDown from 'lucide-svelte/icons/chevron-down';
  import Toggle from '../../components/ui/Toggle.svelte';
  import Badge from '../../components/ui/Badge.svelte';
  import Spinner from '../../components/ui/Spinner.svelte';
  import { portal } from '../../lib/dom';
  import { createSpring, springTo, springs } from '../../lib/motion.svelte';
  import { rgb } from '../../lib/stores/rgb.svelte';
  import { lighting, persist, setEnabled } from './state.svelte';

  const cfg = $derived(rgb.config);
  let bright = $state(80);
  $effect(() => {
    if (cfg) bright = cfg.brightness;
  });
  const busy = $derived(rgb.saving || lighting.pending > 0);

  // "Saving..." stays up for a beat so quick saves read as a state, not a flicker
  let showSaving = $state(false);
  let hideTimer: ReturnType<typeof setTimeout> | null = null;
  $effect(() => {
    if (busy) {
      if (hideTimer) clearTimeout(hideTimer);
      hideTimer = null;
      showSaving = true;
    } else if (showSaving && !hideTimer) {
      hideTimer = setTimeout(() => {
        showSaving = false;
        hideTimer = null;
      }, 700);
    }
  });
  const sv = createSpring(0, springs.sheet);
  $effect(() => {
    springTo(sv, showSaving ? 1 : 0);
  });

  function onBright(e: Event) {
    const v = Number((e.currentTarget as HTMLInputElement).value);
    bright = v;
    if (!rgb.config) return;
    rgb.config.brightness = v;
    persist();
  }
  function setHardware(v: boolean) {
    if (rgb.config) rgb.config.hardware_output = v;
    persist();
  }

  // ---- wide vs compact ----
  let wide = $state(true);
  onMount(() => {
    const mq = window.matchMedia('(min-width: 1600px)');
    wide = mq.matches;
    const on = (e: MediaQueryListEvent) => (wide = e.matches);
    mq.addEventListener('change', on);
    return () => mq.removeEventListener('change', on);
  });

  // ---- compact popover ----
  let open = $state(false);
  let mounted = $state(false);
  let pill = $state<HTMLButtonElement>();
  let pop = $state<HTMLDivElement>();
  let pos = $state({ right: 16, top: 90 });
  const pp = createSpring(0, { ...springs.sheet, precision: 0.002 });
  async function show() {
    const r = pill!.getBoundingClientRect();
    pos = { right: Math.max(12, window.innerWidth - r.right), top: r.bottom + 8 };
    open = mounted = true;
    springTo(pp, 1);
    await tick();
    pop?.querySelector<HTMLElement>('input, button')?.focus({ preventScroll: true });
  }
  function hide(restore = true) {
    if (!open) return;
    open = false;
    springTo(pp, 0).then(() => {
      if (!open) mounted = false;
    });
    if (restore) pill?.focus({ preventScroll: true });
  }
  $effect(() => {
    if (wide && open) hide(false);
  });
  function outside(e: PointerEvent) {
    if (!open) return;
    const t = e.target as Node;
    if (pop?.contains(t) || pill?.contains(t)) return;
    hide(false);
  }
</script>

<svelte:window onpointerdown={outside} />

{#snippet brightness(w: number)}
  <label class="bright" title="Master brightness">
    <input
      type="range"
      min="0"
      max="100"
      step="1"
      value={bright}
      aria-label="Master brightness"
      aria-valuetext="{bright}%"
      style:--pct="{bright}%"
      style:width="{w}px"
      oninput={onBright}
    />
    <span class="pct num">{bright}%</span>
  </label>
{/snippet}

{#if cfg}
  <div class="master">
    {#if wide}
      <Sun size={14} strokeWidth={2.25} class="sun" aria-hidden="true" />
      {@render brightness(84)}
      <span class="tg"><Toggle size="sm" label="Hardware" showLabel checked={cfg.hardware_output} onchange={setHardware} /></span>
      <span class="tg"><Toggle size="sm" label="Enabled" showLabel checked={cfg.enabled} onchange={(v) => setEnabled(v)} /></span>
      <Badge size="sm" title="Render rate"><span class="num">{cfg.fps} FPS</span></Badge>
    {:else}
      <button
        bind:this={pill}
        type="button"
        class="pill"
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-label="Lighting master controls: {bright}% brightness, {cfg.enabled ? 'enabled' : 'off'}, {cfg.fps} FPS"
        onclick={() => (open ? hide() : show())}
      >
        <Sun size={14} strokeWidth={2.25} aria-hidden="true" />
        <span class="num">{bright}%</span>
        <span class="dot" class:on={cfg.enabled} aria-hidden="true"></span>
        <span class="pstate">{cfg.enabled ? 'On' : 'Off'}</span>
        <span class="pfps num">{cfg.fps} FPS</span>
        <ChevronDown size={13} strokeWidth={2.25} aria-hidden="true" />
      </button>
    {/if}

    {#if sv.current > 0.01}
      <span
        class="saving"
        role="status"
        style:opacity={Math.min(1, sv.current)}
        style:transform="translateY({(1 - sv.current) * -6}px) scale({0.96 + 0.04 * sv.current})"
      >
        <Spinner size={11} /><span>Saving...</span>
      </span>
    {/if}
    <span class="sr-only" aria-live="polite">{showSaving ? 'Saving...' : ''}</span>
  </div>
{/if}

{#if mounted && cfg}
  <div
    use:portal
    bind:this={pop}
    class="pop"
    role="dialog"
    aria-label="Lighting master controls"
    tabindex="-1"
    onkeydown={(e) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        hide();
      }
    }}
    style:right="{pos.right}px"
    style:top="{pos.top}px"
    style:opacity={Math.min(1, pp.current * 1.4)}
    style:transform="scale({0.96 + 0.04 * pp.current}) translateY({(1 - pp.current) * -6}px)"
    style:pointer-events={open ? 'auto' : 'none'}
  >
    <div class="prow">
      <span class="plab">Master brightness</span>
    </div>
    <div class="prow slider-row">
      <Sun size={14} strokeWidth={2.25} aria-hidden="true" />
      {@render brightness(176)}
    </div>
    <div class="psep" aria-hidden="true"></div>
    <div class="prow">
      <span class="ptext"><span class="pname">Hardware</span><span class="pdesc">Send frames to the hubs</span></span>
      <Toggle size="sm" label="Hardware" checked={cfg.hardware_output} onchange={setHardware} />
    </div>
    <div class="prow">
      <span class="ptext"><span class="pname">Enabled</span><span class="pdesc">Every zone on or off</span></span>
      <Toggle size="sm" label="Enabled" checked={cfg.enabled} onchange={(v) => setEnabled(v)} />
    </div>
    <div class="psep" aria-hidden="true"></div>
    <div class="prow">
      <span class="pname">Render rate</span>
      <Badge size="sm"><span class="num">{cfg.fps} FPS</span></Badge>
    </div>
  </div>
{/if}

<style>
  .master {
    position: relative;
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .master :global(.sun) {
    color: var(--t2);
    flex: none;
  }
  .tg :global(.tg) {
    gap: 7px;
  }
  .bright {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--t2);
  }
  .bright input {
    -webkit-appearance: none;
    appearance: none;
    height: 20px;
    margin: 0;
    background: transparent;
    cursor: pointer;
  }
  .bright input::-webkit-slider-runnable-track {
    height: 4px;
    border-radius: 999px;
    background: linear-gradient(
      to right,
      var(--accent) 0%,
      var(--accent) var(--pct),
      rgba(255, 255, 255, 0.14) var(--pct),
      rgba(255, 255, 255, 0.14) 100%
    );
  }
  .bright input::-moz-range-track {
    height: 4px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.14);
  }
  .bright input::-moz-range-progress {
    height: 4px;
    border-radius: 999px;
    background: var(--accent);
  }
  .bright input::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 16px;
    height: 16px;
    margin-top: -6px;
    border-radius: 50%;
    background: #f4f6fb;
    box-shadow: var(--sh-knob);
    transition: transform var(--dur-press) ease-out;
  }
  .bright input::-moz-range-thumb {
    width: 16px;
    height: 16px;
    border: 0;
    border-radius: 50%;
    background: #f4f6fb;
    box-shadow: var(--sh-knob);
  }
  .bright input:active::-webkit-slider-thumb {
    transform: scale(1.08);
  }
  .bright input:focus-visible {
    box-shadow: none;
  }
  .bright input:focus-visible::-webkit-slider-thumb {
    box-shadow:
      var(--sh-knob),
      var(--focus-ring);
  }
  .pct {
    width: 32px;
    font-size: 12px;
    font-weight: 600;
    color: var(--t1);
    text-align: right;
  }

  .pill {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 30px;
    padding: 0 10px 0 11px;
    border: 0;
    border-radius: 999px;
    background: var(--mat-raised);
    box-shadow:
      inset 0 0 0 1px var(--hair),
      var(--hair-inner);
    color: var(--t1);
    font-size: 12px;
    font-weight: 600;
    white-space: nowrap;
    cursor: pointer;
    transition:
      background-color var(--dur-fast) ease-out,
      transform var(--dur-press) ease-out;
  }
  .pill:hover {
    background: var(--mat-raised-hover);
  }
  .pill:active {
    transform: scale(0.97);
  }
  .pill[aria-expanded='true'] {
    background: var(--mat-pressed);
  }
  .pill :global(svg) {
    color: var(--t2);
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--t4);
  }
  .dot.on {
    background: var(--success);
    box-shadow: 0 0 6px var(--success);
  }
  .pstate {
    color: var(--t2);
    font-weight: 500;
  }
  .pfps {
    padding-left: 7px;
    box-shadow: inset 1px 0 0 var(--hair-strong);
    color: var(--t2);
    font-weight: 500;
  }

  .saving {
    position: absolute;
    top: calc(100% + 10px);
    right: 0;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 24px;
    padding: 0 10px;
    border-radius: 999px;
    background: var(--mat-sheet);
    -webkit-backdrop-filter: var(--mat-blur);
    backdrop-filter: var(--mat-blur);
    box-shadow:
      0 0 0 1px var(--hair),
      var(--sh-1);
    font-size: 12px;
    font-weight: 500;
    color: var(--t2);
    white-space: nowrap;
    pointer-events: none;
    transform-origin: top right;
  }

  .pop {
    position: fixed;
    z-index: 200;
    width: 280px;
    padding: 8px 0;
    border-radius: 16px;
    background: var(--mat-sheet);
    -webkit-backdrop-filter: var(--mat-blur);
    backdrop-filter: var(--mat-blur);
    box-shadow:
      var(--sh-pop),
      inset 0 1px 0 rgba(255, 255, 255, 0.06);
    transform-origin: top right;
  }
  .prow {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 40px;
    padding: 4px 16px;
  }
  .prow.slider-row {
    justify-content: flex-start;
    min-height: 32px;
    padding-top: 0;
    color: var(--t2);
  }
  .plab {
    font-size: 12px;
    font-weight: 600;
    color: var(--t2);
  }
  .ptext {
    display: flex;
    flex-direction: column;
  }
  .pname {
    font-size: 13px;
    line-height: 18px;
    font-weight: 500;
    color: var(--t1);
  }
  .pdesc {
    font-size: 12px;
    line-height: 16px;
    color: var(--t3);
  }
  .psep {
    height: 1px;
    margin: 4px 16px;
    background: var(--hair);
  }
</style>
