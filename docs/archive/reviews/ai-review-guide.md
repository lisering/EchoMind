# EchoMind AI Code Review Integration

This document describes the AI-powered code review integration for the EchoMind project.

## Overview

EchoMind uses Claude Code Action for automated AI code review in GitHub Actions. This integration provides:

- **Automated Security Review**: Detects SQL injection, XSS, command injection, path traversal, and hardcoded secrets
- **Performance Analysis**: Identifies N+1 queries, blocking I/O, and unnecessary allocations
- **Architecture Compliance**: Verifies hexagonal architecture and dependency direction
- **Code Quality**: Reviews Rust best practices, error handling, and documentation
- **Frontend Review**: Checks DOM manipulation, event handling, and state management

## Setup

### 1. GitHub Secrets Required

Add the following secrets to your GitHub repository:

- `ANTHROPIC_API_KEY`: Your Anthropic API key for Claude Code

### 2. Installation

The AI review integration is automatically configured when you push to the repository. The workflow files are located in:

- `.github/workflows/ci.yml` - Main CI pipeline with AI review step
- `.github/workflows/ai-review.yml` - Dedicated AI review workflow

### 3. Configuration Files

- `.ai-review-config.json` - Review configuration and settings
- `.ai-review-rules.md` - Detailed review rules and guidelines

## Usage

### Automatic Review

AI review runs automatically on:

1. **Pull Requests**: Every PR gets an automated review
2. **Comments**: Mention `@claude` in a PR comment to trigger a review
3. **Push to Main**: Additional review on main branch updates

### Manual Review

Run the local review script:

```bash
# Review staged changes
git add .
./scripts/ai-review.sh

# Review last commit
git commit -m "feature: add new functionality"
./scripts/ai-review.sh
```

### Triggering Review via Comment

In a PR comment, mention:

```
@claude Please review this PR for security issues
```

or

```
@claude Review the performance implications of these changes
```

## Review Process

### 1. Automated Analysis

The AI reviewer analyzes:

- **Security**: SQL injection, XSS, command injection, path traversal, hardcoded secrets
- **Performance**: N+1 queries, blocking I/O, unnecessary allocations
- **Architecture**: Hexagonal architecture compliance, dependency direction
- **Rust**: unwrap/expect usage, error handling, async safety
- **Code Quality**: Naming, documentation, test coverage, SOLID principles
- **Frontend**: DOM manipulation, event handling, state management

### 2. Issue Classification

Issues are classified by severity:

- **CRITICAL**: Must fix before merge (security vulnerabilities, data loss, crashes)
- **HIGH**: Should fix before merge (performance issues, logic errors, architecture violations)
- **MEDIUM**: Consider fixing (code quality, maintainability, best practices)
- **LOW**: Optional improvements (style, documentation, minor optimizations)

### 3. Output Format

Review results are provided in Markdown format:

```markdown
## AI Code Review Summary

### Critical Issues (Must Fix)
- [CRITICAL] **File:line** - Description
  ```suggestion
  // Suggested fix
  ```

### High Priority Issues (Should Fix)
- [HIGH] **File:line** - Description

### Medium Priority Issues (Consider)
- [MEDIUM] **File:line** - Description

### Low Priority Issues (Optional)
- [LOW] **File:line** - Description
```

## Configuration

### Review Settings

Edit `.ai-review-config.json` to customize:

```json
{
  "review_settings": {
    "model": "claude-4-0-sonnet-20250805",
    "max_turns": 10,
    "timeout_minutes": 15,
    "language": "en"
  },
  "severity_levels": {
    "critical": {
      "fail_ci": true,
      "auto_fix": false
    }
  }
}
```

### Review Rules

Edit `.ai-review-rules.md` to add or modify review rules:

```markdown
### New Rule: Rule Name
- **Rule**: Description of the rule
- **Severity**: CRITICAL/HIGH/MEDIUM/LOW
- **Check**: What to look for
- **Example**: Code examples
```

## Integration with CI

### CI Pipeline

The AI review is integrated into the CI pipeline:

1. **Lint Step**: Runs clippy and fmt checks
2. **AI Review Step**: Runs after lint passes
3. **Test Step**: Runs after AI review
4. **Build Step**: Runs after tests pass

### Failure Handling

- **Critical Issues**: CI fails immediately
- **High Issues**: CI fails
- **Medium Issues**: CI passes with warnings
- **Low Issues**: CI passes

### Auto-Fix

The AI review can automatically fix:

- Clippy warnings
- Formatting issues
- Unused imports
- Missing documentation

## Best Practices

### For Developers

1. **Review AI Suggestions**: Always review AI suggestions before applying
2. **Test Changes**: Run tests after applying AI fixes
3. **Understand Changes**: Don't blindly apply suggestions; understand why they're needed
4. **Provide Context**: Give clear PR descriptions to help AI understand context

### For Reviewers

1. **Focus on Critical Issues**: Prioritize security and correctness
2. **Verify AI Findings**: AI may have false positives; verify issues
3. **Suggest Alternatives**: Provide better solutions when AI suggestions are suboptimal
4. **Update Rules**: Add new rules based on common issues found

## Troubleshooting

### Common Issues

1. **AI Review Not Running**
   - Check if `ANTHROPIC_API_KEY` secret is set
   - Verify GitHub Actions permissions
   - Check workflow file syntax

2. **False Positives**
   - Update review rules to exclude patterns
   - Add exclusions in `.ai-review-config.json`
   - Provide more context in PR descriptions

3. **Performance Issues**
   - Reduce `max_turns` in configuration
   - Exclude large files from review
   - Use path filters in workflow

### Debugging

Enable debug logging:

```bash
# Run with verbose output
ANTHROPIC_LOG=debug ./scripts/ai-review.sh
```

Check review artifacts:

```bash
# View review results
cat .ai-review/review-report.md
```

## Cost Management

### API Usage

- Each review uses approximately 10-20 API calls
- Typical cost: $0.10-0.50 per review
- Cost depends on PR size and complexity

### Optimization

1. **Path Filters**: Only review changed files
2. **Size Limits**: Skip reviews for very large PRs
3. **Caching**: Cache review results for unchanged code
4. **Batching**: Review multiple related PRs together

## Security Considerations

### Data Privacy

- Code is sent to Anthropic's API for review
- No code is stored permanently
- Review results are stored in GitHub artifacts

### Access Control

- Only repository admins can configure AI review
- Review results are visible to all contributors
- API keys are stored securely in GitHub Secrets

### Best Practices

1. **Never commit API keys** to source code
2. **Use environment variables** for sensitive data
3. **Regularly rotate API keys**
4. **Monitor API usage** for unusual activity

## Future Enhancements

### Planned Features

1. **Custom Rules Engine**: Allow teams to define custom review rules
2. **Integration with Issue Trackers**: Automatically create issues for critical findings
3. **Learning from Feedback**: Improve suggestions based on developer feedback
4. **Multi-Language Support**: Extend beyond Rust and JavaScript

### Contributing

To contribute to the AI review system:

1. Review the existing rules in `.ai-review-rules.md`
2. Add new rules for common issues
3. Test rules with sample code
4. Submit a PR with your improvements

## Support

For issues or questions:

1. Check the [Claude Code documentation](https://claude.ai/code)
2. Review the [AI review rules](.ai-review-rules.md)
3. Open an issue in the repository
4. Contact the maintainers