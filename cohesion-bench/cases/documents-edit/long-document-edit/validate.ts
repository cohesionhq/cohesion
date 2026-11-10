export default function validate() {
  const text = B.doc('Handbook').contents;
  const want = Array.from({ length: 600 }, (_, i) => i === 436 ? 'Section 437: The office closes at 5pm on Fridays.' : `Section ${i + 1}: Policy text number ${i + 1}, unchanged since 2019.`).join('\n');
  return B.check([
    [B.mentions(text, 'closes at 5pm on Fridays'), 'the new closing time'],
    [text === want, 'every other line exactly as it was'],
  ])
}
