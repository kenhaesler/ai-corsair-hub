<!--
  Toasts: glass toasts bottom-centre above the status bar. Spring-like
  in/out (damped spring easing), FLIP reflow, hover/focus pauses dismissal.
-->
<script lang="ts">
  import { flip } from 'svelte/animate';
  import CircleCheck from 'lucide-svelte/icons/circle-check';
  import CircleAlert from 'lucide-svelte/icons/circle-alert';
  import Info from 'lucide-svelte/icons/info';
  import X from 'lucide-svelte/icons/x';
  import { toasts } from '../../lib/stores/toasts.svelte';
  import { reducedMotion } from '../../lib/motion.svelte';

  /** Damped spring easing (stiffness ~.2, damping ~.75 feel), normalised 0→1. */
  function springEase(t: number) {
    return 1 - Math.exp(-6.2 * t) * Math.cos(7.5 * t);
  }

  function toastIn(_node: Element) {
    return {
      duration: reducedMotion.current ? 0 : 520,
      css: (t: number) => {
        const e = springEase(t);
        return `opacity:${Math.min(1, t * 2.4)};transform:translateY(${(1 - e) * 24}px) scale(${0.94 + 0.06 * e})`;
      },
    };
  }
  function toastOut(_node: Element) {
    return {
      duration: reducedMotion.current ? 0 : 220,
      css: (t: number) => `opacity:${t};transform:translateY(${(1 - t) * 10}px) scale(${0.96 + 0.04 * t})`,
    };
  }
</script>

<div class="toasts" aria-live="polite" aria-relevant="additions">
  {#each toasts.items as t (t.id)}
    <div
      class="toast {t.kind}"
      role={t.kind === 'error' ? 'alert' : 'status'}
      in:toastIn
      out:toastOut
      animate:flip={{ duration: reducedMotion.current ? 0 : 300 }}
      onmouseenter={() => toasts.hold(t.id)}
      onmouseleave={() => toasts.resume(t.id)}
      onfocusin={() => toasts.hold(t.id)}
      onfocusout={() => toasts.resume(t.id)}
    >
      <span class="ic" aria-hidden="true">
        {#if t.kind === 'success'}<CircleCheck size={17} strokeWidth={2.25} />
        {:else if t.kind === 'error'}<CircleAlert size={17} strokeWidth={2.25} />
        {:else}<Info size={17} strokeWidth={2.25} />{/if}
      </span>
      <div class="txt">
        <p class="title">{t.title}</p>
        {#if t.detail}<p class="detail">{t.detail}</p>{/if}
      </div>
      <button type="button" class="x" aria-label="Dismiss notification" onclick={() => toasts.dismiss(t.id)}>
        <X size={13} strokeWidth={2.5} />
      </button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    left: 50%;
    bottom: calc(var(--statusbar-h) + 16px);
    transform: translateX(-50%);
    z-index: 200;
    display: flex;
    flex-direction: column-reverse;
    align-items: center;
    gap: 8px;
    pointer-events: none;
    width: min(460px, calc(100vw - 32px));
  }
  .toast {
    pointer-events: auto;
    display: flex;
    align-items: flex-start;
    gap: 10px;
    width: 100%;
    padding: 11px 10px 11px 14px;
    border-radius: 14px;
    background: rgba(28, 30, 38, 0.72);
    -webkit-backdrop-filter: blur(30px) saturate(180%);
    backdrop-filter: blur(30px) saturate(180%);
    box-shadow:
      var(--sh-pop),
      inset 0 1px 0 rgba(255, 255, 255, 0.07);
    will-change: transform, opacity;
  }
  .ic {
    display: grid;
    place-items: center;
    height: 20px;
  }
  .success .ic {
    color: var(--success);
  }
  .error .ic {
    color: var(--error);
  }
  .info .ic {
    color: var(--accent);
  }
  .txt {
    flex: 1;
    min-width: 0;
  }
  .title {
    font-size: 13px;
    line-height: 20px;
    font-weight: 600;
    color: var(--t1);
  }
  .detail {
    font-size: 12px;
    line-height: 17px;
    color: var(--t2);
    margin-top: 1px;
    overflow-wrap: anywhere;
  }
  .x {
    flex: none;
    width: 22px;
    height: 22px;
    display: grid;
    place-items: center;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--t3);
    cursor: pointer;
  }
  .x:hover {
    color: var(--t1);
    background: rgba(255, 255, 255, 0.08);
  }
</style>
