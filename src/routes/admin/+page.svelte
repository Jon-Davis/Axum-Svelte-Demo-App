<script lang="ts">
  import { onMount } from 'svelte';
  import { listApiKeys, createApiKey, deleteApiKey, ApiError } from '$lib/api/client';
  import type { ApiKey } from '$lib/api/client';

  let keys = $state<ApiKey[]>([]);
  let loading = $state(true);
  let forbidden = $state(false);
  let actionError = $state<string | null>(null);
  let newName = $state('');
  let newRole = $state('user');
  let newExpiry = $state('');
  let creating = $state(false);
  let createdToken = $state<string | null>(null);
  let copied = $state(false);

  onMount(loadKeys);

  function describe(e: unknown): string {
    return e instanceof Error ? e.message : String(e);
  }

  async function loadKeys() {
    loading = true;
    actionError = null;
    try {
      keys = await listApiKeys();
    } catch (e: unknown) {
      if (e instanceof ApiError && e.status === 403) forbidden = true;
      else actionError = `Failed to load keys: ${describe(e)}`;
    }
    loading = false;
  }

  async function createKey(e: SubmitEvent) {
    e.preventDefault();
    if (!newName.trim()) return;
    creating = true;
    actionError = null;
    try {
      const data = await createApiKey({
        name: newName.trim(),
        role: newRole,
        expires_at: newExpiry ? new Date(newExpiry).toISOString() : null,
      });
      createdToken = data.token;
      newName = '';
      newRole = 'user';
      newExpiry = '';
      await loadKeys();
    } catch (e: unknown) {
      actionError = `Failed to create key: ${describe(e)}`;
    } finally {
      creating = false;
    }
  }

  async function revokeKey(id: string, name: string) {
    if (!confirm(`Revoke key "${name}"? This cannot be undone.`)) return;
    actionError = null;
    try {
      await deleteApiKey(id);
      await loadKeys();
    } catch (e: unknown) {
      actionError = `Failed to revoke key: ${describe(e)}`;
    }
  }

  async function copyToken() {
    if (!createdToken) return;
    await navigator.clipboard.writeText(createdToken);
    copied = true;
    setTimeout(() => { copied = false; }, 2000);
  }

  function fmt(iso: string | null): string {
    if (!iso) return '—';
    return new Date(iso).toLocaleString();
  }
</script>

<main>
  <h1>API Key Control Panel</h1>

  {#if forbidden}
    <p class="error">Access denied — admin accounts only.</p>
  {:else if loading}
    <p class="muted">Loading…</p>
  {:else}
    {#if actionError}
      <p class="error">{actionError}</p>
    {/if}

    {#if createdToken}
      <div class="banner">
        <p><strong>Key created — copy it now. It will not be shown again.</strong></p>
        <div class="token-row">
          <code>{createdToken}</code>
          <button onclick={copyToken} class="copy">{copied ? '✓ Copied' : 'Copy'}</button>
        </div>
        <button class="dismiss" onclick={() => { createdToken = null; }}>Dismiss</button>
      </div>
    {/if}

    <section>
      <h2>Create Key</h2>
      <form onsubmit={createKey}>
        <label>
          Name
          <input type="text" bind:value={newName} placeholder="my-service" required />
        </label>
        <label>
          Role
          <select bind:value={newRole}>
            <option value="user">user</option>
            <option value="admin">admin</option>
          </select>
        </label>
        <label>
          Expires (optional)
          <input type="datetime-local" bind:value={newExpiry} />
        </label>
        <button type="submit" disabled={creating}>
          {creating ? 'Creating…' : 'Create'}
        </button>
      </form>
    </section>

    <section>
      <h2>Active Keys</h2>
      {#if keys.length === 0}
        <p class="muted">No API keys yet.</p>
      {:else}
        <table>
          <thead>
            <tr>
              <th>Name</th>
              <th>Role</th>
              <th>Created</th>
              <th>Expires</th>
              <th>Last Used</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {#each keys as key (key.id)}
              <tr>
                <td>{key.name}</td>
                <td><span class="role role-{key.role}">{key.role}</span></td>
                <td>{fmt(key.created_at)}</td>
                <td>{fmt(key.expires_at)}</td>
                <td>{fmt(key.last_used_at)}</td>
                <td>
                  <button class="revoke" onclick={() => revokeKey(key.id, key.name)}>
                    Revoke
                  </button>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </section>
  {/if}
</main>

<style>
  main {
    max-width: 900px;
    margin: 2rem auto;
    padding: 0 1.5rem;
    font-family: system-ui, sans-serif;
  }

  h1 { margin-bottom: 1.5rem; }
  h2 { margin: 0 0 1rem; font-size: 1rem; font-weight: 600; }

  section {
    margin-bottom: 2rem;
    padding: 1.25rem 1.5rem;
    border: 1px solid #e5e7eb;
    border-radius: 8px;
  }

  form {
    display: flex;
    gap: 1rem;
    align-items: flex-end;
    flex-wrap: wrap;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.8125rem;
    font-weight: 500;
    color: #374151;
  }

  input {
    padding: 0.4375rem 0.75rem;
    border: 1px solid #d1d5db;
    border-radius: 6px;
    font-size: 0.875rem;
    min-width: 180px;
  }

  input:focus {
    outline: 2px solid #2563eb;
    outline-offset: -1px;
    border-color: transparent;
  }

  button {
    padding: 0.4375rem 1rem;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.875rem;
    font-weight: 500;
    background: #2563eb;
    color: white;
    white-space: nowrap;
  }

  button:disabled { opacity: 0.5; cursor: not-allowed; }
  button.revoke { background: #dc2626; padding: 0.25rem 0.75rem; font-size: 0.8125rem; }
  button.copy { background: #059669; }
  button.dismiss {
    background: transparent;
    color: #1d4ed8;
    border: 1px solid #1d4ed8;
    margin-top: 0.75rem;
    font-size: 0.8125rem;
  }

  table { width: 100%; border-collapse: collapse; font-size: 0.875rem; }
  th, td { text-align: left; padding: 0.625rem 0.75rem; border-bottom: 1px solid #e5e7eb; }
  th { font-weight: 600; color: #6b7280; font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.05em; background: #f9fafb; }
  tr:last-child td { border-bottom: none; }

  .banner {
    margin-bottom: 1.5rem;
    padding: 1rem 1.25rem;
    background: #ecfdf5;
    border: 1px solid #6ee7b7;
    border-radius: 8px;
  }

  .token-row {
    display: flex;
    gap: 0.75rem;
    align-items: center;
    margin-top: 0.5rem;
  }

  code {
    font-family: ui-monospace, monospace;
    font-size: 0.8125rem;
    word-break: break-all;
    flex: 1;
    color: #065f46;
  }

  select {
    padding: 0.4375rem 0.75rem;
    border: 1px solid #d1d5db;
    border-radius: 6px;
    font-size: 0.875rem;
    background: white;
  }

  .role {
    display: inline-block;
    padding: 0.125rem 0.5rem;
    border-radius: 9999px;
    font-size: 0.75rem;
    font-weight: 500;
  }

  .role-admin { background: #fef3c7; color: #92400e; }
  .role-user  { background: #f3f4f6; color: #374151; }

  .muted { color: #6b7280; }
  .error { color: #dc2626; }
</style>
