export default function validate() {
  const text = B.doc('Lisbon itinerary').contents;
  const day4 = text.search(/^#+\s*Day 4/im);
  return B.check([
    [day4 > text.search(/^#+\s*Day 3/im), 'a Day 4 heading after Day 3'],
    [day4 >= 0 && /sintra/i.test(text.slice(day4)), 'Sintra under it'],
    [B.mentions(text, 'Alfama', 'Belém', 'Cacilhas'), 'days 1 to 3 kept'],
  ])
}
