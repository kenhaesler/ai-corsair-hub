<script lang="ts">
  import type { RgbColor } from '../../lib/types';

  interface Props {
    color: RgbColor;
    onchange: (color: RgbColor) => void;
  }

  let { color, onchange }: Props = $props();

  let expanded = $state(false);

  const presets: RgbColor[] = [
    { r: 255, g: 255, b: 255 }, // white
    { r: 255, g: 0, b: 0 },     // red
    { r: 255, g: 140, b: 0 },   // orange
    { r: 255, g: 220, b: 50 },  // yellow
    { r: 52, g: 211, b: 153 },  // green
    { r: 0, g: 200, b: 255 },   // cyan
    { r: 96, g: 165, b: 250 },  // blue
    { r: 168, g: 85, b: 247 },  // purple
    { r: 255, g: 100, b: 180 }, // pink
  ];

  function hexFromColor(c: RgbColor): string {
    return '#' + [c.r, c.g, c.b].map(v => v.toString(16).padStart(2, '0')).join('');
  }

  function colorFromHex(hex: string): RgbColor | null {
    const m = hex.match(/^#?([0-9a-f]{6})$/i);
    if (!m) return null;
    return {
      r: parseInt(m[1].slice(0, 2), 16),
      g: parseInt(m[1].slice(2, 4), 16),
      b: parseInt(m[1].slice(4, 6), 16),
    };
  }

  function handleHexInput(e: Event) {
    const val = (e.target as HTMLInputElement).value;
    const c = colorFromHex(val);
    if (c) onchange(c);
  }

  function handleNative(e: Event) {
    const hex = (e.target as HTMLInputElement).value;
    const c = colorFromHex(hex);
    if (c) onchange(c);
  }
</script>

<div class="color-picker">
  <!-- The swatch is a label wrapping a hidden native <input type="color">.
       Clicking the swatch opens the OS-level colour picker overlay
       directly — no internal dropdown, so no z-index / overflow:hidden
       traps from the surrounding scrollable layer-panel. -->
  <label class="swatch-wrap" title="Click to change colour">
    <span
      class="swatch-btn"
      style="background: rgb({color.r},{color.g},{color.b})"
    ></span>
    <input
      type="color"
      value={hexFromColor(color)}
      oninput={handleNative}
      class="native-picker-hidden"
    />
  </label>

  <!-- Expand button shows the hex + presets panel for fine-tuning. -->
  <button
    class="expand-btn"
    onclick={() => { expanded = !expanded; }}
    title={expanded ? 'Close details' : 'Hex + presets'}
  >{expanded ? '×' : '…'}</button>

  {#if expanded}
    <div class="picker-dropdown">
      <div class="hex-row">
        <input
          type="text"
          value={hexFromColor(color)}
          onchange={handleHexInput}
          class="hex-input"
          placeholder="#RRGGBB"
        />
      </div>

      <div class="channel-row">
        <label>R<input type="number" min="0" max="255" value={color.r}
          onchange={(e) => onchange({ ...color, r: parseInt((e.target as HTMLInputElement).value) })} /></label>
        <label>G<input type="number" min="0" max="255" value={color.g}
          onchange={(e) => onchange({ ...color, g: parseInt((e.target as HTMLInputElement).value) })} /></label>
        <label>B<input type="number" min="0" max="255" value={color.b}
          onchange={(e) => onchange({ ...color, b: parseInt((e.target as HTMLInputElement).value) })} /></label>
      </div>

      <div class="presets">
        {#each presets as preset}
          <button
            class="preset-swatch"
            style="background: rgb({preset.r},{preset.g},{preset.b})"
            onclick={() => onchange(preset)}
            aria-label="Use preset colour"
          ></button>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .color-picker {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .swatch-wrap {
    position: relative;
    display: inline-block;
    cursor: pointer;
    line-height: 0;
  }
  .swatch-btn {
    display: inline-block;
    width: 24px;
    height: 24px;
    border-radius: 4px;
    border: 1px solid rgba(255,255,255,0.15);
    box-sizing: border-box;
  }
  .swatch-wrap:hover .swatch-btn {
    border-color: rgba(255,255,255,0.3);
  }
  /* Hide the native colour input but keep it clickable on top of the
     swatch — the label wrapper makes the whole swatch a trigger. */
  .native-picker-hidden {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    opacity: 0;
    cursor: pointer;
    border: none;
    padding: 0;
  }
  .expand-btn {
    width: 20px;
    height: 20px;
    padding: 0;
    background: transparent;
    border: 1px solid var(--border);
    border-radius: 3px;
    color: var(--text-muted);
    cursor: pointer;
    font-size: 11px;
    line-height: 1;
  }
  .expand-btn:hover {
    color: var(--accent);
    border-color: var(--accent-dim);
  }
  .picker-dropdown {
    position: fixed;
    z-index: 1000;
    /* Positioned via JS on open so we don't get clipped by overflow
       scrolling ancestors. Default centre on screen via transform. */
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    padding: 10px;
    background: rgba(30, 30, 30, 0.98);
    backdrop-filter: blur(20px);
    border: 1px solid var(--border-bright);
    border-radius: var(--radius);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 180px;
  }
  .hex-input {
    width: 100%;
    font-family: var(--font-mono);
    font-size: 11px;
    text-align: center;
  }
  .hex-row {
    display: flex;
  }
  .channel-row {
    display: flex;
    gap: 4px;
  }
  .channel-row label {
    display: flex;
    align-items: center;
    gap: 2px;
    font-size: 10px;
    color: var(--text-muted);
  }
  .channel-row input {
    width: 40px;
    font-size: 10px;
    padding: 2px 4px;
  }
  .presets {
    display: flex;
    gap: 3px;
    flex-wrap: wrap;
  }
  .preset-swatch {
    width: 18px;
    height: 18px;
    border-radius: 3px;
    border: 1px solid rgba(255,255,255,0.1);
    cursor: pointer;
    padding: 0;
  }
  .preset-swatch:hover {
    border-color: rgba(255,255,255,0.4);
    transform: scale(1.15);
  }
</style>
