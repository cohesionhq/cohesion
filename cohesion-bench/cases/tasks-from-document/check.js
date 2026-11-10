const titles = cohesion.workspace('Personal').tasks().map((t) => t.title);
const covered = [/runbook/i, /alert/i, /code review/i].map((pattern) => titles.some((title) => pattern.test(title)));
({
  pass: covered.every(Boolean) && titles.length === 3,
  reason: `tasks: ${titles.join(' | ') || 'none'}`,
})
