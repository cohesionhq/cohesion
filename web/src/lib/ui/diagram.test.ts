import { marked } from 'marked';
import { describe, expect, it, vi } from 'vitest';
import { ProtocolError } from '$lib/protocol/errors';
import { createMock } from '$lib/protocol/mock';
import type { RenderedMermaid, Send } from '$lib/protocol/types';
import { createDiagrams, fenceClosed } from './diagram';

const asIs = (svg: string) => svg;
const DRAWN: RenderedMermaid = { kind: 'svg', svg: '<svg></svg>', width: 10, height: 5 };

/** A `send` answering `renderMermaid` with each reply in turn, counting what it was asked. */
function replying(...replies: (RenderedMermaid | Error)[]) {
	return vi.fn(async () => {
		const reply = replies.shift();
		if (reply instanceof Error) throw reply;
		return reply;
	}) as unknown as Send & ReturnType<typeof vi.fn>;
}

describe('diagrams', () => {
	it('draws through the command', async () => {
		const { draw } = createDiagrams(createMock().send, asIs);
		expect((await draw('graph TD\n  A --> B', 300))?.svg).toMatch(/^<svg/);
		expect(await draw('hello world', 300)).toBeNull();
	});

	it('asks once per source and width, even while the first is on its way', async () => {
		const send = replying(DRAWN, DRAWN, DRAWN);
		const { draw } = createDiagrams(send, asIs);
		await Promise.all([draw('a', 300), draw('a', 300)]);
		await draw('a', 300);
		expect(send).toHaveBeenCalledTimes(1);
		await draw('a', 600);
		await draw('b', 300);
		expect(send).toHaveBeenCalledTimes(3);
	});

	it('leaves the width out for a block not laid out yet', async () => {
		const send = replying(DRAWN);
		await createDiagrams(send, asIs).draw('a', 0);
		expect(send).toHaveBeenCalledWith('renderMermaid', {
			source: 'a',
			width: undefined,
			format: 'svg'
		});
	});

	it('sanitizes what it keeps', async () => {
		const { draw } = createDiagrams(replying(DRAWN), () => 'clean');
		expect((await draw('a', 300))?.svg).toBe('clean');
	});

	it('keeps a refusal, but asks again after busy or a lost connection', async () => {
		const refused = (reason: 'syntax' | 'busy'): RenderedMermaid => ({
			kind: 'refused',
			reason,
			message: ''
		});
		const send = replying(
			refused('syntax'),
			refused('busy'),
			new ProtocolError('disconnected', 'Lost the connection.'),
			DRAWN
		);
		const { draw } = createDiagrams(send, asIs);
		expect(await draw('mistyped', 300)).toBeNull();
		expect(await draw('mistyped', 300)).toBeNull();
		expect(send).toHaveBeenCalledTimes(1);
		expect(await draw('a', 300)).toBeNull();
		expect(await draw('a', 300)).toBeNull();
		expect(await draw('a', 300)).toEqual(DRAWN);
		expect(send).toHaveBeenCalledTimes(4);
	});
});

describe('fenceClosed', () => {
	/** The raw text marked gives the one code block in `markdown`. */
	const raw = (markdown: string) => {
		const [token] = marked.lexer(markdown).filter((t) => t.type === 'code');
		return token.raw;
	};

	it.each([
		'```mermaid\ngraph TD\n```',
		'```mermaid\ngraph TD\n```\n\nThen more.',
		'```mermaid\n```',
		'````mermaid\ngraph TD\n`````',
		'~~~mermaid\ngraph TD\n~~~',
		'  ```mermaid\n  graph TD\n  ```'
	])('sees the end of %j', (markdown) => {
		expect(fenceClosed(raw(markdown))).toBe(true);
	});

	it.each([
		'```mermaid',
		'```mermaid\n',
		'```mermaid\ngraph TD',
		'```mermaid\ngraph TD\n``',
		'````mermaid\ngraph TD\n```',
		'```mermaid\ngraph TD\n```more'
	])('sees %j is still open', (markdown) => {
		expect(fenceClosed(raw(markdown))).toBe(false);
	});
});
