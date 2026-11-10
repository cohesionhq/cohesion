export default function validate() {
  return B.check([
    [B.unchanged(), 'changed nothing yet'],
    [bench.asked.length > 0 || /\?/.test(reply), 'asked what to clear first'],
  ])
}
