export default function validate() {
  const text = B.doc('Garden plan').contents;
  return B.check([
    [!/old ideas/i.test(text), 'the heading gone'],
    [!B.mentions(text, 'pond') && !B.mentions(text, 'second shed'), 'its text gone'],
    [B.mentions(text, 'Tomatoes and beans', 'Herbs by the kitchen door', 'About £300'), 'the other sections kept'],
  ])
}
