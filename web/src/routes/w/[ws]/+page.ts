import { redirect } from '@sveltejs/kit';
import { routes } from '$lib/shell/routes';
import type { PageLoad } from './$types';

// `/w/<ws>` on its own lands where a workspace opens.
export const load: PageLoad = ({ params }) => {
	redirect(307, routes.workspace(Number(params.ws)));
};
