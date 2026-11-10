export default function validate() {
  return B.answered([
    [B.mentions(reply, 'itinerary'), 'the Lisbon itinerary'],
    [B.mentions(reply, 'Cacilhas'), 'Cacilhas'],
  ])
}
