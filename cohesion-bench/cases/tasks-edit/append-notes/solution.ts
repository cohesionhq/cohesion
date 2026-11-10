export default function solution() {
  const task = B.task('Plan Lisbon trip');
  task.description = task.description.replace('Hotel: undecided.', 'Hotel: booked, Casa do Mar.');
}
