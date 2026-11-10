export default function solution() {
  const errands = B.taskFolder('Errands');
  for (const title of ['Buy an onion', 'Buy garlic', 'Buy coconut milk', 'Buy curry powder', 'Buy eggs', 'Buy a red pepper', 'Buy cumin', 'Buy feta']) {
    B.ws().createTask({ title, folder: errands });
  }
}
