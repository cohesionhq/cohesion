const w = cohesion.workspace('Personal');
const errands = w.taskFolders().find((f) => f.title === 'Errands');
const filed = new Set(errands.tasks({}).map((t) => t.title));
const groceries = ['Buy milk', 'Buy eggs and bread', 'Get coffee beans'];
const missing = groceries.filter((t) => !filed.has(t));
const extra = [...filed].filter((t) => !groceries.includes(t));
({
  pass: missing.length === 0 && extra.length === 0,
  reason: [missing.length ? `not filed: ${missing.join(', ')}` : '', extra.length ? `filed by mistake: ${extra.join(', ')}` : '']
    .filter(Boolean).join('; ') || 'the three grocery tasks are in Errands',
})
