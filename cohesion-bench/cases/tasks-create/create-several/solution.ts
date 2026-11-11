export default function solution() {
  const folder = B.taskFolder('Errands');
  for (const title of ['Buy eggs', 'Buy flour', 'Buy butter', 'Buy vanilla extract']) B.ws().createTask({ title, folder });
}
