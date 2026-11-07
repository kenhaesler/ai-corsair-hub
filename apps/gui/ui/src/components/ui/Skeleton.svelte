<!-- Skeleton: loading placeholder with a soft sheen (static under reduced motion). -->
<script lang="ts">
  interface Props {
    width?: string;
    height?: string;
    radius?: string;
  }
  let { width = '100%', height = '14px', radius = '6px' }: Props = $props();
</script>

<span class="sk" style:width style:height style:border-radius={radius} aria-hidden="true"></span>

<style>
  .sk {
    display: block;
    background: linear-gradient(
      100deg,
      rgba(255, 255, 255, 0.05) 30%,
      rgba(255, 255, 255, 0.1) 50%,
      rgba(255, 255, 255, 0.05) 70%
    );
    background-size: 300% 100%;
    animation: sheen 1.6s ease-in-out infinite;
  }
  @keyframes sheen {
    from {
      background-position: 100% 0;
    }
    to {
      background-position: 0 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .sk {
      animation: none;
    }
  }
</style>
