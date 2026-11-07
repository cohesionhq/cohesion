import js from '@eslint/js';
import prettier from 'eslint-config-prettier';
import svelte from 'eslint-plugin-svelte';
import globals from 'globals';
import ts from 'typescript-eslint';
import svelteConfig from './svelte.config.js';

export default ts.config(
	{ ignores: ['build/', '.svelte-kit/', 'node_modules/'] },
	js.configs.recommended,
	...ts.configs.recommended,
	...svelte.configs.recommended,
	prettier,
	...svelte.configs.prettier,
	{
		languageOptions: { globals: { ...globals.browser, ...globals.node } },
		rules: {
			'no-undef': 'off',
			'@typescript-eslint/no-unused-vars': ['error', { argsIgnorePattern: '^_' }],
			// Every path comes from `$lib/shell/routes`, which already calls `resolve()`.
			'svelte/no-navigation-without-resolve': 'off'
		}
	},
	{
		files: ['**/*.svelte'],
		rules: {
			// A native tooltip gets in the way of a resting cursor; an icon-only control names itself with `aria-label`.
			'no-restricted-syntax': [
				'error',
				{
					selector:
						"SvelteElement[kind='html'] > SvelteStartTag > :matches(SvelteAttribute[key.name='title'], SvelteShorthandAttribute[key.name='title'])",
					message: 'No tooltips: drop `title`, and give an icon-only control an `aria-label`.'
				}
			]
		}
	},
	{
		// A page works in either half of a split view, so it reads and moves its own half
		// (`pane()` in `$lib/shell/pane.svelte`), never the router's one page.
		files: ['src/routes/**', 'src/lib/features/**', 'src/lib/ui/**'],
		ignores: ['src/routes/+layout.svelte'],
		rules: {
			'no-restricted-imports': [
				'error',
				{
					paths: [
						{ name: '$app/state', message: 'Read `pane()` from `$lib/shell/pane.svelte`.' },
						{
							name: '$app/navigation',
							importNames: ['goto', 'beforeNavigate', 'afterNavigate', 'replaceState', 'pushState'],
							message: 'Use `pane().goto` or `pane().beforeLeave` from `$lib/shell/pane.svelte`.'
						}
					]
				}
			]
		}
	},
	{
		// The connection's maps and sets are bookkeeping; its reactive state is `$state` fields.
		files: ['src/lib/protocol/**'],
		rules: { 'svelte/prefer-svelte-reactivity': 'off' }
	},
	{
		files: ['**/*.svelte', '**/*.svelte.ts', '**/*.svelte.js'],
		languageOptions: {
			parserOptions: {
				projectService: true,
				extraFileExtensions: ['.svelte'],
				parser: ts.parser,
				svelteConfig
			}
		}
	}
);
