export default function validate() {
  return B.answered([
    [/1(st)?\s+(of\s+)?december|december\s+1(st)?\b/i.test(reply), '1 December'],
  ])
}
