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
    const pad = { top: 10, right: 48, bottom: 24, left: 40 };
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

    // Left Y-axis grid lines (Temperature)
    ctx.strokeStyle = 'rgba(255,255,255,0.05)';
    ctx.lineWidth = 1;
    for (let t = 30; t <= 90; t += 10) {
      const y = pad.top + plotH * (1 - (t - minTemp) / (maxTemp - minTemp));
      ctx.beginPath();
      ctx.moveTo(pad.left, y);
      ctx.lineTo(w - pad.right, y);
      ctx.stroke();

      ctx.fillStyle = 'rgba(255,255,255,0.25)';
      ctx.font = '9px system-ui';
      ctx.textAlign = 'right';
      ctx.fillText(`${t}\u00B0`, pad.left - 4, y + 3);
    }

    // Right Y-axis labels (Duty %)
    for (let d = 20; d <= 100; d += 20) {
      const y = pad.top + plotH * (1 - d / 100);
      ctx.fillStyle = 'rgba(96,165,250,0.25)';
      ctx.font = '9px system-ui';
      ctx.textAlign = 'left';
      ctx.fillText(`${d}%`, w - pad.right + 4, y + 3);
    }

    // Collect unique sensor names
    const sensorNames = new Set<string>();
    for (const snap of slice) {
      for (const t of snap.temperatures) sensorNames.add(t.source);
    }

    // Collect unique group names
    const groupNames = new Set<string>();
    for (const snap of slice) {
      for (const g of snap.group_duties) groupNames.add(g.name);
    }

    const tempColors: Record<string, string> = {
      cpu: '#60a5fa',
      gpu: '#34d399',
      psu_vrm: '#fbbf24',
      psu_case: '#f87171',
    };

    const dutyColors: Record<string, string> = {};
    const dutyColorPalette = ['rgba(96,165,250,0.5)', 'rgba(147,197,253,0.5)', 'rgba(191,219,254,0.5)', 'rgba(96,165,250,0.35)'];
    let ci = 0;
    for (const name of groupNames) {
      dutyColors[name] = dutyColorPalette[ci % dutyColorPalette.length];
      ci++;
    }

    // Draw fan duty lines (dashed, right axis mapped 0-100% to plot area)
    for (const name of groupNames) {
      const color = dutyColors[name];
      const points: { x: number; y: number }[] = [];

      for (let i = 0; i < slice.length; i++) {
        const snap = slice[i];
        const gd = snap.group_duties.find(g => g.name === name);
        if (!gd) continue;
        const x = pad.left + (i / (POINTS - 1)) * plotW;
        const y = pad.top + plotH * (1 - gd.duty_percent / 100);
        points.push({ x, y: Math.max(pad.top, Math.min(pad.top + plotH, y)) });
      }

      if (points.length < 2) continue;

      ctx.beginPath();
      ctx.moveTo(points[0].x, points[0].y);
      for (let i = 1; i < points.length; i++) ctx.lineTo(points[i].x, points[i].y);
      ctx.strokeStyle = color;
      ctx.lineWidth = 1.5;
      ctx.setLineDash([4, 3]);
      ctx.stroke();
      ctx.setLineDash([]);

      // Current value label at right edge
      const lastPt = points[points.length - 1];
      const lastSnap = slice[slice.length - 1];
      const lastGd = lastSnap.group_duties.find(g => g.name === name);
      if (lastGd) {
        ctx.fillStyle = color;
        ctx.font = '9px system-ui';
        ctx.textAlign = 'left';
        ctx.fillText(`${Math.round(lastGd.duty_percent)}%`, lastPt.x + 4, lastPt.y + 3);
      }
    }

    // Draw each sensor line (solid, left axis)
    for (const name of sensorNames) {
      const color = tempColors[name] || '#a78bfa';
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
      gradient.addColorStop(0, color + '26');
      gradient.addColorStop(1, color + '00');

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

      // Current value label at right edge
      const lastPt = points[points.length - 1];
      const lastSnap = slice[slice.length - 1];
      const lastTemp = lastSnap.temperatures.find(t => t.source === name);
      if (lastTemp) {
        ctx.fillStyle = color;
        ctx.font = 'bold 9px system-ui';
        ctx.textAlign = 'left';
        ctx.fillText(`${lastTemp.celsius.toFixed(1)}°`, lastPt.x + 4, lastPt.y + 3);
      }
    }

    // Legend
    let legendX = pad.left;
    ctx.font = '9px system-ui';
    // Temp sensors (solid)
    for (const name of sensorNames) {
      const color = tempColors[name] || '#a78bfa';
      ctx.fillStyle = color;
      ctx.fillRect(legendX, h - 12, 8, 8);
      ctx.fillStyle = 'rgba(255,255,255,0.5)';
      ctx.textAlign = 'left';
      ctx.fillText(name.toUpperCase(), legendX + 12, h - 4);
      legendX += ctx.measureText(name.toUpperCase()).width + 24;
    }
    // Duty groups (dashed indicator)
    for (const name of groupNames) {
      const color = dutyColors[name];
      ctx.strokeStyle = color;
      ctx.lineWidth = 1.5;
      ctx.setLineDash([3, 2]);
      ctx.beginPath();
      ctx.moveTo(legendX, h - 8);
      ctx.lineTo(legendX + 8, h - 8);
      ctx.stroke();
      ctx.setLineDash([]);
      ctx.fillStyle = 'rgba(255,255,255,0.35)';
      ctx.textAlign = 'left';
      ctx.fillText(name, legendX + 12, h - 4);
      legendX += ctx.measureText(name).width + 24;
    }
  });
</script>

<canvas bind:this={canvas} class="chart-canvas"></canvas>

<style>
  .chart-canvas {
    width: 100%;
    height: 100%;
    min-height: 120px;
    display: block;
  }
</style>
