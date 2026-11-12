// Extensions: permissions, hooks, installing and registries.

import { ProtocolError } from '../../errors';
import type { Handlers, MockCore } from '../core';

export function extensionsHandlers(core: MockCore) {
	const { state, nextId, notFound, grantedRegistries, extensionUpdates, preview, granted, orphan } =
		core;
	return {
		listExtensionPermissions: ({ extension_ids }) =>
			state.permissions
				.filter((p) => extension_ids.includes(p.extension_id))
				.map((p) => ({ ...p })),
		setExtensionPermission: ({ extension_id, permission, enabled }) => {
			const row = state.permissions.find(
				(p) => p.extension_id === extension_id && p.permission === permission
			);
			if (row) row.enabled = enabled;
			else
				state.permissions.push({
					id: nextId(state.permissions),
					extension_id,
					permission,
					enabled
				});
			return null;
		},
		listExtensionHooks: ({ extension_ids }) =>
			state.hooks.filter((h) => extension_ids.includes(h.extension_id)).map((h) => ({ ...h })),
		setExtensionHook: ({ extension_id, hook, enabled }) => {
			const row = state.hooks.find((h) => h.extension_id === extension_id && h.hook === hook);
			if (row) row.enabled = enabled;
			else state.hooks.push({ id: nextId(state.hooks), extension_id, hook, enabled });
			return null;
		},
		uninstallExtension: ({ extension_id }) => {
			state.extensions = state.extensions.filter((e) => e.id !== extension_id);
			state.permissions = state.permissions.filter((p) => p.extension_id !== extension_id);
			state.hooks = state.hooks.filter((h) => h.extension_id !== extension_id);
			orphan((a) => a.host.kind === 'extension' && a.host.extension_id === extension_id);
			return null;
		},
		// Installs a script `previewExtension` served the way the daemon does:
		// new permissions and hooks start off, existing grants stay, dropped
		// ones go. A pasted script the mock can't read is accepted and ignored.
		registerExtension: ({ script, allow_overwrite }) => {
			const source = [...state.sources.values()].find((s) => s.script === script);
			if (!source) return null;
			const { manifest } = source;
			let extension = state.extensions.find((e) => e.script_identifier === manifest.identifier);
			if (extension && !allow_overwrite)
				throw new ProtocolError(
					'validationFailed',
					'This extension is already installed. You must override to allow this.'
				);
			if (!extension) {
				extension = {
					id: nextId(state.extensions),
					name: '',
					version: 0,
					script_identifier: manifest.identifier,
					priority: 0,
					description: '',
					author: '',
					icon: null
				};
				state.extensions.push(extension);
			}
			Object.assign(extension, {
				name: manifest.name,
				version: manifest.version,
				description: manifest.description ?? '',
				author: manifest.author ?? '',
				icon: manifest.icon
			});
			const id = extension.id;
			state.permissions = state.permissions.filter(
				(p) => p.extension_id !== id || manifest.permissions.includes(p.permission)
			);
			for (const permission of manifest.permissions)
				if (!state.permissions.some((p) => p.extension_id === id && p.permission === permission))
					state.permissions.push({
						id: nextId(state.permissions),
						extension_id: id,
						permission,
						enabled: false
					});
			state.hooks = state.hooks.filter(
				(h) => h.extension_id !== id || manifest.hooks.includes(h.hook)
			);
			for (const hook of manifest.hooks)
				if (!state.hooks.some((h) => h.extension_id === id && h.hook === hook))
					state.hooks.push({ id: nextId(state.hooks), extension_id: id, hook, enabled: false });
			return null;
		},
		// Each registry the user granted `registry` answers; matching is the
		// registry's business, so the mock's match name, author and description.
		searchExtensionRegistries: ({ query }) => {
			const q = query.trim().toLowerCase();
			return grantedRegistries().map((e) => {
				const registry = state.registries.find((r) => r.extension_id === e.id);
				return {
					extension_id: e.id,
					name: e.name,
					results: (registry?.results ?? [])
						.filter(({ manifest: m }) =>
							[m.name, m.author ?? '', m.description ?? ''].some((text) =>
								text.toLowerCase().includes(q)
							)
						)
						.map((r) => structuredClone(r)),
					error: registry?.error ?? null
				};
			});
		},
		// The mock checks on every call, so listing answers what checking would.
		listExtensionUpdates: () => extensionUpdates(),
		checkExtensionUpdates: () => extensionUpdates(),
		previewExtension: ({ source_url }) => {
			const source = state.sources.get(source_url);
			if (!source)
				throw new ProtocolError('validationFailed', `${source_url} answered 404 Not Found.`);
			return preview(source);
		},
		// Only scripts the mock serves have a manifest it can read.
		previewExtensionScript: ({ script }) => {
			const source = [...state.sources.values()].find((s) => s.script === script);
			if (!source)
				throw new ProtocolError('validationFailed', 'The mock can only read scripts it serves.');
			return preview(source);
		},
		// No mock extension defines `serviceRequest`.
		callExtension: ({ identifier }) => {
			const extension =
				state.extensions.find((e) => e.script_identifier === identifier) ?? notFound('extension');
			if (!granted(extension.id, 'service'))
				throw new ProtocolError(
					'validationFailed',
					"This extension doesn't have the `service` permission enabled."
				);
			throw new ProtocolError(
				'validationFailed',
				`${extension.name}: script does not define a function named \`serviceRequest\``
			);
		}
	} satisfies Partial<Handlers>;
}
