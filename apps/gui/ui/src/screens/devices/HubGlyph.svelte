<!-- Glyph for an iCUE LINK System Hub: a body with its row of LINK ports. -->
<script lang="ts">
  interface Props {
    size?: number;
    strokeWidth?: number;
  }
  let { size = 22, strokeWidth = 1.75 }: Props = $props();
</script>

<svg
  viewBox="0 0 24 24"
  width={size}
  height={size}
  fill="none"
  stroke="currentColor"
  stroke-width={strokeWidth}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <rect x="3" y="6.5" width="18" height="11" rx="3" />
  <path d="M7.2 10.8v2.4M10.4 10.8v2.4M13.6 10.8v2.4M16.8 10.8v2.4" />
</svg>
