export default function validate() {
  return B.answered([[B.saysNone(reply), `said there's no such entry (said: ${reply.slice(0, 200)})`]])
}
