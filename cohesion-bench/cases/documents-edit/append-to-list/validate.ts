export default function validate() {
  const text = B.doc('Pantry staples').contents;
  return B.check([
    [/^\s*-\s*olive oil/im.test(text), 'olive oil as a list item'],
    [B.mentions(text, 'Rice', 'Pasta', 'Tinned tomatoes', 'Red lentils', 'Coffee beans'), 'every earlier item kept'],
  ])
}
