export default function validate() {
  return B.check([[bench.memories.some((m) => /vegetarian/i.test(m)), 'a memory that the user is vegetarian']])
}
