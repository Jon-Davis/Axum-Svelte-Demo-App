<svelte:head>
  <title>Access denied</title>
</svelte:head>

<main>
  <h1>Access denied</h1>
  <p>This area is restricted to admin accounts. If you believe you should have
    access, contact an administrator.</p>
  <a href="/">← Back to home</a>
</main>

<style>
  main {
    font-family: system-ui, sans-serif;
    max-width: 480px;
    margin: 4rem auto;
    padding: 0 1.5rem;
    text-align: center;
  }
  h1 { margin-bottom: 1rem; }
  p { color: #4b5563; line-height: 1.6; }
  a {
    display: inline-block;
    margin-top: 1.5rem;
    color: #4f46e5;
    text-decoration: none;
    font-size: 0.9rem;
  }
  a:hover { text-decoration: underline; }
</style>
