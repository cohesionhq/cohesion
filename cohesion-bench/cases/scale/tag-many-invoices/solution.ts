export default function solution() {
  for (const t of B.taskFolder('Imported').tasks({ status: B.ALL })) if (/invoice/i.test(t.title)) t.tag('billing');
}
