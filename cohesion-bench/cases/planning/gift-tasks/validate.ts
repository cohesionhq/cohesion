export default function validate() {
  const presents = B.newTasks(B.ANY);
  const forWhom = (name: string) => presents.find((t) => B.mentions(t.title, name));
  return B.check([
    [forWhom('Mum') !== undefined && /kneeler|cleeves/i.test(forWhom('Mum')?.description ?? ''), 'a present for Mum, with the idea'],
    [forWhom('Sam') !== undefined && /board game/i.test(forWhom('Sam')?.description ?? ''), 'a present for Sam, with the idea'],
    [forWhom('Priya') !== undefined && /cookbook/i.test(forWhom('Priya')?.description ?? ''), 'a present for Priya, with the idea'],
  ])
}
