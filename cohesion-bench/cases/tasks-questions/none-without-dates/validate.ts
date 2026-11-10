export default function validate() {
  return B.answered([
    [B.saysNone(reply), `said there are none (said: ${reply.slice(0, 200)})`],
    [!B.mentions(reply, 'train fare') || /\bnone\b|\ball\b|every/i.test(reply), 'listed no task as undated'],
  ])
}
