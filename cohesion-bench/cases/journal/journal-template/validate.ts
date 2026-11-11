export default function validate() {
  const text = B.ws().getJournal(B.day(0))?.contents ?? '';
  const at = (name: string) => text.search(new RegExp(`^#+\\s*${name}\\s*$`, 'im'));
  return B.check([
    [at('Morning') >= 0 && at('Afternoon') > at('Morning') && at('Evening') > at('Afternoon'), 'Morning, Afternoon and Evening headings, in order'],
  ]);
}
