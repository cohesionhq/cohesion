export default function solution() {
  const budget = B.doc('Lisbon budget');
  budget.contents = budget.contents
    .replace('| Item | Cost (EUR) |', '| Item | Cost (EUR) | Paid |')
    .replace('|---|---|', '|---|---|---|')
    .replace(/^(\| (Flights|Hotel) \| \d+ \|)$/gm, '$1 Yes |')
    .replace(/^(\| (Food|Transport|Museums) \| \d+ \|)$/gm, '$1 No |');
}
