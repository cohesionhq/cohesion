export default function validate() {
  const text = B.doc('Launch plan', 'Acme').contents;
  const after = text.split(/^#+\s*Risks\s*$/im)[1] ?? '';
  const risks = after.split('\n').filter((line) => /^\s*([-*+]|\d+[.)])\s/.test(line));
  return B.check([
    [/^#+\s*Risks\s*$/im.test(text), 'a Risks heading'],
    [risks.length >= 2, `at least two risks under it (found ${risks.length})`],
    [B.mentions(text, 'Ship 3.2 to every customer', 'Code freeze on the 20th', 'Release on the 27th'), 'the goals and timeline kept'],
  ])
}
