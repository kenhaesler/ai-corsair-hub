<script lang="ts">
  import { onMount } from 'svelte';

  interface Props {
    leds: [number, number, number][] | null;
    label: string;
    width?: number;
    height?: number;
  }

  let { leds, label, width = 300, height = 40 }: Props = $props();

  let canvas: HTMLCanvasElement;
  let animFrame: number;

  const LED_COUNT = 21; // LS350

  onMount(() => {
    const dpr = window.devicePixelRatio || 1;
    canvas.width = width * dpr;
    canvas.height = height * dpr;
    canvas.style.width = `${width}px`;
    canvas.style.height = `${height}px`;

    const ctx = canvas.getContext('2d')!;
    ctx.scale(dpr, dpr);

    function draw() {
      ctx.clearRect(0, 0, width, height);

      const count = leds?.length ?? LED_COUNT;
      const cy = height / 2;
      const padding = 12;
      const ledRadius = 4;
      const glowRadius = 10;
      const spacing = (width - padding * 2) / Math.max(count - 1, 1);

      // Background bar
      ctx.beginPath();
      ctx.roundRect(4, 4, width - 8, height - 8, 6);
      ctx.fillStyle = 'rgba(255,255,255,0.02)';
      ctx.fill();

      for (let i = 0; i < count; i++) {
        const x = padding + i * spacing;
        const [r, g, b] = leds?.[i] ?? [30, 30, 30];

        // Glow
        const glow = ctx.createRadialGradient(x, cy, 0, x, cy, glowRadius);
        glow.addColorStop(0, `rgba(${r},${g},${b},0.3)`);
        glow.addColorStop(1, 'rgba(0,0,0,0)');
        ctx.beginPath();
        ctx.arc(x, cy, glowRadius, 0, Math.PI * 2);
        ctx.fillStyle = glow;
        ctx.fill();

        // LED dot
        ctx.beginPath();
        ctx.arc(x, cy, ledRadius, 0, Math.PI * 2);
        ctx.fillStyle = `rgb(${r},${g},${b})`;
        ctx.fill();
      }

      animFrame = requestAnimationFrame(draw);
    }

    draw();

    return () => cancelAnimationFrame(animFrame);
  });
</script>

<div class="strip-container">
  <canvas bind:this={canvas}></canvas>
  <span class="strip-label">{label}</span>
</div>

<style>
  .strip-container {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }
  canvas {
    border-radius: 6px;
  }
  .strip-label {
    font-size: 10px;
    color: var(--text-muted);
  }
</style>
