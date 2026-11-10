export default function validate() {
  return B.answered([
    [B.saysNone(reply), `said it isn't in my notes (said: ${reply.slice(0, 200)})`],
    [!/\b\d{4}\b/.test(reply.replace(/20\d\d/g, '')), 'invented no code'],
  ])
}
