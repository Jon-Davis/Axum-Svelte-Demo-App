import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({
      pages: 'build',
      assets: 'build',
      fallback: 'index.html',
      precompress: false,
      strict: true,
    }),
    files: {
      routes: 'src/routes',
    },
    prerender: {
      handleHttpError: ({ path, message }) => {
        // Auth routes are handled by Rust — ignore 404s from the prerender crawler
        if (path.startsWith('/auth/')) return;
        throw new Error(message);
      },
    },
  },
};

export default config;
