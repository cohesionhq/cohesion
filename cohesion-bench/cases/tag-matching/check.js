const tasks = cohesion.workspace('Personal').tasks();
const tagged = (t) => t.tags().some((tag) => tag.name.toLowerCase() === 'finance');
const wanted = tasks.filter((t) => /invoice/i.test(t.title));
const missing = wanted.filter((t) => !tagged(t)).map((t) => t.title);
const extra = tasks.filter((t) => !/invoice/i.test(t.title) && tagged(t)).map((t) => t.title);
({
  pass: wanted.length === 3 && missing.length === 0 && extra.length === 0,
  reason: [missing.length ? `untagged: ${missing.join(', ')}` : '', extra.length ? `tagged by mistake: ${extra.join(', ')}` : '']
    .filter(Boolean).join('; ') || 'the three invoice tasks are tagged',
})
