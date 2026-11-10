const due = Object.fromEntries(cohesion.workspace('Personal').tasks().map((t) => [t.title, t.dueDate]));
const expected = {
  'Pay rent': '2026-11-19',
  'Submit the expense report': '2026-11-19',
  'Water the plants': '2026-11-19',
  'Dentist appointment': '2026-11-13',
  'Call grandma': null,
};
const wrong = Object.entries(expected).filter(([title, date]) => due[title] !== date);
({
  pass: wrong.length === 0,
  reason: wrong.length ? wrong.map(([title, date]) => `${title}: ${due[title]} (wanted ${date})`).join('; ') : 'all dates right',
})
