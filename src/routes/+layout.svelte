<script>
  import { afterNavigate } from '$app/navigation';

  let { children } = $props();

  afterNavigate(async () => {
    const res = await fetch('/api/me');
    if (!res.ok) {
      // Force a full page load so Axum handles the redirect to Dex
      window.location.href = '/auth/login';
    }
  });
</script>

{@render children()}
