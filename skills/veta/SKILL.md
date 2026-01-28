---
name: veta
description: >
  Persistent memory and knowledge base for agents. Use PROACTIVELY when:
  (1) user says "remember this" or "don't forget",
  (2) you learn something important about the project/user/preferences,
  (3) you make a decision that should survive compaction,
  (4) you discover technical gotchas or non-obvious behavior,
  (5) you need to recall context from previous sessions,
  (6) work spans multiple sessions or days,
  (7) you're about to do exploratory work where discoveries should be preserved.
  Write memory BEFORE you forget - don't wait for compaction.
---

# Veta - Persistent Memory for Agents

Veta is a note database that survives conversation compaction. Notes have titles, bodies, tags for organization, and optional references to external resources (source code paths, URLs, documentation links). **Write notes proactively** - if something is worth remembering, write it NOW.

## ⚠️ CRITICAL: Write Memory Proactively

**Don't wait to be asked.** If you learn something important, write it immediately:

| When you... | Write a note about... |
|-------------|----------------------|
| Learn a user preference | Their preference (coding style, communication style, tool choices) |
| Discover a project gotcha | The gotcha and workaround |
| Make a non-obvious decision | The decision AND rationale |
| Complete significant work | Summary of what was done and why |
| Find undocumented behavior | The behavior you discovered |
| Hit a tricky bug | The bug and fix (future you will hit it again) |

**The test:** "Will this be useful context in 2 weeks?" → YES = write it now.

## ⚠️ ALWAYS Include References

**References are how future-you finds the code again.** When writing notes about code, bugs, or technical decisions, ALWAYS include `--references` pointing to:

- **Source code locations:** `src/auth/token.rs:142` - the exact file and line
- **URLs:** `https://docs.rs/jsonwebtoken` - documentation you consulted
- **Related files:** `tests/auth_test.rs,src/config.rs` - other relevant code

Without references, notes become "I fixed a bug somewhere" instead of "I fixed a bug HERE."

```bash
# ❌ BAD: No references - where was this bug?
veta add --title "Fixed auth bug" --tags "debugging" \
  --body "Token wasn't refreshing properly"

# ✅ GOOD: References point exactly where to look
veta add --title "Fixed auth bug" --tags "debugging" \
  --body "Token wasn't refreshing properly" \
  --references "src/auth/token.rs:142,src/middleware/auth.rs:58"
```

**When to add references:**
- Bug fixes → file:line where the fix was made
- Gotchas → file:line where the gotcha manifests  
- Architecture decisions → files affected by the decision
- API behaviors → URL to the documentation
- Stack Overflow solutions → URL to the answer

## When to Use Veta

**Use veta when:**
- User says "remember this" or asks you to note something
- You learn project conventions, architecture decisions, or user preferences
- You discover gotchas, workarounds, or non-obvious behaviors
- Work spans multiple sessions (need to preserve context)
- You make decisions that should be explained to future Claude
- You're doing research/exploration with valuable discoveries

**Don't use veta for:**
- Temporary task tracking (use TodoWrite or beads instead)
- Things already documented in README/code comments
- Transient session state

## CLI Reference

### Initialize (one-time setup)

```bash
veta init
```

### Add a note (always include meaningful tags!)

```bash
# Short notes
veta add --title "User prefers explicit types" --tags "preferences,coding-style" \
  --body "User wants explicit type annotations, not inferred types"

# Long notes via stdin
echo "Detailed explanation..." | veta add --title "API rate limiting" --tags "gotchas,api"

# Include references to source code, URLs, or docs for context
veta add --title "Auth token refresh bug" --tags "debugging,auth" \
  --body "Token refresh was failing silently. Fixed by checking expiry before refresh." \
  --references "src/auth/token.rs:142,https://docs.rs/jsonwebtoken"
```

References are optional comma-separated pointers to source code locations (e.g., `src/file.rs:42`), URLs, or documentation links that provide context for the note.

### Find and read notes

