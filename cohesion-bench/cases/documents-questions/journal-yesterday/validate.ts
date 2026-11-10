export default function validate() {
  return B.answered([
    [B.mentions(reply, '5k') || B.mentions(reply, 'ran'), 'the run'],
    [B.mentions(reply, 'Priya'), 'dinner with Priya'],
  ])
}
