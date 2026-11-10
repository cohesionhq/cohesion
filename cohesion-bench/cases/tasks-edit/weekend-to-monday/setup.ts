export default function setup() {
  const saturday = [1, 2, 3, 4, 5, 6, 7].find((n) => B.date(n).getDay() === 6)!;
  const w = B.ws();
  w.createTask({ title: 'Wash the car', dueDate: B.day(saturday) });
  w.createTask({ title: 'Batch cook for the week', dueDate: B.day(saturday + 1) });
  w.createTask({ title: 'Mow the lawn', dueDate: B.day(saturday + 7) });
  w.createTask({ title: 'Pay the window cleaner', dueDate: B.day(saturday + 2) });
  w.createTask({ title: 'Water the plants', dueDate: B.day(saturday - 1) });
  w.createTask({ title: 'Book a haircut' });
}
