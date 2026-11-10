export default function validate() {
  const acme = B.ws('Acme').documents();
  const brief = acme.find((d) => /streaming billing feed/.test(d.contents));
  const plan = B.doc('Launch plan', 'Acme').contents;
  return B.check([
    [acme.every((d) => !/falcon/i.test(d.contents)), `no Falcon left (in: ${acme.filter((d) => /falcon/i.test(d.contents)).map((d) => d.title).join(', ')})`],
    [brief !== undefined && /kestrel/i.test(brief.contents) && B.mentions(brief.contents, 'Leo Park', 'end of Q1'), 'the brief says Kestrel and keeps the rest'],
    [/kestrel must be live/i.test(plan) && B.mentions(plan, 'Code freeze on the 20th'), 'the launch plan says Kestrel and keeps the rest'],
  ]);
}
