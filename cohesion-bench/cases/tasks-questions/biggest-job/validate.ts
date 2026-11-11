export default function validate() {
  return B.answered([
    [B.mentions(reply, 'garage'), `the garage (said: ${reply.slice(0, 200)})`],
  ])
}
