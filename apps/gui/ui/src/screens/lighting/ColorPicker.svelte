<!--
  ColorPicker (inventory 4.11). A swatch button opens a custom popover:
  saturation/value field + hue slider (pointer and keyboard), hex input
  (#RRGGBB, applied when valid), R/G/B inputs 0–255, and 9 preset swatches
  (white, red, orange, yellow, green, cyan, blue, purple, pink). Changes
  apply live. Rendered in a portal so scroll containers never clip it;
  Escape or a click outside closes it and focus returns to the swatch.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import { portal } from '../../lib/dom';
  import { createSpring, springTo, springs } from '../../lib/motion.svelte';
  import type { RgbColor } from '../../lib/types';
  import { COLOR_PRESETS, colorFromHex, hexFromColor, hsvToRgb, rgbCss, rgbToHsv } from './effects';

  interface Props {
    color: RgbColor;
    label: string;
    onchange: (c: RgbColor) => void;
    size?: 'sm' | 'md';
  }
  let { color, label, onchange, size = 'md' }: Props = $props();

  let open = $state(false);
  let mounted = $state(false);
  let btn: HTMLButtonElement;
  let pop = $state<HTMLDivElement>();
  let pos = $state({ x: 0, y: 0, above: false });
  const p = createSpring(0, { ...springs.sheet, precision: 0.002 });
  const uid = `cp-${Math.random().toString(36).slice(2, 8)}`;

  // HSV is kept locally so hue survives at s=0 / v=0.
  let hsv = $state({ h: 0, s: 0, v: 1 });
  let hexDraft = $state('');
  let hexInvalid = $state(false);

  function syncFrom(c: RgbColor) {
    const next = rgbToHsv(c);
    const same = hsvToRgb(hsv.h, hsv.s, hsv.v);
    if (same.r !== c.r || same.g !== c.g || same.b !== c.b) {
      hsv = { h: next.s === 0 || next.v === 0 ? hsv.h : next.h, s: next.s, v: next.v };
    }
    hexDraft = hexFromColor(c);
    hexInvalid = false;
  }
  $effect(() => {
    const c = color;
    if (open) syncFrom(c);
  });

  function place() {
    const r = btn.getBoundingClientRect();
    const W = 264;
    const H = 356;
    const x = Math.min(window.innerWidth - W - 12, Math.max(12, r.left + r.width / 2 - W / 2));
    const below = r.bottom + 8 + H < window.innerHeight - 12;
    pos = { x, y: below ? r.bottom + 8 : r.top - 8 - H, above: !below };
  }

  async function show() {
    syncFrom(color);
    place();
    open = true;
    mounted = true;
    springTo(p, 1);
    await tick();
    pop?.querySelector<HTMLElement>('[data-autofocus]')?.focus({ preventScroll: true });
  }
  function hide(restore = true) {
    if (!open) return;
    open = false;
    springTo(p, 0).then(() => {
      if (!open) mounted = false;
    });
    if (restore) btn?.focus({ preventScroll: true });
  }

  function emit(c: RgbColor) {
    hexDraft = hexFromColor(c);
    hexInvalid = false;
    onchange(c);
  }
  function setHsv(h: number, s: number, v: number) {
    hsv = { h, s: Math.min(1, Math.max(0, s)), v: Math.min(1, Math.max(0, v)) };
    emit(hsvToRgb(hsv.h, hsv.s, hsv.v));
  }

  // --- SV field ---
  let field = $state<HTMLDivElement>();
  function svFromPointer(e: PointerEvent) {
    const r = field!.getBoundingClientRect();
    setHsv(hsv.h, (e.clientX - r.left) / r.width, 1 - (e.clientY - r.top) / r.height);
  }
  function svDown(e: PointerEvent) {
    field!.setPointerCapture(e.pointerId);
    svFromPointer(e);
  }
  function svMove(e: PointerEvent) {
    if (field!.hasPointerCapture(e.pointerId)) svFromPointer(e);
  }
  function svKey(e: KeyboardEvent) {
    const d = e.shiftKey ? 0.1 : 0.02;
    let { s, v } = hsv;
    if (e.key === 'ArrowLeft') s -= d;
    else if (e.key === 'ArrowRight') s += d;
    else if (e.key === 'ArrowUp') v += d;
    else if (e.key === 'ArrowDown') v -= d;
    else return;
    e.preventDefault();
    setHsv(hsv.h, s, v);
  }

  function onHex(v: string) {
    hexDraft = v;
    const c = colorFromHex(v);
    hexInvalid = !c && v.replace('#', '').length >= 6;
    if (c) {
      const n = rgbToHsv(c);
      hsv = { h: n.s === 0 ? hsv.h : n.h, s: n.s, v: n.v };
      onchange(c);
    }
  }
  function onChannel(k: 'r' | 'g' | 'b', raw: string) {
    const n = Math.round(Number(raw));
    if (!Number.isFinite(n) || raw.trim() === '') return;
    const c = { ...color, [k]: Math.min(255, Math.max(0, n)) };
    const h = rgbToHsv(c);
    hsv = { h: h.s === 0 ? hsv.h : h.h, s: h.s, v: h.v };
    emit(c);
  }

  function outside(e: PointerEvent) {
    if (!open) return;
    const t = e.target as Node;
    if (pop?.contains(t) || btn?.contains(t)) return;
    hide(false);
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      hide();
    }
  }

  const hueColor = $derived(rgbCss(hsvToRgb(hsv.h, 1, 1)));
  const cur = $derived(rgbCss(color));
  const isPreset = (c: RgbColor) => c.r === color.r && c.g === color.g && c.b === color.b;
