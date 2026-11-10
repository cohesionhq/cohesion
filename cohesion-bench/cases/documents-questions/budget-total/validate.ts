export default function validate() {
  return B.answered([
    [B.saysNumber(reply, 1500), `said 1500 (said: ${reply.slice(0, 200)})`],
  ])
}
