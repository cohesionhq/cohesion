export default function validate() {
  return B.check([[bench.memories.some((m) => /grocer/i.test(m) && /errand/i.test(m)), 'a memory that groceries go in Errands']]);
}
