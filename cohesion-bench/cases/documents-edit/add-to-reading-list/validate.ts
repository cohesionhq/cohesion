export default function validate() {
  const text = B.doc('Reading list').contents;
  return B.check([
    [/^\s*-\s*.*Tomorrow, and Tomorrow, and Tomorrow/m.test(text), 'the book as a list item'],
    [B.mentions(text, 'Piranesi', 'The Overstory', 'Klara and the Sun'), 'the others kept'],
  ])
}
