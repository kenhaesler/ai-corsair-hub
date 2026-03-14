<script lang="ts">
  import { sensors } from '../../lib/stores/sensors.svelte';

  let canvas: HTMLCanvasElement;

  const POINTS = 60;

  $effect(() => {
    if (!canvas) return;
    const data = sensors.history;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const rect = canvas.getBoundingClientRect();
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    ctx.scale(dpr, dpr);

    const w = rect.width;
    const h = rect.height;
    const pad = { top: 8, right: 8, bottom: 20, left: 36 };
    const plotW = w - pad.left - pad.right;
    const plotH = h - pad.top - pad.bottom;

    // Clear
    ctx.clearRect(0, 0, w, h);

    // Use last POINTS snapshots
    const slice = data.slice(-POINTS);
    if (slice.length < 2) {
      ctx.fillStyle = 'rgba(255,255,255,0.2)';
      ctx.font = '11px system-ui';
      ctx.textAlign = 'center';
      ctx.fillText('Collecting data...', w / 2, h / 2);
      return;
    }

    const minTemp = 20;
    const maxTemp = 100;

    // Grid lines
    ctx.strokeStyle = 'rgba(255,255,255,0.05)';
    ctx.lineWidth = 1;
    for (let t = 30; t <= 90; t += 10) {
      const y = pad.top + plotH * (1 - (t - minTemp) / (maxTemp - minTemp));
      ctx.beginPath();
      ctx.moveTo(pad.left, y);
      ctx.lineTo(w - pad.right, y);
      ctx.stroke();

      ctx.fillStyle = 'rgba(255,255,255,0.2)';
      ctx.font = '9px system-ui';
      ctx.textAlign = 'right';
      ctx.fillText(`${t}\u00B0`, pad.left - 4, y + 3);
    }

    // Collect unique sensor names
    const sensorNames = new Set<string>();
    for (const snap of slice) {
      for (const t of snap.temperatures) sensorNames.add(t.source);
    }

    const colors: Record<string, string> = {
      cpu: '#60a5fa',
      gpu: '#34d399',
      psu_vrm: '#fbbf24',
      psu_case: '#f87171',
    };

    // Draw each sensor line
    for (const name of sensorNames) {
      const color = colors[name] || '#a78bfa';
      const points: { x: number; y: number }[] = [];

      for (let i = 0; i < slice.length; i++) {
        const snap = slice[i];
        const temp = snap.temperatures.find((t) => t.source === name);
        if (!temp) continue;

        const x = pad.left + (i / (POINTS - 1)) * plotW;
        const y = pad.top + plotH * (1 - (temp.celsius - minTemp) / (maxTemp - minTemp));
        points.push({ x, y: Math.max(pad.top, Math.min(pad.top + plotH, y)) });
      }

      if (points.length < 2) continue;

      // Gradient fill under line
      const gradient = ctx.createLinearGradient(0, pad.top, 0, pad.top + plotH);
      gradient.addColorStop(0, color + '26'); // ~15% alpha
      gradient.addColorStop(1, color + '00'); // transparent

      ctx.beginPath();
      ctx.moveTo(points[0].x, pad.top + plotH);
      for (const p of points) ctx.lineTo(p.x, p.y);
      ctx.lineTo(points[points.length - 1].x, pad.top + plotH);
      ctx.closePath();
      ctx.fillStyle = gradient;
      ctx.fill();

      // Line
      ctx.beginPath();
      ctx.moveTo(points[0].x, points[0].y);
      for (let i = 1; i < points.length; i++) ctx.lineTo(points[i].x, points[i].y);
      ctx.strokeStyle = color;
      ctx.lineWidth = 1.5;
      ctx.stroke();
    }

    // Legend
    let legendX = pad.left;
    ctx.font = '9px system-ui';
    for (const name of sensorNames) {
      const color = colors[name] || '#a78bfa';
      ctx.fillStyle = color;
      ctx.fillRect(legendX, h - 12, 8, 8);
      ctx.fillStyle = 'rgba(255,255,255,0.5)';
      ctx.textAlign = 'left';
      ctx.fillText(name.toUpperCase(), legendX + 12, h - 4);
      legendX += ctx.measureText(name.toUpperCase()).width + 24;
    }
  });
</script>

<canvas bind:this={canvas} class="chart-canvas"></canvas>

<style>
  .chart-canvas {
    width: 100%;
    height: 160px;
    display: block;
  }
</style>
