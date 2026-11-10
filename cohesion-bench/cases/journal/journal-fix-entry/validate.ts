export default function validate() {
  const text = B.ws().getJournal(B.day(-1))?.contents ?? '';
  return B.check([
    [B.mentions(text, '29') && !B.mentions(text, '31'), '29 minutes, not 31'],
    [B.mentions(text, 'Priya', '5k'), 'the rest kept'],
  ]);
}
