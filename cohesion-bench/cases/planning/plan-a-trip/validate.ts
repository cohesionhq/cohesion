export default function validate() {
  const rome = B.findDocFolder('Rome 2027');
  const itinerary = rome?.documents()[0];
  const text = itinerary?.contents ?? '';
  const flights = B.newTasks(B.ANY).find((t) => /flight/i.test(t.title));
  const hotel = B.newTasks(B.ANY).find((t) => /hotel/i.test(t.title));
  return B.check([
    [rome?.parentId === B.docFolder('Trips').id, 'Rome 2027 inside Trips'],
    [/day 1/i.test(text) && /day 2/i.test(text) && /day 3/i.test(text), 'an itinerary covering days 1 to 3'],
    [flights?.dueDate === B.day(30), `a flights task due ${B.day(30)}`],
    [hotel?.dueDate === B.day(30), `a hotel task due ${B.day(30)}`],
  ])
}
