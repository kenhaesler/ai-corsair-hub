<script lang="ts">
  import { onMount } from 'svelte';

  interface Props {
    leds: [number, number, number][] | null;
    label: string;
    size?: number;
  }

  let { leds, label, size = 120 }: Props = $props();

  let canvas: HTMLCanvasElement;
  let animFrame: number;

  const LED_COUNT = 34; // QX fan

  onMount(() => {
    const dpr = window.devicePixelRatio || 1;
    canvas.width = size * dpr;
    canvas.height = size * dpr;
    canvas.style.width = `${size}px`;
    canvas.style.height = `${size}px`;

    const ctx = canvas.getContext('2d')!;
    ctx.scale(dpr, dpr);

    function draw() {
      ctx.clearRect(0, 0, size, size);

      const cx = size / 2;
      const cy = size / 2;
      const ringRadius = size * 0.38;
      const ledRadius = size * 0.035;
      const glowRadius = size * 0.07;
      const count = leds?.length ?? LED_COUNT;

      // Background circle
      ctx.beginPath();
      ctx.arc(cx, cy, ringRadius + ledRadius * 2, 0, Math.PI * 2);
      ctx.fillStyle = 'rgba(255,255,255,0.02)';
      ctx.fill();

      // Draw each LED
      for (let i = 0; i < count; i++) {
        const angle = (2 * Math.PI * i) / count - Math.PI / 2;
        const x = cx + Math.cos(angle) * ringRadius;
        const y = cy + Math.sin(angle) * ringRadius;

        const [r, g, b] = leds?.[i] ?? [30, 30, 30];

        // Bloom/glow pass
        const glow = ctx.createRadialGradient(x, y, 0, x, y, glowRadius);
        glow.addColorStop(0, `rgba(${r},${g},${b},0.35)`);
        glow.addColorStop(1, 'rgba(0,0,0,0)');
        ctx.beginPath();
        ctx.arc(x, y, glowRadius, 0, Math.PI * 2);
        ctx.fillStyle = glow;
        ctx.fill();

        // LED dot
        ctx.beginPath();
        ctx.arc(x, y, ledRadius, 0, Math.PI * 2);
        ctx.fillStyle = `rgb(${r},${g},${b})`;
        ctx.fill();
      }

      // Center hub circle
      ctx.beginPath();
      ctx.arc(cx, cy, size * 0.12, 0, Math.PI * 2);
      ctx.fillStyle = 'rgba(0,0,0,0.6)';
      ctx.fill();
      ctx.strokeStyle = 'rgba(255,255,255,0.08)';
      ctx.lineWidth = 1;
      ctx.stroke();

      // Label
      ctx.fillStyle = 'rgba(255,255,255,0.4)';
      ctx.font = `${size * 0.07}px system-ui`;
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.fillText(label, cx, cy);

      animFrame = requestAnimationFrame(draw);
    }

    draw();

    return () => {
      cancelAnimationFrame(animFrame);
    };
  });
</script>

<canvas bind:this={canvas}></canvas>

<style>
  canvas {
    border-radius: 50%;
  }
</style>
