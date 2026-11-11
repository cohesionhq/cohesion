export default function validate() {
  const text = B.doc('Onboarding guide', 'Acme').contents;
  return B.check([
    [!/recieve/i.test(text), 'no "recieve" left'],
    [(text.match(/receive/g) ?? []).length === 3, 'three "receive"s'],
    [B.mentions(text, 'laptop and a badge', '#it-help', 'by noon'), 'the rest kept'],
  ])
}
