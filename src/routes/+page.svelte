<script lang="ts">
  import { onMount } from 'svelte';
  import { getMe, getHello } from '$lib/api/gen';
  import type { UserInfo } from '$lib/api/gen';

  let count = $state(0);
  let apiMessage = $state('(not fetched)');
  let user = $state<UserInfo | null>(null);

  // `??` only guards null/undefined; use `||` so an empty-string username/email
  // also falls through (otherwise `''[0]` below would throw).
  const displayName = $derived(user ? (user.username || user.email || '?') : '?');

  async function fetchHello() {
    const { data, error } = await getHello();
    apiMessage = data ? data.message : `(error: ${error ? JSON.stringify(error) : 'request failed'})`;
  }

  onMount(async () => {
    // No session → 401; leave `user` null (the generated client returns the
    // error in `error` rather than throwing).
    const { data } = await getMe();
    if (data) user = data;
  });
</script>

<svelte:head>
  <title>Svelte + Rust/Axum</title>
</svelte:head>

<header>
  <span class="brand">Svelte + Rust/Axum</span>
  {#if user}
    <div class="profile">
      <span class="avatar">{displayName[0].toUpperCase()}</span>
      <span>{displayName}</span>
      {#if user.role === 'admin'}
        <a href="/admin/">Admin panel</a>
      {/if}
      <form method="POST" action="/auth/logout" class="logout-form">
        <button type="submit" class="logout-link">Log out</button>
      </form>
    </div>
  {:else}
    <a href="/auth/login" class="login-link">Log in</a>
  {/if}
</header>

<main>
  <section>
    <h2>Counter</h2>
    <p>Count: {count}</p>
    <button onclick={() => count++}>Increment</button>
  </section>

  <section>
    <h2>API Call</h2>
    <p>Response: {apiMessage}</p>
    <button onclick={fetchHello}>Call GET /api/hello</button>
  </section>

  <nav>
    <a href="/hello">Go to /hello →</a>
  </nav>
</main>

<style>
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.75rem 1.5rem;
    border-bottom: 1px solid #e0e0e0;
    font-family: sans-serif;
  }

  .brand { font-weight: 600; font-size: 1rem; }

  .profile {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    font-size: 0.9rem;
  }

  .avatar {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 2rem;
    height: 2rem;
    border-radius: 50%;
    background: #4f46e5;
    color: white;
    font-weight: 600;
    font-size: 0.85rem;
  }

  .login-link, .profile a {
    font-size: 0.85rem;
    color: #4f46e5;
    text-decoration: none;
  }

  .login-link:hover, .profile a:hover { text-decoration: underline; }

  /* A POST form, styled to read as the inline link it replaces.
     `display: contents` removes the form's own box so the button sits directly
     in the header's flex row — same gap and alignment as the other links. */
  .logout-form { display: contents; }
  .logout-link {
    font-size: 0.85rem;
    color: #4f46e5;
    background: none;
    border: none;
    padding: 0;
    margin: 0;
    cursor: pointer;
    font-family: inherit;
  }
  .logout-link:hover { text-decoration: underline; }

  main { font-family: sans-serif; max-width: 480px; margin: 2rem auto; padding: 0 1rem; }
  section { margin-top: 1.5rem; padding: 1rem; border: 1px solid #ccc; border-radius: 6px; }
  button { margin-top: 0.5rem; padding: 0.4rem 1rem; cursor: pointer; }
  nav { margin-top: 1.5rem; }
</style>
