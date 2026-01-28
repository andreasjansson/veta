# Veta Skill for Claude Code

A Claude Code skill for using [Veta](https://github.com/andreasjansson/veta) as persistent agent memory.

## What This Skill Does

This skill teaches Claude Code to use Veta effectively for:
- **Persistent memory** - Notes that survive conversation compaction
- **Project knowledge** - Documenting gotchas, decisions, and conventions
- **User preferences** - Remembering coding styles and preferences across sessions
- **Session continuity** - Preserving context for multi-day work

## Installation

### Global installation

```bash
mkdir -p ~/.claude/skills
ln -s /path/to/veta/skills/veta ~/.claude/skills/veta
```

### Project-local installation

```bash
mkdir -p .claude/skills
cp -r /path/to/veta/skills/veta .claude/skills/
```

## When Claude Uses This Skill

The skill activates when:
- User says "remember this" or "don't forget"
- Claude learns important project/user information
- Decisions are made that should survive compaction
- Technical gotchas or non-obvious behaviors are discovered
- Work spans multiple sessions

## File Structure

```
veta/
├── SKILL.md      # Main skill file (Claude reads this)
├── CLAUDE.md     # Maintenance guide for updating the skill
└── README.md     # This file (for humans)
```

## Prerequisites

- [Veta CLI](https://github.com/andreasjansson/veta) installed and in PATH
- Initialized database (`veta init` in project root)

## Key Concepts

### Proactive Memory

The most important behavior: **write memory before you forget**. Don't wait for compaction or explicit requests. If something is worth knowing later, write it now.

### Tags for Organization

Veta organizes notes by tags. Use consistent tags:

| Tag | Use for |
|-----|---------|
| `preferences` | User coding style, tool choices |
| `decisions` | Architectural decisions + rationale |
| `gotchas` | Non-obvious behaviors, workarounds |
| `debugging` | Bug patterns, fixes that worked |
| `<project-name>` | Project-specific knowledge |

### Veta vs Beads

| Veta | Beads |
|------|-------|
| Knowledge and context | Tasks and work tracking |
| Unstructured notes | Structured issues with dependencies |
| "Remember this fact" | "Track this work item" |

Use both: Veta for knowledge, Beads for tasks.

## Contributing

Issues and PRs welcome for:
- Documentation improvements
- New usage patterns
- Better trigger scenarios
- Tag convention suggestions

## License

MIT (same as Veta)
