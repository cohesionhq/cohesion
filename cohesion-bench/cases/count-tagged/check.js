const all = cohesion.workspace('Personal').tasks({ status: ['incomplete', 'completed', 'cancelled'] });
const unchanged = all.length === 6 && all.filter((t) => t.status === 'completed').length === 1;
const numbers = reply.match(/\b\d+\b/g) ?? [];
const said = /\b3\b|\bthree\b/i.test(reply);
({
  pass: unchanged && said && !numbers.some((n) => n !== '3'),
  reason: !unchanged ? 'the tasks changed' : said ? 'answered 3' : `answered: ${reply.slice(0, 120)}`,
})
