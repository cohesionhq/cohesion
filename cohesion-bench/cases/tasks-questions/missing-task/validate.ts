export default function validate() {
  return B.answered([
    [B.saysNone(reply), `said there's no such task (said: ${reply.slice(0, 200)})`],
    [!/\d{4}-\d{2}-\d{2}/.test(reply), 'gave no date'],
  ])
}
