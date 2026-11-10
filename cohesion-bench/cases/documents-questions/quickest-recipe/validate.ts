export default function validate() {
  return B.answered([
    [B.mentions(reply, 'salad'), `the green salad (said: ${reply.slice(0, 200)})`],
  ])
}
