export default function validate() {
  const titles = ['Buy printer ink', 'Call mum', 'Read Piranesi'];
  const checks = titles.flatMap((title) => {
    const copies = B.tasksLike(new RegExp(`^${title}$`)).sort((a, b) => a.id - b.id);
    return [
      [copies[0]?.status === 'incomplete', `the original "${title}" left open`],
      [copies[1]?.status === 'cancelled', `the copy of "${title}" cancelled`],
    ];
  });
  const others = B.changedTasks().filter((title) => !titles.includes(title));
  return B.check([...checks, [others.length === 0, `nothing else changed (changed: ${others.join(', ')})`]])
}
