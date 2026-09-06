#!/bin/bash

# EchoMind AI Code Review Script
# This script runs AI code review locally for testing and validation

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
AI_REVIEW_CONFIG=".ai-review-config.json"
AI_REVIEW_RULES=".ai-review-rules.md"
REVIEW_OUTPUT_DIR=".ai-review"
REVIEW_REPORT="$REVIEW_OUTPUT_DIR/review-report.md"

echo -e "${BLUE}=== EchoMind AI Code Review ===${NC}"
echo -e "${BLUE}Starting AI code review process...${NC}"

# Check if Claude CLI is available
if ! command -v claude &> /dev/null; then
    echo -e "${RED}Error: Claude CLI not found. Please install Claude Code first.${NC}"
    echo "Visit https://claude.ai/code for installation instructions."
    exit 1
fi

# Check for API key
if [ -z "$ANTHROPIC_API_KEY" ]; then
    echo -e "${RED}Error: ANTHROPIC_API_KEY environment variable not set.${NC}"
    echo "Please set your Anthropic API key:"
    echo "  export ANTHROPIC_API_KEY='your-api-key'"
    exit 1
fi

# Check if config file exists
if [ ! -f "$AI_REVIEW_CONFIG" ]; then
    echo -e "${RED}Error: AI review config file not found: $AI_REVIEW_CONFIG${NC}"
    exit 1
fi

# Check if rules file exists
if [ ! -f "$AI_REVIEW_RULES" ]; then
    echo -e "${RED}Error: AI review rules file not found: $AI_REVIEW_RULES${NC}"
    exit 1
fi

# Create output directory
mkdir -p "$REVIEW_OUTPUT_DIR"

# Get the diff for review
echo -e "${YELLOW}Getting diff for review...${NC}"
DIFF=$(git diff --cached 2>/dev/null || git diff HEAD~1 2>/dev/null || git diff)

if [ -z "$DIFF" ]; then
    echo -e "${YELLOW}No changes detected for review.${NC}"
    exit 0
fi

# Save diff to file
echo "$DIFF" > "$REVIEW_OUTPUT_DIR/current.diff"

# Read review rules
echo -e "${YELLOW}Reading review rules...${NC}"
REVIEW_RULES=$(cat "$AI_REVIEW_RULES")

# Prepare the review prompt
REVIEW_PROMPT="You are reviewing code changes for the EchoMind project.

## Project Context
EchoMind is a Rust + Tauri v2 desktop application for local AI knowledge base with RAG capabilities.

## Review Rules
$REVIEW_RULES

## Changes to Review
\`\`\`diff
$DIFF
\`\`\`

## Instructions
1. Analyze the changes according to the review rules
2. Identify issues by severity (CRITICAL > HIGH > MEDIUM > LOW)
3. Provide specific file paths and line numbers
4. Suggest fixes for critical and high issues
5. Follow the output format specified in the rules

Please provide a comprehensive review."

# Run the AI review
echo -e "${BLUE}Running AI code review...${NC}"
REVIEW_RESULT=$(ANTHROPIC_API_KEY="$ANTHROPIC_API_KEY" claude -p "$REVIEW_PROMPT" 2>&1) || {
    echo -e "${RED}Error: AI review failed.${NC}"
    echo "$REVIEW_RESULT"
    exit 1
}

# Save the review result
echo "$REVIEW_RESULT" > "$REVIEW_REPORT"

# Parse and display results
echo -e "${GREEN}=== AI Review Complete ===${NC}"
echo -e "${BLUE}Review report saved to: $REVIEW_REPORT${NC}"

# Count issues by severity
CRITICAL_COUNT=$(echo "$REVIEW_RESULT" | grep -c "\[CRITICAL\]" || true)
HIGH_COUNT=$(echo "$REVIEW_RESULT" | grep -c "\[HIGH\]" || true)
MEDIUM_COUNT=$(echo "$REVIEW_RESULT" | grep -c "\[MEDIUM\]" || true)
LOW_COUNT=$(echo "$REVIEW_RESULT" | grep -c "\[LOW\]" || true)

echo -e "${BLUE}Issue Summary:${NC}"
echo -e "  ${RED}Critical: $CRITICAL_COUNT${NC}"
echo -e "  ${YELLOW}High: $HIGH_COUNT${NC}"
echo -e "  ${BLUE}Medium: $MEDIUM_COUNT${NC}"
echo -e "  ${GREEN}Low: $LOW_COUNT${NC}"

# Exit with error if critical or high issues found
if [ "$CRITICAL_COUNT" -gt 0 ] || [ "$HIGH_COUNT" -gt 0 ]; then
    echo -e "${RED}Critical or high issues found. Please fix before merging.${NC}"
    exit 1
fi

echo -e "${GREEN}No critical or high issues found. Review passed.${NC}"
exit 0