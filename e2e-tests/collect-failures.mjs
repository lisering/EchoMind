import { execSync } from 'child_process';
import { readFileSync } from 'fs';

// Run playwright with JSON reporter, capture output
const result = execSync('npx playwright test --reporter=json 2>/dev/null', {
  cwd: process.cwd(),
  encoding: 'utf8',
  maxBuffer: 100 * 1024 * 1024,
  env: { ...process.env, E2E_SPEED: '0.2' }
});

// Find the JSON line (playwright outputs it as a single line)
const jsonStart = result.indexOf('{');
const jsonStr = result.slice(jsonStart);
const data = JSON.parse(jsonStr);

const suites = data.suites || [];
const failed = [];

function collect(specs) {
  for (const s of specs) {
    if (s.specs) {
      for (const spec of s.specs) {
        const tests = spec.tests || [];
        const hasFail = tests.some(t => t.results && t.results.some(r => r.status === 'failed' || r.status === 'timedOut' || r.status === 'interrupted'));
        if (hasFail) {
          const file = s.file || '';
          failed.push({ file, title: spec.title, line: spec.line });
        }
      }
    }
    if (s.specs) collect(s.specs);
  }
}
collect(suites);

const byFile = {};
for (const f of failed) {
  const k = f.file || 'unknown';
  if (!byFile[k]) byFile[k] = [];
  byFile[k].push(f.title);
}
const sorted = Object.entries(byFile).sort((a, b) => b[1].length - a[1].length);
console.log('=== Failed tests by file ===');
for (const [file, tests] of sorted) {
  console.log(file + ': ' + tests.length + ' failed');
  for (const t of tests) console.log('  - ' + t);
}
console.log('Total failed: ' + failed.length);
