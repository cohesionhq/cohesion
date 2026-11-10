export default function validate() {
  const text = B.ws().getJournal(B.day(0))?.contents ?? '';
  return B.check([
    [B.mentions(text, 'tax'), 'the tax paperwork'],
    [B.mentions(text, 'coffee with Sam'), 'the morning kept'],
  ])
}
