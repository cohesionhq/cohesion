export default function validate() {
  return B.answered([
    [B.mentions(reply, 'tap'), `the kitchen tap (said: ${reply.slice(0, 200)})`],
  ])
}
