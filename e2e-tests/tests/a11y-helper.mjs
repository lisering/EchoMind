// axe-core accessibility testing helpers for Playwright
// Shared utilities: scan runner, threshold assertions, report formatters.
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const configPath = path.resolve(__dirname, '../axe.config.json');

let cachedConfig = null;

/**
 * Load axe.config.json (cached after first read).
 * @returns {import('axe-core').AxeOptions}
 */
export function loadAxeConfig() {
  if (cachedConfig) return cachedConfig;
  cachedConfig = JSON.parse(readFileSync(configPath, 'utf-8'));
  return cachedConfig;
}

/**
 * Run an axe-core scan on the given page.
 *
 * @param {import('@playwright/test').Page} page - Playwright page
 * @param {object} [options]
 * @param {string[]} [options.tags] - WCAG tags (default from axe.config.json)
 * @param {string[]} [options.rules] - Specific rule IDs to enable/disable
 * @param {string} [options.include] - CSS selector to scope the scan
 * @param {string} [options.exclude] - CSS selector to exclude from scan
 * @param {object} [options.axeOptions] - Additional axe-core options
 * @returns {Promise<import('axe-core').AxeResults>}
 */
export async function runAxeScan(page, options = {}) {
  const { AxeBuilder } = await import('@axe-core/playwright');
  const config = loadAxeConfig();

  const tags = options.tags ?? config.tags;
  const include = options.include;
  const exclude = options.exclude;

  let builder = new AxeBuilder({ page });

  if (tags.length > 0) {
    builder = builder.withTags(tags);
  }

  if (options.rules && options.rules.length > 0) {
    builder = builder.withRules(options.rules);
  }

  if (include) {
    builder = builder.include(include);
  }

  if (exclude) {
    builder = builder.exclude(exclude);
  }

  if (options.axeOptions) {
    builder = builder.options(options.axeOptions);
  }

  return builder.analyze();
}

/**
 * Assert that violations are within configured thresholds.
 *
 * @param {import('axe-core').AxeResults} results - axe scan results
 * @param {object} [thresholds] - Override thresholds (default from axe.config.json)
 * @param {number} [thresholds.critical] - Max allowed critical violations
 * @param {number} [thresholds.serious] - Max allowed serious violations
 * @param {number} [thresholds.moderate] - Max allowed moderate violations
 * @param {number} [thresholds.minor] - Max allowed minor violations
 * @returns {{ pass: boolean, summary: object, violations: object[] }}
 */
export function assertA11yThresholds(results, thresholds) {
  const config = loadAxeConfig();
  const t = { ...config.thresholds, ...thresholds };

  const counts = { critical: 0, serious: 0, moderate: 0, minor: 0 };
  for (const v of results.violations) {
    if (v.impact in counts) {
      counts[v.impact] += v.nodes.length;
    }
  }

  const pass =
    counts.critical <= t.critical &&
    counts.serious <= t.serious &&
    counts.moderate <= t.moderate &&
    counts.minor <= t.minor;

  return {
    pass,
    summary: {
      total: results.violations.length,
      totalNodes: results.violations.reduce((s, v) => s + v.nodes.length, 0),
      counts,
      thresholds: t,
    },
    violations: results.violations.map((v) => ({
      id: v.id,
      impact: v.impact,
      description: v.description,
      help: v.help,
      helpUrl: v.helpUrl,
      tags: v.tags,
      nodeCount: v.nodes.length,
      nodes: v.nodes.slice(0, 5).map((n) => ({
        html: n.html.slice(0, 200),
        target: n.target,
        failureSummary: n.failureSummary,
      })),
    })),
  };
}

/**
 * Format a human-readable a11y report for console output.
 *
 * @param {object} assertionResult - Output from assertA11yThresholds
 * @returns {string}
 */
export function formatA11yReport(assertionResult) {
  const { pass, summary, violations } = assertionResult;
  const lines = [];

  lines.push('='.repeat(60));
  lines.push('  axe-core Accessibility Report');
  lines.push('='.repeat(60));
  lines.push('');
  lines.push(`  Result: ${pass ? 'PASS ✓' : 'FAIL ✗'}`);
  lines.push('');
  lines.push('  Violation Counts:');
  lines.push(`    critical : ${summary.counts.critical} / ${summary.thresholds.critical} allowed`);
  lines.push(`    serious  : ${summary.counts.serious} / ${summary.thresholds.serious} allowed`);
  lines.push(`    moderate : ${summary.counts.moderate} / ${summary.thresholds.moderate} allowed`);
  lines.push(`    minor    : ${summary.counts.minor} / ${summary.thresholds.minor} allowed`);
  lines.push('');
  lines.push(`  Total violation groups: ${summary.total}`);
  lines.push(`  Total affected nodes  : ${summary.totalNodes}`);

  if (violations.length > 0) {
    lines.push('');
    lines.push('  Top Violations:');
    for (const v of violations.slice(0, 10)) {
      lines.push(`    [${v.impact}] ${v.id} (${v.nodeCount} nodes)`);
      lines.push(`      ${v.help}`);
      lines.push(`      ${v.helpUrl}`);
    }
  }

  lines.push('');
  lines.push('='.repeat(60));
  return lines.join('\n');
}

/**
 * Write a JSON report file.
 *
 * @param {object} assertionResult - Output from assertA11yThresholds
 * @param {string} outputPath - Output file path (default: test-results/a11y-report.json)
 */
export function writeA11yJsonReport(assertionResult, outputPath) {
  const configPath = process.env.A11Y_REPORT_JSON;
  const resolved = outputPath || configPath || 'test-results/a11y-report.json';

  const dir = path.dirname(resolved);
  mkdirSync(dir, { recursive: true });

  const report = {
    timestamp: new Date().toISOString(),
    pass: assertionResult.pass,
    summary: assertionResult.summary,
    violations: assertionResult.violations,
  };

  writeFileSync(resolved, JSON.stringify(report, null, 2), 'utf-8');
  return resolved;
}
