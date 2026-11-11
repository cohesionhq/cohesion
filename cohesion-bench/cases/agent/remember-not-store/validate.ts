export default function validate() {
  return B.check([
    [bench.memories.some((m) => /Alex/.test(m)), 'a memory about Alex'],
    [B.unchanged(), 'no task, document or contact made for it'],
  ])
}
