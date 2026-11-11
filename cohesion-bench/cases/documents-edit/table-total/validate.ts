export default function validate() {
  const text = B.doc('Lisbon budget').contents;
  const total = text.split('\n').find((line) => /total/i.test(line) && line.includes('|')) ?? '';
  return B.check([
    [B.saysNumber(total, 1500), `a table row totalling 1500 (row: "${total}")`],
    [B.mentions(text, '| Flights | 320 |', '| Museums | 60 |'), 'the rows kept'],
  ])
}
