for (const task of cohesion.workspace('Personal').tasks()) {
  if (/invoice/i.test(task.title)) task.tag('finance');
}
