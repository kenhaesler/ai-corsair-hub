// Motion primitives. Spring presets come straight from DESIGN.md's motion
// table. Every spring created through these helpers becomes instant when
// the user prefers reduced motion.

import { Spring, Tween } from 'svelte/motion';
import { cubicOut } from 'svelte/easing';

export interface SpringPreset {
  stiffness: number;
  damping: number;
  precision?: number;
}

export const springs = {
  /** live numbers */
  number: { stiffness: 0.14, damping: 0.82 },
  /** bars, chart cursor */
  bar: { stiffness: 0.12, damping: 0.78 },
  /** tab pill (x, width), segmented pills */
  tab: { stiffness: 0.22, damping: 0.72 },
  /** sheets / modals scale+opacity */
  sheet: { stiffness: 0.2, damping: 0.75 },
  /** scrub spring-back to live (~650 ms) */
  scrubBack: { stiffness: 0.07, damping: 0.86, precision: 0.002 },
  /** ambient hue (~20 s drift) */
  ambient: { stiffness: 0.006, damping: 1, precision: 0.0005 },
  /** ambient while scrubbing: follows the finger */
  ambientScrub: { stiffness: 0.05, damping: 0.9, precision: 0.001 },
  /** page cross-fade between tabs */
  page: { stiffness: 0.18, damping: 0.8, precision: 0.002 },
} satisfies Record<string, SpringPreset>;

// ---------------------------------------------------------------------------
// Environment runes: reduced motion + page visibility
// ---------------------------------------------------------------------------
class ReducedMotion {
  current = $state(false);
  constructor() {
    if (typeof window === 'undefined') return;
    const mq = window.matchMedia('(prefers-reduced-motion: reduce)');
    this.current = mq.matches;
    mq.addEventListener('change', (e) => (this.current = e.matches));
  }
}
/** `reducedMotion.current` is true when the OS asks for reduced motion. */
export const reducedMotion = new ReducedMotion();

class PageVisibility {
  visible = $state(true);
  constructor() {
    if (typeof document === 'undefined') return;
    this.visible = !document.hidden;
    document.addEventListener('visibilitychange', () => (this.visible = !document.hidden));
  }
}
/** `pageVisibility.visible`: false while the window is hidden/minimised. Pause rAF loops on it. */
export const pageVisibility = new PageVisibility();

// ---------------------------------------------------------------------------
// Spring helpers
// ---------------------------------------------------------------------------

/** Create a Spring from a preset. */
export function createSpring<T = number>(value: T, preset: SpringPreset = springs.number): Spring<T> {
  return new Spring<T>(value, { ...preset });
}

/** Set a spring's target; instant under reduced motion or when `instant` is passed. */
export function springTo<T>(s: Spring<T>, value: T, instant = false): Promise<void> {
  return s.set(value, { instant: instant || reducedMotion.current });
}

/**
 * Spring that follows a reactive getter. Must be called during component
 * initialisation (it registers an $effect). Starts at the first value.
 *
 *   const v = followSpring(() => temp, springs.number);
 *   {v.current.toFixed(1)}
 */
export function followSpring(get: () => number, preset: SpringPreset = springs.number): Spring<number> {
  const s = new Spring(get(), { ...preset });
  $effect(() => {
    const v = get();
    if (!Number.isFinite(v)) return;
    s.set(v, { instant: reducedMotion.current });
  });
  return s;
}

/** Tween helper honouring reduced motion. */
export function createTween(value: number, duration = 240): Tween<number> {
  return new Tween(value, { duration, easing: cubicOut });
}
export function tweenTo(t: Tween<number>, value: number, duration?: number): Promise<void> {
  return t.set(value, reducedMotion.current ? { duration: 0 } : duration !== undefined ? { duration } : undefined);
}

/**
 * Run `cb(dtSeconds)` every animation frame while `active()` is true and the
 * page is visible. Returns a stop function. Use inside $effect for cleanup.
 */
export function rafLoop(cb: (dt: number) => void): () => void {
  let id = 0;
  let last = performance.now();
  let running = true;
  const frame = (now: number) => {
    if (!running) return;
    const dt = Math.min(0.1, (now - last) / 1000);
    last = now;
    if (!document.hidden) cb(dt);
    id = requestAnimationFrame(frame);
  };
  const onVis = () => {
    if (document.hidden) {
      cancelAnimationFrame(id);
    } else if (running) {
      last = performance.now();
      id = requestAnimationFrame(frame);
    }
  };
  document.addEventListener('visibilitychange', onVis);
  id = requestAnimationFrame(frame);
  return () => {
    running = false;
    cancelAnimationFrame(id);
    document.removeEventListener('visibilitychange', onVis);
  };
}
