export default function validate() {
  return B.answered([
    [/2(nd)?\s+(of\s+)?august|august\s+2(nd)?\b/i.test(reply), '2 August'],
  ])
}
