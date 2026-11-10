export default function validate() {
  return B.answered([[B.saysDay(reply, -2) || /two days ago|day before yesterday/i.test(reply), 'two days ago']]);
}
