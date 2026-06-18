import adapter from '@sveltejs/adapter-static';

/** @type {import('@sveltejs/kit').Config} */
const config = {
  kit: {
    adapter: adapter({
      pages: 'build',
      assets: 'build',
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
