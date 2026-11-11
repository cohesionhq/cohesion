export default function validate() {
  return B.answered([[B.saysNumber(reply, 6.5) || /\b6\.5\b/.test(reply), `said 6.5 (said: ${reply.slice(0, 200)})`]]);
}
