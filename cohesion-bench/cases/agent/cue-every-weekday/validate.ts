export default function validate() {
  const monday = B.date(0);
  while (monday.getDay() !== 1) monday.setDate(monday.getDate() + 1);
  const at = (hour: number) => {
    const time = new Date(monday.getFullYear(), monday.getMonth(), monday.getDate(), hour, 0).getTime();
    return { time, hour, minute: 0, since: time - 60_000 };
  };
  const timers = bench.cues.filter((cue) => cue.hook === 'timer');
  const fires = (cue: CueLeft, hour: number) => {
    try { return Boolean(eval(`(${cue.code})`)(at(hour))); } catch { return false; }
  };
  return B.check([
    [timers.length >= 1, `a timer cue (cues: ${bench.cues.map((c) => c.hook).join(', ') || 'none'})`],
    [timers.some((cue) => fires(cue, 8) && !fires(cue, 9) && !fires(cue, 7)), 'fires at 8:00 on a Monday and not at 7 or 9'],
  ])
}
