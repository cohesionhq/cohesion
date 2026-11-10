export default function solution() {
  ['Clean out the garage', 'Call Bob about the roof', 'Hang the new shelves'].forEach((title, i) => { B.task(title).dueDate = B.day(1 + i); });
}
