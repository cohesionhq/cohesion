export default function validate() {
  const entry = B.ws().getJournal(B.day(0));
  return B.check([[B.mentions(entry?.contents ?? '', '20 lengths'), "today's entry mentions the swim"]])
}
