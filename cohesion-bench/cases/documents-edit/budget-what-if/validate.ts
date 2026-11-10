export default function validate() {
  const text = B.doc('Lisbon budget').contents;
  const after = text.slice(text.lastIndexOf('|') + 1);
  return B.check([
    [B.saysNumber(after, 1265), `1265 under the table (added: "${after.trim().slice(0, 200)}")`],
    [B.mentions(text, '| Flights | 320 |', '| Hotel | 690 |', '| Food | 350 |', '| Transport | 80 |', '| Museums | 60 |'), 'the table kept'],
  ]);
}
