export default function validate() {
  return B.answered([
    [B.saysNumber(reply, 3), `said 3 (said: ${reply.slice(0, 200)})`],
  ])
}
