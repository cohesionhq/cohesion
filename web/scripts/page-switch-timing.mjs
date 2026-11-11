// Times page switches in the web UI: from a click on a link to the new page
// drawn, in Chromium and WebKit, with view transitions turned off so only
// arriving at the page is counted.
//
// A switch ends at the frame that draws the page's last change in view: the
// URL is the link's, the page in <main> has changed, and nothing in the
// document has for QUIET_MS; of the changes, those that ended up outside the
// window or a scroller's clip (a feed's older messages drawn above it) don't
// count. VERBOSE prints the time to the last change of any kind as `all`.
// Each page is visited ROUNDS times in a loop; the first visit is "cold"
// (its first render), the rest show the median and worst of the others
// ("warm").
//
// Run it against the mock (`VITE_MOCK=1`), built or dev:
//
//   VITE_MOCK=1 VITE_DEV_PROXY=1 npm run build && npx vite preview --port 4173 &
//   node scripts/page-switch-timing.mjs http://localhost:4173
//
// Playwright isn't a dependency of the web UI; install it anywhere and point
// PLAYWRIGHT at it, e.g. `npm i --prefix /tmp/pw playwright` and
// `PLAYWRIGHT=/tmp/pw/node_modules/playwright/index.mjs`. Off Ubuntu its
// WebKit may want Ubuntu's libraries (libicu74, libjpeg8): copy them into the
// browser's `minibrowser-*/sys/lib`. Run it on the machine itself: in a
// container, WebKit stops drawing for a second or two at a time, which no
// browser does on a desktop.
//
// It runs under reduced motion, so a column sliding in isn't counted either.
//
// Options (environment): BROWSERS=chromium,webkit ROUNDS=6 TRANSITIONS=1
// (keep view transitions on) MOTION=1 (keep Svelte transitions on) PAGES=tasks,documents (only these) JSON=1 (print
// the raw samples) VERBOSE=1 (print each switch as it's timed).

const base = (process.argv[2] ?? 'http://localhost:4173').replace(/\/$/, '');
const { chromium, webkit } = await import(process.env.PLAYWRIGHT ?? 'playwright');
const engines = { chromium, webkit };
const browsers = (process.env.BROWSERS ?? 'chromium,webkit').split(',');
const ROUNDS = Number(process.env.ROUNDS ?? 6);
const QUIET_MS = 250;
const TIMEOUT_MS = 5000;

/** The pages, by the path after the workspace or from the root, in visiting order. */
const pages = (ws, agent) => [
	['today', `/w/${ws}/today`],
	['journal', `/w/${ws}/journal`],
	['documents', `/w/${ws}/documents`],
	['tasks', `/w/${ws}/tasks`],
	['articles', `/w/${ws}/articles`],
	['contacts', `/w/${ws}/contacts`],
	['rewind', `/w/${ws}/rewind`],
	['agents', `/agents`],
	...(agent ? [['agent', agent]] : []),
	['actors', `/actors`],
	['accounts', `/connectors/accounts`],
	['extensions', `/extensions`],
	['settings', `/settings`],
	// Last, as the app opens on it.
	['dashboard', `/w/${ws}/dashboard`]
];

