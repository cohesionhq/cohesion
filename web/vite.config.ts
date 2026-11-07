import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { loadEnv } from 'vite';
import { defineConfig } from 'vitest/config';

// The CLI proxy (`cohesion open --dev --port <port>`) serves the protocol; Vite
// forwards the three protocol paths to it so the UI stays same-origin. The proxy
// checks the Host header, hence `changeOrigin`, and a token: the one in
// `COHESION_DEV_TOKEN`, which the CLI reads too (`scripts/web-dev.sh` sets both), added
// here so the browser never holds it. `VITE_PROXY_PORT` (environment or
// `.env.local`) picks the port; 4000 by default. The keys are anchored so
// `/connect` doesn't also catch `/connectors/…`.
export default defineConfig(({ mode }) => {
	const env = loadEnv(mode, process.cwd(), '');
	const port = env.VITE_PROXY_PORT || '4000';
	const token = env.COHESION_DEV_TOKEN;
	const daemon = `http://127.0.0.1:${port}`;
	const headers = token ? { Authorization: `Bearer ${token}` } : undefined;
	// Browsers can't set headers on a WebSocket upgrade, so `/connect` takes it in the query.
	const withToken = (path: string) =>
		token ? `${path}${path.includes('?') ? '&' : '?'}token=${encodeURIComponent(token)}` : path;
	return {
		plugins: [tailwindcss(), sveltekit()],
		// The licences of the packages bundled into the build, which cohesiond
		// embeds and serves; see THIRD_PARTY_NOTICES.md.
		build: { license: { fileName: 'third-party-licenses.md' } },
		server: {
			proxy: {
				'^/client(\\?|$)': { target: daemon, changeOrigin: true, headers },
				'^/connect(\\?|$)': {
					target: `ws://127.0.0.1:${port}`,
					ws: true,
					changeOrigin: true,
					rewrite: withToken
				}
			}
		},
		test: {
			include: ['src/**/*.test.ts'],
			environment: 'node',
			// The daemon's words, as the `hello` a test never receives would carry them.
			setupFiles: ['src/lib/protocol/vocabulary.setup.ts']
		}
	};
});
