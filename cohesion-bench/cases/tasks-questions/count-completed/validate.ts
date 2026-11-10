export default function validate() {
  return B.answered([
    [B.saysNumber(reply, 1), `said 1 (said: ${reply.slice(0, 200)})`],
    [!B.saysNumber(reply, 0), 'not 0'],
  ])
}
