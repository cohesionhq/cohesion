// See https://svelte.dev/docs/kit/types#app.d.ts
import type { Back } from '$lib/shell/back';

declare global {
	namespace App {
		interface PageState {
			/**
			 * The page this one was reached from (see `trackBack`), or null when it
			 * replaced an entry that had none, so nothing in history leads back.
			 */
			back?: Back | null;
		}
	}

	interface ImportMetaEnv {
		readonly VITE_DEV_PROXY?: string;
		readonly VITE_MOCK?: string;
	}
}

export {};
