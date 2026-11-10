// The mock's stand-in for the daemon's formula sandbox: it has no JavaScript
// engine to run a formula in, and won't `eval` one, so it reads a small part
// of the language itself. A formula is `(row) => expression`, where the
// expression is made of numbers, strings, `true`, `false`, `null`, the row's
// values (`row.Pages`, `row["Unit price"]`), parentheses, `! -` before a
// value, `* / % + -`, comparisons, `=== !==`, `&& || ??`, `a ? b : c`, a
// string's or number's `.length`, `.toFixed(n)`, `.toUpperCase()` and
// `.toLowerCase()`, `Math.round floor ceil abs min max`, `String` and
// `Number`. Anything else isn't a formula the mock can run.

type Value = unknown;
type Row = Record<string, unknown>;
type Node = (row: Row) => Value;

const TOKEN =
	/\s*(?:(\d+(?:\.\d+)?)|("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*')|([A-Za-z_$][\w$]*)|(===|!==|==|!=|<=|>=|&&|\|\||\?\?|=>|[-+*/%<>!?:().,[\]]))/y;

function tokenize(source: string): string[] {
	const tokens: string[] = [];
	TOKEN.lastIndex = 0;
	while (TOKEN.lastIndex < source.length) {
		if (/^\s*$/.test(source.slice(TOKEN.lastIndex))) break;
		const match = TOKEN.exec(source);
		if (!match) throw new SyntaxError('Unexpected text');
		tokens.push(match[1] ?? match[2] ?? match[3] ?? match[4]);
	}
	return tokens;
}

const MATH: Record<string, (...n: number[]) => number> = {
	round: Math.round,
	floor: Math.floor,
	ceil: Math.ceil,
	abs: Math.abs,
	min: Math.min,
	max: Math.max
};

const BINARY: [string[], (a: Value, b: Value, op: string) => Value][] = [
	[['??'], (a, b) => a ?? b],
	[['||'], (a, b) => a || b],
	[['&&'], (a, b) => a && b],
	[['===', '!==', '==', '!='], (a, b, op) => (op === '===' || op === '==' ? a === b : a !== b)],
	[
		['<', '>', '<=', '>='],
		(a, b, op) => {
			const [x, y] = [a as number, b as number];
			return op === '<' ? x < y : op === '>' ? x > y : op === '<=' ? x <= y : x >= y;
		}
	],
	[
		['+', '-'],
		(a, b, op) => (op === '+' ? (a as number) + (b as number) : (a as number) - (b as number))
	],
	[
		['*', '/', '%'],
		(a, b, op) => {
			const [x, y] = [a as number, b as number];
			return op === '*' ? x * y : op === '/' ? x / y : x % y;
		}
	]
];

/** Reads `value.name`, failing as JavaScript would on null and undefined. */
function member(value: Value, name: string): Value {
	if (value === null || value === undefined)
		throw new TypeError(`Cannot read properties of ${value} (reading '${name}')`);
	if (name === 'length' && typeof value === 'string') return value.length;
	if (typeof value === 'number' && name === 'toFixed')
		return (digits: number) => value.toFixed(digits);
	if (typeof value === 'string' && name === 'toUpperCase') return () => value.toUpperCase();
	if (typeof value === 'string' && name === 'toLowerCase') return () => value.toLowerCase();
	if (typeof value === 'object') return (value as Row)[name];
	return undefined;
}

function parse(tokens: string[]): Node {
	let at = 0;
	const peek = () => tokens[at];
	const take = (expected?: string) => {
		const token = tokens[at++];
		if (token === undefined || (expected !== undefined && token !== expected))
			throw new SyntaxError(`Expected ${expected ?? 'more'}`);
		return token;
	};

	const expression = (): Node => {
		const test = binary(0);
		if (peek() !== '?') return test;
		take('?');
		const yes = expression();
		take(':');
		const no = expression();
		return (row) => (test(row) ? yes(row) : no(row));
	};

	const binary = (level: number): Node => {
		if (level === BINARY.length) return unary();
		const [ops, apply] = BINARY[level];
		let left = binary(level + 1);
		while (ops.includes(peek())) {
			const op = take();
			const [l, r] = [left, binary(level + 1)];
			left = (row) => apply(l(row), r(row), op);
		}
		return left;
	};

	const unary = (): Node => {
		if (peek() === '!') {
			take();
			const operand = unary();
			return (row) => !operand(row);
		}
		if (peek() === '-') {
			take();
			const operand = unary();
			return (row) => -(operand(row) as number);
		}
		return postfix(primary());
	};

	const args = (): Node[] => {
		const list: Node[] = [];
		take('(');
		while (peek() !== ')') {
			list.push(expression());
			if (peek() !== ')') take(',');
		}
		take(')');
		return list;
	};

	const postfix = (base: Node): Node => {
		let node = base;
		for (;;) {
			if (peek() === '.') {
				take();
				const name = take();
				const object = node;
				if (peek() === '(') {
					const list = args();
					node = (row) => {
						const method = member(object(row), name);
						if (typeof method !== 'function') throw new TypeError(`${name} is not a function`);
						return method(...list.map((a) => a(row)));
					};
				} else node = (row) => member(object(row), name);
			} else if (peek() === '[') {
				take();
				const key = expression();
				take(']');
				const object = node;
				node = (row) => member(object(row), String(key(row)));
			} else return node;
		}
	};

	const primary = (): Node => {
		const token = take();
		if (/^\d/.test(token)) return () => Number(token);
		if (/^["']/.test(token)) {
			const text: string =
				token[0] === '"'
					? JSON.parse(token)
					: JSON.parse(
							`"${token
								.slice(1, -1)
								.replace(/\\'/g, "'")
								.replace(/(?<!\\)"/g, '\\"')}"`
						);
			return () => text;
		}
		if (token === '(') {
			const inner = expression();
			take(')');
			return inner;
		}
		if (token === 'true' || token === 'false') return () => token === 'true';
		if (token === 'null') return () => null;
		if (token === 'undefined') return () => undefined;
		if (token === 'row') return (row) => row;
		if (token === 'Math' && peek() === '.') {
			take('.');
			const fn = MATH[take()];
			if (!fn) throw new SyntaxError('Unknown Math function');
			const list = args();
			return (row) => fn(...list.map((a) => a(row) as number));
		}
		if (token === 'String' || token === 'Number') {
			const list = args();
			const convert = token === 'String' ? String : Number;
			return (row) => convert(list[0]?.(row));
		}
		throw new SyntaxError(`Unexpected ${token}`);
	};

	const node = expression();
	if (at !== tokens.length) throw new SyntaxError(`Unexpected ${peek()}`);
	return node;
}

/**
 * Formula `source` as a function of a row, or null when it isn't one the mock
 * can read.
 */
export function compileMockFormula(source: string): ((row: Row) => Value) | null {
	try {
		const tokens = tokenize(source.trim());
		const parameter =
			tokens[0] === '(' && tokens[2] === ')' && tokens[3] === '=>' ? 4 : tokens[1] === '=>' ? 2 : 0;
		if (parameter === 0 || tokens[parameter === 4 ? 1 : 0] !== 'row') return null;
		return parse(tokens.slice(parameter));
	} catch {
		return null;
	}
}
