export default function solution() {
  const next = (month: number, dayOfMonth: number) => {
    const today = B.date(0);
    let at = new Date(today.getFullYear(), month, dayOfMonth, 12);
    if (at < today) at = new Date(today.getFullYear() + 1, month, dayOfMonth, 12);
    at.setDate(at.getDate() - 7);
    return B.iso(at);
  };
  B.ws().createTask({ title: 'Send Priya a birthday card', dueDate: next(2, 14) });
  B.ws().createTask({ title: 'Send Sam a birthday card', dueDate: next(7, 2) });
  B.ws().createTask({ title: 'Send Helen (Mum) a birthday card', dueDate: next(10, 30) });
}