```bash
# List all tags to see what's stored
veta tags

# List notes in a tag
veta ls --tags preferences
veta ls --tags gotchas,debugging

# Read a specific note
veta show 42

# Read multiple notes at once
veta show 1 2 3

# Show only first n lines of body (useful for long notes)
veta show 42 -n 10
veta show 1 2 3 -n 5

# Search notes
veta grep "authentication"
veta grep "postgres" --tags debugging
```

### Update notes (keep them current!)

```bash
# Update body
echo "Updated content..." | veta edit 42

# Update metadata
veta edit 42 --title "New title" --tags "new,tags"

# Update references
veta edit 42 --references "src/new_location.rs:100,https://new-docs.example.com"
```

### Delete outdated notes

```bash
veta delete 42
```

## Best Practices

### Tag Conventions

Use consistent, descriptive tags:

| Tag | Use for |
|-----|---------|
| `preferences` | User coding style, tool choices, communication preferences |
| `decisions` | Architectural decisions with rationale |
| `gotchas` | Non-obvious behaviors, common mistakes, workarounds |
| `debugging` | Bug patterns, troubleshooting steps that worked |
| `<project-name>` | Project-specific knowledge |
| `api` | API behaviors, rate limits, authentication patterns |

### Writing Good Notes

**Title:** Make it searchable. Ask "what would I search for to find this?"

**Body:** Include:
- The key fact or decision
- WHY (rationale/context)
- Example if helpful

**References:** Always include for technical notes - future you needs to find the code!

**Example of a good note:**
```bash
veta add --title "Django ORM: select_related vs prefetch_related" \
  --tags "gotchas,django,performance" \
  --body "select_related: use for ForeignKey/OneToOne (single JOIN query)
prefetch_related: use for ManyToMany/reverse FK (separate query, then Python join)

Discovered this when N+1 queries caused 10s page loads. 
Fixed by adding select_related('author') to book queryset." \
  --references "myapp/views.py:45,myapp/models.py:12"
```

## Session Protocol

### Session Start
```bash
# Check what you remember about this project
veta tags
veta ls --tags "$(basename $PWD)"
veta ls --from "3 days ago"
```

### During Work
- Learn something? Write it immediately
- Make a decision? Document the WHY
- Hit a bug? Note the fix

### Session End
```bash
# Review what you learned this session
veta ls --from "today"

# Add any missing context for next session
veta add --title "Session summary: auth refactor" --tags "sessions,auth" \
  --body "Completed JWT -> session cookie migration. 
Still TODO: rate limiting on login endpoint.
Key decision: 15-min session timeout based on security audit." \
  --references "src/auth/session.rs,src/middleware/auth.rs,docs/security-audit.md"
```

## Common Workflows

### Recall Project Context

```bash
# What do I know about this project?
veta tags
veta grep "$(basename $PWD)"

# What gotchas have I hit?
veta ls --tags gotchas

# Recent notes
veta ls --from "1 week ago"
```

### Document a Decision

```bash
veta add --title "Chose SQLite over Postgres for local dev" \
  --tags "decisions,architecture" \
  --body "Rationale:
- Zero setup for new devs
- Tests run faster (in-memory option)
- Good enough for expected data size (<100k rows)

Tradeoff: Some Postgres-specific features unavailable locally" \
  --references "docker-compose.yml,src/db/connection.rs,docs/local-setup.md"
```

### Remember User Preferences

```bash
veta add --title "Code review preferences" \
  --tags "preferences,code-review" \
  --body "- Prefers smaller PRs over large ones
- Wants tests for all new functions
- Likes explicit error handling (no silent failures)
- Uses conventional commits format"
```

## Veta vs Beads

| Veta | Beads |
|------|-------|
| Knowledge/facts/context | Tasks/issues/work items |
| "Remember this" | "Track this work" |
| No dependencies | Has blockers/dependencies |
| Unstructured notes | Structured issues |

**Use both:** Veta for knowledge, Beads for tasks.
