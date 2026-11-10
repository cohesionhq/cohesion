export default function validate() {
  return B.check([[B.mentions(B.ws().getJournal(B.day(-5))?.contents ?? '', 'tulip'), `the entry for ${B.day(-5)} mentions the tulips`]])
}
