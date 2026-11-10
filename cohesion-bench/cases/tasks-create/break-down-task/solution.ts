export default function solution() {
  const folder = B.taskFolder('Home');
  for (const title of ['Sort the garage shelves', 'Take old paint to the tip', 'Sweep the garage', 'Donate the old bikes']) {
    B.ws().createTask({ title, folder });
  }
}