</script>

<svelte:window onpointerdown={outside} onresize={() => open && place()} />

<button
  bind:this={btn}
  type="button"
  class="swatch {size}"
  style:--c={cur}
  aria-label="{label}: {hexFromColor(color)}"
  aria-haspopup="dialog"
  aria-expanded={open}
  aria-controls={open ? uid : undefined}
  title="{label} · {hexFromColor(color)}"
  onclick={() => (open ? hide() : show())}
>
  <span class="fill"></span>
</button>

{#if mounted}
  <div
    use:portal
    bind:this={pop}
    id={uid}
    class="pop"
    role="dialog"
    aria-label="{label} colour"
    tabindex="-1"
    onkeydown={onKey}
    style:left="{pos.x}px"
    style:top="{pos.y}px"
    style:opacity={Math.min(1, p.current * 1.4)}
    style:transform="scale({0.96 + 0.04 * p.current}) translateY({(1 - p.current) * (pos.above ? 6 : -6)}px)"
    style:transform-origin={pos.above ? 'bottom center' : 'top center'}
    style:pointer-events={open ? 'auto' : 'none'}
  >
    <div
      bind:this={field}
      class="sv"
      style:--hue={hueColor}
      role="slider"
      tabindex="0"
      aria-label="Saturation and brightness"
      aria-valuetext="Saturation {Math.round(hsv.s * 100)}%, brightness {Math.round(hsv.v * 100)}%"
      aria-valuenow={Math.round(hsv.s * 100)}
      data-autofocus
      onpointerdown={svDown}
      onpointermove={svMove}
      onkeydown={svKey}
    >
      <span class="knob" style:left="{hsv.s * 100}%" style:top="{(1 - hsv.v) * 100}%" style:background={cur}></span>
    </div>

    <div class="hue-row">
      <span class="preview" style:background={cur} aria-hidden="true"></span>
      <input
        class="hue"
        type="range"
        min="0"
        max="359"
        step="1"
        aria-label="Hue"
        value={Math.round(hsv.h)}
        style:--hue={hueColor}
        oninput={(e) => setHsv(Number(e.currentTarget.value), hsv.s || 1, hsv.v || 1)}
      />
    </div>

    <div class="fields">
      <label class="hex">
        <span class="flab">Hex</span>
        <input
          class="mono"
          class:invalid={hexInvalid}
          type="text"
          spellcheck="false"
          autocomplete="off"
          maxlength="7"
          value={hexDraft}
          aria-invalid={hexInvalid}
          oninput={(e) => onHex(e.currentTarget.value)}
          onblur={() => {
            hexDraft = hexFromColor(color);
            hexInvalid = false;
          }}
        />
      </label>
      {#each ['r', 'g', 'b'] as const as k (k)}
        <label class="ch">
          <span class="flab">{k.toUpperCase()}</span>
          <input
            class="num"
            type="number"
            min="0"
            max="255"
            step="1"
            inputmode="numeric"
            value={color[k]}
            onchange={(e) => onChannel(k, e.currentTarget.value)}
          />
        </label>
      {/each}
    </div>

    <div class="presets" role="group" aria-label="Preset colours">
      {#each COLOR_PRESETS as pr (pr.name)}
        <button
          type="button"
          class="pre"
          class:on={isPreset(pr.color)}
          style:--c={rgbCss(pr.color)}
          aria-label={pr.name}
          aria-pressed={isPreset(pr.color)}
          title={pr.name}
          onclick={() => {
            const h = rgbToHsv(pr.color);
            hsv = { h: h.s === 0 ? hsv.h : h.h, s: h.s, v: h.v };
            emit(pr.color);
          }}
        ></button>
      {/each}
    </div>
  </div>
{/if}

<style>
  .swatch {
    position: relative;
    flex: none;
    width: 44px;
    height: 26px;
    padding: 3px;
    border: 0;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.06);
    box-shadow:
      inset 0 0 0 1px var(--hair-strong),
      var(--hair-inner);
    cursor: pointer;
    transition:
      transform var(--dur-press) ease-out,
      background-color var(--dur-fast) ease-out;
  }
  .swatch.sm {
    width: 34px;
    height: 22px;
    border-radius: 7px;
  }
  .swatch:hover {
    background: rgba(255, 255, 255, 0.1);
  }
  .swatch:active {
    transform: scale(0.95);
  }
  .swatch[aria-expanded='true'] {
    box-shadow:
      inset 0 0 0 1px var(--accent),
      var(--focus-ring);
  }
  .fill {
    display: block;
    width: 100%;
    height: 100%;
    border-radius: 5px;
    background: var(--c);
    box-shadow:
      inset 0 0 0 1px rgb(0 0 0 / 0.25),
      0 0 12px -2px var(--c);
  }
  .swatch.sm .fill {
    border-radius: 4px;
  }

  .pop {
    position: fixed;
    z-index: 200;
    width: 264px;
    padding: 12px;
    border-radius: 16px;
    background: var(--mat-sheet);
    -webkit-backdrop-filter: var(--mat-blur);
    backdrop-filter: var(--mat-blur);
    box-shadow:
      var(--sh-pop),
      inset 0 1px 0 rgba(255, 255, 255, 0.06);
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .sv {
    position: relative;
    height: 152px;
    border-radius: 10px;
    background:
      linear-gradient(to top, #000, transparent),
      linear-gradient(to right, #fff, var(--hue));
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.1);
    cursor: crosshair;
    touch-action: none;
  }
  .sv:focus-visible {
    box-shadow:
      inset 0 0 0 1px rgba(255, 255, 255, 0.1),
      var(--focus-ring);
  }
  .knob {
    position: absolute;
    width: 18px;
    height: 18px;
    margin: -9px 0 0 -9px;
    border-radius: 50%;
    box-shadow:
      0 0 0 2.5px #fff,
      0 0 0 3.5px rgb(0 0 0 / 0.35),
      0 3px 8px rgb(0 0 0 / 0.5);
    pointer-events: none;
  }
  .hue-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .preview {
    flex: none;
    width: 28px;
    height: 28px;
    border-radius: 8px;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.14);
  }
  .hue {
    -webkit-appearance: none;
    appearance: none;
    flex: 1;
    height: 14px;
    margin: 0;
    border-radius: 999px;
    background: linear-gradient(90deg, #f00, #ff0 17%, #0f0 33%, #0ff 50%, #00f 67%, #f0f 83%, #f00);
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.25);
    cursor: pointer;
  }
  .hue::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--hue);
    box-shadow:
      0 0 0 2.5px #fff,
      0 2px 6px rgb(0 0 0 / 0.5);
  }
  .hue::-moz-range-thumb {
    width: 16px;
    height: 16px;
    border: 2.5px solid #fff;
    border-radius: 50%;
    background: var(--hue);
  }
  .hue:focus-visible {
    box-shadow: var(--focus-ring);
  }
  .fields {
    display: grid;
    grid-template-columns: 1.7fr 1fr 1fr 1fr;
    gap: 6px;
  }
  .fields label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .flab {
    font-size: 11px;
    line-height: 14px;
    font-weight: 600;
    color: var(--t3);
    padding-left: 2px;
  }
  .fields input {
    width: 100%;
    height: 28px;
    padding: 0 8px;
    border: 0;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.06);
    box-shadow: inset 0 0 0 1px var(--hair);
    font-size: 12px;
    color: var(--t1);
    -moz-appearance: textfield;
    appearance: textfield;
  }
  .fields input::-webkit-inner-spin-button,
  .fields input::-webkit-outer-spin-button {
    -webkit-appearance: none;
    margin: 0;
  }
  .fields input:focus-visible {
    box-shadow:
      inset 0 0 0 1px var(--accent),
      var(--focus-ring);
  }
  .fields input.invalid {
    box-shadow: inset 0 0 0 1px var(--error);
  }
  .presets {
    display: grid;
    grid-template-columns: repeat(9, 1fr);
    gap: 6px;
  }
  .pre {
    aspect-ratio: 1;
    border: 0;
    padding: 0;
    border-radius: 50%;
    background: var(--c);
    box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.3);
    cursor: pointer;
    transition: transform var(--dur-press) ease-out;
  }
  .pre:hover {
    transform: scale(1.12);
  }
  .pre:active {
    transform: scale(0.94);
  }
  .pre.on {
    box-shadow:
      0 0 0 2px var(--bg-0),
      0 0 0 3.5px var(--t1);
  }
</style>
