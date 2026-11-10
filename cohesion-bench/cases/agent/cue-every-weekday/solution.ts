export default function solution() {
  return ({ cues: [{ name: 'Morning summary', hook: 'timer', code: '({ time, hour, minute }) => hour === 8 && minute === 0 && ![0, 6].includes(new Date(time).getDay())', instructions: 'Tell the user what is due today.', effort: 'low' }] })
}
