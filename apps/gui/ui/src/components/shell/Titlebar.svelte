<!--
  Titlebar (inventory 0.1): 32px frameless chrome with a drag region,
  "Corsair Hub", Windows 11 caption buttons and, while scrubbing, the centred time pill ("−1:42 · 14:03:21").
-->
<script lang="ts">
  import { appWindow } from '../../lib/api';
  import { toasts } from '../../lib/stores/toasts.svelte';
  import { display, scrub } from '../../lib/stores/scrub.svelte';
  import { fmtClock, fmtOffset } from '../../lib/format';
  import { createSpring, springTo, springs } from '../../lib/motion.svelte';

  /** Window commands are IPC calls too: surface a failure instead of an unhandled rejection. */
  function windowCmd(cmd: () => Promise<void>) {
    cmd().catch((e) => toasts.error('Window command failed', { detail: String(e) }));
  }

  const drop = createSpring(0, springs.sheet);
  $effect(() => {
    springTo(drop, scrub.active ? 1 : 0);
  });
</script>

<header class="titlebar" data-tauri-drag-region>
  <div class="left" data-tauri-drag-region>
    <svg class="mark" viewBox="0 0 16 16" aria-hidden="true">
      <defs>
        <linearGradient id="tb-mark" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stop-color="#9DB4FF" />
          <stop offset="1" stop-color="#6F7DF2" />
        </linearGradient>
      </defs>
      <rect x="0.5" y="0.5" width="15" height="15" rx="4.5" fill="url(#tb-mark)" />
      <path d="M3.5 9.2h2.1l1.3-3.4 2.1 5.4 1.3-2h2.2" fill="none" stroke="#0B1230" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
    </svg>
    <span class="title" data-tauri-drag-region>Corsair Hub</span>
  </div>

  <div class="center" data-tauri-drag-region>
    {#if drop.current > 0.01}
      <button
        type="button"
        class="pill"
        style:opacity={Math.min(1, drop.current)}
        style:transform="translateY({(1 - drop.current) * -14}px) scale({0.92 + 0.08 * drop.current})"
        onclick={() => scrub.release()}
        title="Return to live (Esc)"
        aria-live="polite"
      >
        <span class="rew" aria-hidden="true"></span>
        <span class="num">−{fmtOffset(display.offsetSec)}</span>
        <span class="sep" aria-hidden="true">·</span>
        <span class="num clock">{fmtClock(display.timestamp)}</span>
        <span class="live">Live</span>
      </button>
    {/if}
  </div>

  <div class="right">
    <div class="caption">
      <button type="button" class="cb" title="Minimize" aria-label="Minimize" onclick={() => windowCmd(() => appWindow.minimize())}>
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><rect x="0" y="4.5" width="10" height="1" fill="currentColor" /></svg>
      </button>
      <button type="button" class="cb" title="Maximize" aria-label="Maximize" onclick={() => windowCmd(() => appWindow.toggleMaximize())}>
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><rect x="0.5" y="0.5" width="9" height="9" rx="1.5" fill="none" stroke="currentColor" stroke-width="1" /></svg>
      </button>
      <button type="button" class="cb close" title="Close" aria-label="Close" onclick={() => windowCmd(() => appWindow.close())}>
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M0.5 0.5l9 9M9.5 0.5l-9 9" stroke="currentColor" stroke-width="1.1" /></svg>
      </button>
    </div>
  </div>
</header>

<style>
  .titlebar {
    position: relative;
    z-index: 30;
    height: var(--titlebar-h);
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    -webkit-app-region: drag;
    user-select: none;
  }
  .left {
    display: flex;
    align-items: center;
    gap: 8px;
    padding-left: 14px;
  }
  .mark {
    width: 15px;
    height: 15px;
  }
  .title {
    font-size: 12px;
    font-weight: 600;
    color: var(--t2);
    letter-spacing: 0.005em;
  }
  .center {
    display: flex;
    justify-content: center;
    min-width: 0;
  }
  .pill {
    -webkit-app-region: no-drag;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 24px;
    padding: 0 4px 0 10px;
    border: 0;
    border-radius: 999px;
    background: rgba(124, 156, 255, 0.18);
    box-shadow:
      inset 0 0 0 1px rgba(124, 156, 255, 0.4),
      0 6px 18px -8px rgba(124, 156, 255, 0.6);
    color: #dfe6ff;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    will-change: transform, opacity;
  }
  .rew {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 0 3px rgba(124, 156, 255, 0.25);
  }
  .sep {
    color: rgba(223, 230, 255, 0.5);
  }
  .clock {
    color: rgba(223, 230, 255, 0.78);
  }
  .live {
    margin-left: 3px;
    height: 18px;
    padding: 0 8px;
    display: inline-grid;
    place-items: center;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.14);
    font-size: 11px;
    color: #fff;
    transition: background-color var(--dur-fast) ease-out;
  }
  .pill:hover .live {
    background: rgba(255, 255, 255, 0.24);
  }
  .right {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: 6px;
  }
  .caption {
    display: flex;
    -webkit-app-region: no-drag;
  }
  .cb {
    width: 46px;
    height: var(--titlebar-h);
    display: grid;
    place-items: center;
    border: 0;
    background: transparent;
    color: var(--t2);
    cursor: pointer;
    transition:
      background-color 80ms linear,
      color 80ms linear;
  }
  .cb:hover {
    background: rgba(255, 255, 255, 0.08);
    color: var(--t1);
  }
  .cb:active {
    background: rgba(255, 255, 255, 0.05);
  }
  .cb.close:hover {
    background: #c42b1c;
    color: #fff;
  }
  .cb.close:active {
    background: #b0281a;
  }
  .cb:focus-visible {
    box-shadow: inset var(--focus-ring);
  }
</style>
