export default function validate() {
  const [doc] = B.docsLike(/birthdays/i, B.ANY);
  const text = doc?.contents ?? '';
  const next = (month: number, dayOfMonth: number) => {
    const at = new Date(B.date(0).getFullYear(), month, dayOfMonth, 12);
    if (at < B.date(0)) at.setFullYear(at.getFullYear() + 1);
    return at.getTime();
  };
  const people = [
    { name: 'Priya', at: next(2, 14), where: text.search(/Priya/) },
    { name: 'Sam', at: next(7, 2), where: text.search(/Sam/) },
    { name: 'Mum', at: next(10, 30), where: Math.max(text.search(/Helen/), text.search(/\bMum\b/)) },
  ];
  const order = [...people].sort((a, b) => a.at - b.at).map((p) => p.name);
  const written = people.filter((p) => p.where >= 0).sort((a, b) => a.where - b.where).map((p) => p.name);
  return B.check([
    [doc !== undefined, 'a Birthdays document'],
    [/14(th)?\s+(of\s+)?march|march\s+14|-03-14/i.test(text) && /\b2(nd)?\s+(of\s+)?august|august\s+2(nd)?\b|-08-02/i.test(text) && /30(th)?\s+(of\s+)?november|november\s+30|-11-30/i.test(text), 'the three dates'],
    [JSON.stringify(written) === JSON.stringify(order), `soonest first: ${order.join(', ')} (wrote ${written.join(', ')})`],
  ]);
}
