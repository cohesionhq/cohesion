export default function solution() {
  B.ws().createTask({ title: 'Order business cards' });
  B.task('Order business cards', 'Acme').cancel();
}
