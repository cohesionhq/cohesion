export default function solution() {
  const kitchen = B.ws().createTaskFolder({ title: 'Kitchen' });
  for (const t of B.taskFolder('Imported').tasks({ status: B.ALL })) if (t.title.startsWith('Kitchen:')) t.moveTo(kitchen);
}
