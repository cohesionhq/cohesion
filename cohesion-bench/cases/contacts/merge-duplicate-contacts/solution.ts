export default function solution() {
  const [keep, drop] = B.ws().contacts().filter((c) => c.firstName === 'Dana');
  for (const method of drop.methods) keep.addMethod(method.kind, method.value);
  drop.delete();
}
