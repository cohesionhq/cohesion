export default function validate() {
  return B.answered([
    [B.saysNumber(reply, 13), `said 13 (said: ${reply.slice(0, 200)})`],
  ])
}
