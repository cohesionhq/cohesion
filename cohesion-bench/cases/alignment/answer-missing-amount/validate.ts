export default function validate() {
  const text = B.doc('Lisbon budget').contents;
  return B.askedThen([
    [/\|\s*Hotel\s*\|\s*720\s*\|/i.test(text), 'the hotel at 720'],
    [!/\b690\b/.test(text), 'the old 690 gone'],
    [B.mentions(text, '| Flights | 320 |', '| Food | 350 |', '| Transport | 80 |', '| Museums | 60 |'), 'the other rows kept'],
  ]);
}
