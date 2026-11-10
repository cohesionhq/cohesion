export default function validate() {
  const timers = bench.cues.filter((cue) => cue.hook === 'timer');
  const at = (offset: number, hour: number) => {
    const day = B.date(offset);
    const time = new Date(day.getFullYear(), day.getMonth(), day.getDate(), hour, 0).getTime();
    return { time, hour, minute: 0, since: time - 60_000 };
  };
  const fires = (cue: CueLeft, offset: number, hour: number) => {
    try { return Boolean(eval(`(${cue.code})`)(at(offset, hour))); } catch { return false; }
  };
  const daily = (cue: CueLeft) => [1, 2, 3, 4, 5, 6, 7].every((offset) => fires(cue, offset, 18) && !fires(cue, offset, 17) && !fires(cue, offset, 19));
  return B.check([
    [timers.length >= 1, `a timer cue (cues: ${bench.cues.map((c) => c.hook).join(', ') || 'none'})`],
    [timers.some(daily), 'fires at 18:00 every day of the week and not at 17 or 19'],
  ]);
}
