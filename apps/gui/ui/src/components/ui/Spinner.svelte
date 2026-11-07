<!-- Spinner: 8-spoke activity indicator (macOS style). Static under reduced motion. -->
<script lang="ts">
  interface Props {
    size?: number;
    label?: string;
  }
  let { size = 16, label }: Props = $props();
  const spokes = Array.from({ length: 8 }, (_, i) => i);
</script>

<span
  class="spinner"
  style:width="{size}px"
  style:height="{size}px"
  role={label ? 'status' : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : 'true'}
>
  <svg viewBox="0 0 24 24" width={size} height={size}>
    {#each spokes as i (i)}
      <rect
        x="11"
        y="2"
        width="2"
        height="6"
        rx="1"
        fill="currentColor"
        opacity={0.18 + (i / 8) * 0.82}
        transform="rotate({i * 45} 12 12)"
      />
    {/each}
  </svg>
</span>

<style>
  .spinner {
    display: inline-block;
    flex: none;
    color: currentColor;
  }
  svg {
    display: block;
    animation: spin 0.9s steps(8) infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    svg {
      animation-duration: 2.4s;
    }
  }
</style>
