import { defineConfig } from '@hey-api/openapi-ts';

// Generates the typed API SDK from the OpenAPI spec (itself generated from the
// Rust route tree). Output is committed so the frontend type-checks without a
// Rust/Node generation step; it's regenerated as part of the build (see the
// `svelte-rust` glue in server/build.rs) and by `npm run gen:api`.
export default defineConfig({
  input: 'openapi.json',
  output: 'src/lib/api/gen',
  // The spec has no `servers` entry, so without this the generator bakes the
  // *input filename* (`openapi.json`) in as the runtime `baseUrl`, and every
  // request resolves to a bogus path that the server answers with the SvelteKit
  // fallback (index.html) instead of JSON. The API is served same-origin as the
  // app, so pin the base to the site root.
  plugins: [{ name: '@hey-api/client-fetch', baseUrl: '/' }],
});
