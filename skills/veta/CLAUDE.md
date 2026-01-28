# Veta Skill Maintenance Guide

## Purpose

This skill teaches Claude to use Veta for persistent memory - notes that survive conversation compaction and enable context to be preserved across sessions.

## Key Principles

### 1. Proactive Memory Writes

The skill emphasizes **writing memory before you forget**, not waiting until compaction is imminent. This is the #1 behavior change to reinforce.

### 2. Meaningful Tags

Tags are the primary organization mechanism. The skill includes tag conventions to encourage consistent categorization.

### 3. Good Note Quality

Notes should include rationale/context, not just bare facts. "What" AND "why".

## File Structure

```
skills/veta/
├── SKILL.md      # Main skill file Claude reads
├── CLAUDE.md     # This file - for maintainers
└── README.md     # Human documentation
```

## What Belongs in SKILL.md

| Content Type | Include? | Reason |
|--------------|----------|--------|
| Trigger scenarios | ✅ Yes | Must be in frontmatter description |
| CLI command examples | ✅ Yes | Agents need syntax reference |
| Tag conventions | ✅ Yes | Encourages consistency |
| Best practices | ✅ Yes | Behavioral guidance |
| Worker/HTTP API | ❌ No | Not relevant to agent usage |
| Architecture details | ❌ No | Not needed for using veta |
| Installation | ❌ No | Assumed already available |

## Updating the Skill

### When to Update

- New CLI commands or options added
- Discovered better usage patterns
- Agents consistently misuse veta in a predictable way

### Update Checklist

```
[ ] Keep SKILL.md under ~1000 words (token budget)
[ ] Test trigger scenarios still match description
[ ] Ensure CLI examples work with current veta version
[ ] Update README.md if user-facing docs change
```

## Testing Changes

After updating the skill:

```bash
# Verify word count is reasonable
wc -w skills/veta/SKILL.md  # Target: 600-1000 words

# Test CLI examples still work
veta init
veta add --title "Test" --tags "test" --body "test body"
veta tags
veta ls --tags test
veta grep "Test"
```

## Common Issues

### Agent doesn't write notes proactively

The description frontmatter needs stronger trigger words. Ensure scenarios like "learn something important" and "make a decision" are explicit triggers.

### Notes have poor tags

Add more tag convention examples to SKILL.md. Consider adding a "bad tag" anti-pattern section.

### Notes lack context/rationale

Add more "good note" examples showing the WHY, not just the WHAT.