/** Clicks the sidebar's link to `path`, or one put in the page for it, and times the switch. */
async function time(page, path) {
	return page.evaluate(
		({ path, QUIET_MS, TIMEOUT_MS }) =>
			new Promise((resolve, reject) => {
				let link = [...document.querySelectorAll('nav a[href], aside a[href]')].find(
					(a) => new URL(a.href).pathname === path
				);
				let added = false;
				if (!link) {
					// Pages the sidebar doesn't list (Rewind) are reached by a link like any other.
					link = document.createElement('a');
					link.href = path;
					link.textContent = 'go';
					(document.querySelector('main') ?? document.body).append(link);
					added = true;
				}
				let start = 0;
				let lastChange = 0;
				let changes = 0;
				/** Each batch of changes: when, the frame that painted it, and what it touched. */
				const batches = [];
				const timeline = [];
				// The page itself has to change, not only the URL and the sidebar.
				let drawn = false;
				const main = document.querySelector('main');
				// The frame that draws a change: its time once its style and layout are
				// done, which leaves out only the raster after. (A task posted from
				// there would wait behind whatever the page queued for after the paint.)
				const afterPaint = (cb) =>
					requestAnimationFrame(() => {
						document.documentElement.getBoundingClientRect();
						cb();
					});
				const observer = new MutationObserver((records) => {
					if (added && records.every((r) => r.target === link || r.addedNodes[0] === link)) return;
					const at = performance.now();
					lastChange = at;
					changes += records.length;
					timeline.push(`${Math.round(at - start)}:${records.length}`);
					drawn ||= records.some(
						(r) => r.type === 'childList' && r.addedNodes[0] !== link && main?.contains(r.target)
					);
					const batch = { at, painted: 0, nodes: records.flatMap(touched) };
					batches.push(batch);
					afterPaint(() => (batch.painted = performance.now()));
				});
				/** The elements a change shows on screen: what it added, or what it changed or emptied. */
				function touched(record) {
					if (record.type === 'childList' && !record.removedNodes.length) {
						// Svelte's comment anchors and blank text show nothing.
						const shown = [...record.addedNodes].filter(
							(n) =>
								n.isConnected &&
								(n.nodeType === Node.ELEMENT_NODE ||
									(n.nodeType === Node.TEXT_NODE && n.data.trim() !== ''))
						);
						return shown.map((n) => (n.nodeType === Node.ELEMENT_NODE ? n : n.parentElement));
					}
					const added = [...record.addedNodes].filter((n) => n.isConnected);
					return (added.length ? added : [record.target]).map((n) =>
						n.nodeType === Node.ELEMENT_NODE ? n : n.parentElement
					);
				}
				/** Whether `el` shows any of itself in the window, inside every scroller around it. */
				function inView(el) {
					if (!el?.isConnected) return false;
					if (getComputedStyle(el).display === 'contents') return true;
					const r = el.getBoundingClientRect();
					if (r.width === 0 && r.height === 0) return false;
					let [left, top, right, bottom] = [0, 0, innerWidth, innerHeight];
					for (let p = el.parentElement; p; p = p.parentElement) {
						const style = getComputedStyle(p);
						if (style.overflowX === 'visible' && style.overflowY === 'visible') continue;
						const c = p.getBoundingClientRect();
						[left, top] = [Math.max(left, c.left), Math.max(top, c.top)];
						[right, bottom] = [Math.min(right, c.right), Math.min(bottom, c.bottom)];
					}
					return r.right > left && r.left < right && r.bottom > top && r.top < bottom;
				}
				const finish = () => {
					observer.disconnect();
					if (added) link.remove();
					const shown = batches.filter((b) => b.nodes.some(inView)).at(-1) ?? batches.at(-1);
					resolve({
						ms: shown.painted - start,
						all: batches.at(-1).painted - start,
						changes,
						url: location.pathname,
						timeline: timeline.join(' ')
					});
				};
				// A page may redirect to one beneath it (Extensions to a tab).
				const arrived = () =>
					location.pathname === path || location.pathname.startsWith(`${path}/`);
				const poll = () => {
					const now = performance.now();
					if (now - start > TIMEOUT_MS) {
						observer.disconnect();
						reject(
							new Error(
								`${path}: not drawn after ${TIMEOUT_MS}ms (at ${location.pathname}, page changed: ${drawn})`
							)
						);
						return;
					}
					if (arrived() && drawn && batches.at(-1)?.painted && now - lastChange >= QUIET_MS)
						finish();
					else setTimeout(poll, 20);
				};
				link.addEventListener(
					'click',
					(event) => {
						start = event.timeStamp || performance.now();
						observer.observe(document.documentElement, {
							subtree: true,
							childList: true,
							attributes: true,
							characterData: true
						});
						setTimeout(poll, 20);
					},
					{ once: true, capture: true }
				);
				link.click();
			}),
		{ path, QUIET_MS, TIMEOUT_MS }
	);
}

const median = (xs) => {
	const s = [...xs].sort((a, b) => a - b);
	return s.length ? s[Math.floor((s.length - 1) / 2)] : NaN;
};

const results = {};
for (const name of browsers) {
	const browser = await engines[name].launch();
	const context = await browser.newContext({
		viewport: { width: 1440, height: 900 },
		// Svelte's enter and leave transitions are motion, not arriving, and run for 0ms under it.
		reducedMotion: process.env.MOTION ? 'no-preference' : 'reduce'
	});
	if (!process.env.TRANSITIONS) {
		await context.addInitScript(() => {
			delete Document.prototype.startViewTransition;
		});
	}
	const page = await context.newPage();
	page.on('pageerror', (error) => console.error(`[${name}] ${error.message}`));
	await page.goto(`${base}/`);
	await page.waitForURL(/\/w\/\d+\//, { timeout: 15000 });
	const ws = new URL(page.url()).pathname.split('/')[2];
	await page
		.waitForSelector('nav a[href*="/agents/"], aside a[href*="/agents/"]', {
			timeout: 5000
		})
		.catch(() => undefined);
	const agent = await page.evaluate(() => {
		const a = document.querySelector('nav a[href*="/agents/"], aside a[href*="/agents/"]');
		return a ? new URL(a.href).pathname : null;
	});
	// Settle as a person would before the first click: the app preloads pages a second in.
	await page.waitForTimeout(2000);
	const only = process.env.PAGES?.split(',');
	const list = pages(ws, agent).filter(([label]) => !only || only.includes(label));
	const samples = Object.fromEntries(list.map(([label]) => [label, []]));
	for (let round = 0; round < ROUNDS; round++) {
		for (const [label, path] of list) {
			const sample = await time(page, path);
			samples[label].push(sample.ms);
			if (process.env.VERBOSE) console.error(name, label, JSON.stringify(sample));
		}
	}
	results[name] = samples;
	await browser.close();
}

if (process.env.JSON) console.log(JSON.stringify(results, null, 1));
const labels = Object.keys(Object.values(results)[0] ?? {});
const fmt = (n) => (Number.isFinite(n) ? n.toFixed(0).padStart(5) : '    -');
console.log(
	`page          ${browsers.map((b) => `${b.padEnd(8)} cold median  worst`).join('   ')}`
);
for (const label of labels) {
	const cells = browsers.map((b) => {
		const [cold, ...warm] = results[b][label];
		return `${' '.repeat(8)}${fmt(cold)}  ${fmt(median(warm))}  ${fmt(Math.max(...warm))}`;
	});
	console.log(`${label.padEnd(12)}${cells.join('   ')}`);
}
