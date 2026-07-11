
---

## `AGENTS.md`

```markdown
# AGENTS.md

# Purpose

This document defines behavioral requirements for AI contributors.

Project architecture, goals, and system ownership are defined in ARCHITECTURE.md.

Game design direction (combat feel, progression, AI, economy) is defined in DESIGN.md.

When conflicts occur:

ARCHITECTURE.md takes precedence for system structure.

DESIGN.md takes precedence for intended player experience when ADRs do not yet cover a topic.

Accepted ADRs take precedence over DESIGN.md for implemented behavior.

---

# Required Reading Order

Before making design or implementation decisions:

1. Read ARCHITECTURE.md
2. Read DESIGN.md (game design direction)
3. Read relevant ADRs
4. Read ROADMAP.md
5. Read BEVY_REFERENCE.md
6. Review existing code
7. Only then propose implementation

Do not assume the user's latest request overrides existing architecture.

If a request appears to conflict with architecture, identify the conflict and explain it.

---

# Primary Responsibility

The goal of an AI contributor is not to complete tasks as quickly as possible.

The goal is to improve the project while preserving architectural integrity.

Prefer solutions that remain maintainable months from now over solutions that only satisfy the immediate request.

---

# Design Process

Before implementing any feature:

1. Determine what system should own the data.
2. Determine what system should own the behavior.
3. Check whether an existing abstraction already solves the problem.
4. Evaluate scalability implications.
5. Implement only after ownership and boundaries are clear.

Do not begin implementation before determining ownership.

---

# Prefer Extension Over Creation

Before creating:

- a new system
- a new manager
- a new service
- a new abstraction

Determine whether an existing system can be extended cleanly.

Avoid duplicate functionality.

Avoid parallel systems that solve the same problem.

---

# Prefer Generic Solutions

Prefer solving categories of problems.

Avoid solving individual examples.

Good:

- route-following system

Bad:

- caravan-specific system

Good:

- resource node system

Bad:

- tree-specific harvesting system

Good:

- needs and assignment system

Bad:

- farmer-specific logic

Build abstractions that support multiple future features.

---

# Data First

Persistent game concepts should be represented as data first.

Do not assume ECS entities are the correct representation of persistent state.

When introducing a feature:

Ask:

> What data must survive if the entity disappears?

Design around that data.

---

# Groundwork Rule

Do not build large systems for future use unless there is a current consumer or roadmap phase requiring them.

Future-facing code must be one of:

- a small interface
- a data type
- a clearly marked placeholder
- an ADR decision

Avoid implementing full systems before they are used.

Build seams, not fake future systems.

---

# Scalability Check

Before implementing a solution, consider:

- world size growth
- chunk count growth
- entity count growth
- simulation growth

Avoid solutions that require scanning the entire world when localized solutions exist.

Prefer:

- chunk-local operations
- cached lookups
- query systems
- event-driven updates

over global iteration.

---

# Performance Philosophy

Performance is important.

Premature optimization is not.

Required order:

1. Correct architecture
2. Scalable architecture
3. Profiling
4. Optimization

Do not introduce complexity without evidence.

---

# Renderer Policy

Do not introduce renderer-specific complexity unless profiling demonstrates a need.

Examples:

- custom shaders
- custom render pipelines
- GPU-driven systems
- custom instancing systems

These are optimization tools, not architectural foundations.

Prefer existing engine capabilities first.

---

# Future Compatibility Check

Before introducing a design, ask:

- Does this increase coupling?
- Does this duplicate existing functionality?
- Does this make future simulation harder?
- Does this make future pathfinding harder?
- Does this make future multiplayer harder?
- Does this make future persistence harder?

If the answer is yes, reconsider the design.

---

# Simplicity Rule

When multiple solutions satisfy the requirements:

Prefer:

- fewer systems
- fewer dependencies
- cleaner ownership
- simpler data flow

Do not introduce complexity unless it solves a demonstrated problem.

---

# Refactoring Rule

Do not preserve poor architecture for the sake of minimizing code changes.

If an existing design is inconsistent with project architecture:

1. Identify the issue.
2. Explain the tradeoffs.
3. Recommend the cleaner solution.

Long-term maintainability is more important than minimizing short-term edits.

---

# Communication Requirements

When proposing a solution:

Explain:

- ownership
- affected systems
- scalability implications
- future compatibility implications

Do not justify a design solely because it works.

Explain why it belongs where it belongs.

---

# When Unsure

Prefer:

- simpler solutions
- cleaner ownership
- reusable abstractions
- data-driven designs

Avoid:

- special-case systems
- duplicated functionality
- unnecessary complexity

When uncertainty exists, choose the solution that preserves architectural flexibility.