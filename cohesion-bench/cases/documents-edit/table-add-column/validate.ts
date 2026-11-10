export default function validate() {
  const text = B.doc('Lisbon budget').contents;
  const rows = text.split('\n').filter((line) => line.trim().startsWith('|') && !/^\|[\s|:-]+\|$/.test(line.trim()));
  const cells = (line: string) => line.trim().replace(/^\||\|$/g, '').split('|').map((c) => c.trim());
  const header = rows.length ? cells(rows[0]) : [];
  const paid = header.findIndex((c) => /paid/i.test(c));
  const yes = (item: string) => {
    const row = rows.find((line) => new RegExp(`^\\|\\s*${item}\\s*\\|`, 'i').test(line.trim()));
    return row ? /^(yes|y|✓|✔|✔️|✅|☑|☑️|x|paid|true)$/i.test(cells(row)[paid] ?? '') : null;
  };
  return B.check([
    [paid > 0, 'a Paid column'],
    [yes('Flights') === true && yes('Hotel') === true, 'flights and hotel paid'],
    [['Food', 'Transport', 'Museums'].every((item) => yes(item) === false), 'food, transport and museums not paid'],
    [B.numbers(text).filter((n) => [320, 690, 350, 80, 60].includes(n)).length >= 5, 'the costs kept'],
  ]);
}
