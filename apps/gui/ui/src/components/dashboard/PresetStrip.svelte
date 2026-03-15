<script lang="ts">
  import { applyPreset } from '../../lib/api';
  import { loadConfig, configStore } from '../../lib/stores/config.svelte';

  interface PresetDef {
    id: string;
    name: string;
    desc: string;
    points: { temp: number; duty: number }[];
    hysteresis: number;
    rampRate: number;
  }

  const presets: PresetDef[] = [
    {
      id: 'silent', name: 'Silent', desc: 'Near-silent',
      points: [{ temp: 30, duty: 20 }, { temp: 50, duty: 30 }, { temp: 65, duty: 50 }, { temp: 80, duty: 80 }, { temp: 90, duty: 100 }],
      hysteresis: 5, rampRate: 3,
    },
    {
      id: 'balanced', name: 'Balanced', desc: 'Daily driver',
      points: [{ temp: 30, duty: 25 }, { temp: 45, duty: 35 }, { temp: 60, duty: 55 }, { temp: 75, duty: 80 }, { temp: 85, duty: 100 }],
      hysteresis: 3, rampRate: 5,
    },
    {
      id: 'performance', name: 'Performance', desc: 'Aggressive',
      points: [{ temp: 30, duty: 30 }, { temp: 40, duty: 50 }, { temp: 55, duty: 70 }, { temp: 70, duty: 90 }, { temp: 80, duty: 100 }],
      hysteresis: 2, rampRate: 10,
    },
  ];

  let applying = $state<string | null>(null);

  // Detect active preset by comparing current config curves
  const activePreset = $derived.by(() => {
    const groups = configStore.config?.fan_groups;
    if (!groups || groups.length === 0) return null;
    for (const preset of presets) {
      const allMatch = groups.every(g => {
        if (g.mode.type !== 'curve') return false;
        const pts = g.mode.points;
        if (pts.length !== preset.points.length) return false;
        return pts.every((p, i) => p.temp === preset.points[i].temp && p.duty === preset.points[i].duty);
      });
      if (allMatch) return preset.id;
    }
    return null;
  });

  // Build SVG path for mini curve preview
  function curvePath(points: { temp: number; duty: number }[]): string {
    const w = 100;
    const h = 40;
    const tMin = 20, tMax = 100;
    return points
      .map((p, i) => {
        const x = ((p.temp - tMin) / (tMax - tMin)) * w;
        const y = h - (p.duty / 100) * h;
        return `${i === 0 ? 'M' : 'L'} ${x} ${y}`;
      })
      .join(' ');
  }

  function curveFill(points: { temp: number; duty: number }[]): string {
    const w = 100;
    const h = 40;
    const tMin = 20, tMax = 100;
    const path = curvePath(points);
    const lastX = ((points[points.length - 1].temp - tMin) / (tMax - tMin)) * w;
    const firstX = ((points[0].temp - tMin) / (tMax - tMin)) * w;
    return `${path} L ${lastX} ${h} L ${firstX} ${h} Z`;
  }

  async function doApply(preset: PresetDef) {
    applying = preset.id;
    try {
      await applyPreset(preset.id);
      await loadConfig();
    } catch {
      // ignore
    }
    applying = null;
  }
</script>

<div class="preset-strip">
  {#each presets as preset}
    {@const isActive = activePreset === preset.id}
    <button
      class="preset-card"
      class:active={isActive}
      class:applying={applying === preset.id}
      onclick={() => doApply(preset)}
      disabled={applying !== null}
    >
      <svg viewBox="0 0 100 40" class="curve-preview" preserveAspectRatio="none">
        <path d={curveFill(preset.points)} fill="rgba(96, 165, 250, 0.1)" />
        <path d={curvePath(preset.points)} fill="none" stroke="var(--accent)" stroke-width="2" stroke-linejoin="round" />
      </svg>
      <span class="preset-name">{preset.name}</span>
      <span class="preset-desc">{preset.desc}</span>
      <span class="preset-meta">hy:{preset.hysteresis}° ramp:{preset.rampRate}%/s</span>
    </button>
  {/each}
</div>

<style>
  .preset-strip {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }
  .preset-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 10px 12px;
    background: var(--bg-card-inset);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    cursor: pointer;
    transition: border-color 0.15s, background 0.15s, transform 0.1s;
  }
  .preset-card:hover {
    background: var(--bg-card-hover);
    border-color: var(--border-bright);
  }
  .preset-card:active {
    transform: scale(0.98);
  }
  .preset-card.active {
    border-color: var(--accent-dim);
    background: var(--bg-card-active);
    box-shadow: var(--shadow-glow);
  }
  .preset-card.applying {
    animation: pulse 0.6s ease infinite;
  }
  .curve-preview {
    width: 100%;
    height: 40px;
    display: block;
  }
  .preset-name {
    font-size: 13px;
    font-weight: 700;
    color: var(--text-primary);
  }
  .preset-desc {
    font-size: 11px;
    color: var(--text-muted);
  }
  .preset-meta {
    font-size: 9px;
    color: var(--text-muted);
    font-family: var(--font-mono);
  }
</style>
