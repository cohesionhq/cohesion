const all = cohesion.workspace('Personal').tasks({ status: ['incomplete', 'completed', 'cancelled'] });
const report = all.filter((t) => /quarterly report/i.test(t.title));
const others = all.filter((t) => !/quarterly report/i.test(t.title));
const open = report.filter((t) => t.status !== 'completed').map((t) => t.title);
const touched = others.filter((t) => t.status !== 'incomplete').map((t) => t.title);
({
  pass: report.length === 3 && open.length === 0 && others.length === 3 && touched.length === 0,
  reason: [
    open.length ? `still open: ${open.join(', ')}` : '',
    touched.length ? `wrongly closed: ${touched.join(', ')}` : '',
  ].filter(Boolean).join('; ') || 'the three report tasks are done, the rest untouched',
})
