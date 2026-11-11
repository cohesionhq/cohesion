export default function validate() {
  return B.answered([
    [B.mentions(reply, '/v2/session/refresh'), 'the refresh endpoint'],
  ])
}
