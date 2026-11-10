export default function validate() {
  return B.answered([
    [B.saysNumber(reply, 4), `said 4 (said: ${reply.slice(0, 200)})`],
  ])
}
