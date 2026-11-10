export default function validate() {
  return B.answered([
    [/\bBo\b/.test(reply), 'Bo'],
    [/2026-12-04|4(th)?\s+(of\s+)?december|december\s+4/i.test(reply), 'by 4 December'],
  ])
}
