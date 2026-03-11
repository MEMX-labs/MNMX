# Contributing to MNMX Core

Thank you for your interest in contributing to MNMX. This document describes
the development workflow, code standards, and review process.

## Development Environment Setup

1. Clone the repository:

```bash
git clone https://github.com/mnmx-protocol/mnmx-core.git
cd mnmx-core
```

2. Install dependencies:

```bash
npm ci
```

3. Verify the build and test suite pass:

```bash
npm run build
npm test
```

### Requirements

- Node.js 20 or later
- npm 10 or later
- TypeScript 5.9+
- Rust 1.75+ and Cargo (for the engine crate)
- Python 3.10+ (for the Python SDK)

### Rust Engine

```bash
cd engine
cargo build
cargo test
```

### Python SDK

```bash
cd sdk/python
pip install -e ".[dev]"
pytest -v
```

## Code Style

MNMX enforces strict TypeScript conventions. All contributions must adhere to
the following rules:

- **Strict mode**: `tsconfig.json` enables `strict: true`. Do not weaken it.
- **No `any`**: Every value must have a concrete type. Use `unknown` when the
  type is genuinely indeterminate, then narrow with type guards.
- **Explicit return types**: All exported functions and methods must declare
  their return type.
- **Readonly by default**: Prefer `readonly` properties and `ReadonlyArray`
  parameters. Mutate only when necessary and document why.
- **No default exports**: Use named exports exclusively.
- **Imports**: Use the `.js` extension in relative import paths (required by
  the ESM module system).

## Testing

All tests are written with [Vitest](https://vitest.dev/).

- Every new module must ship with a corresponding test file under `tests/`.
- Tests must be deterministic. Do not depend on network calls, wall-clock
  time, or random values without seeding.
- Run the full suite before submitting a pull request:

```bash
npm test
```

- To run tests in watch mode during development:

```bash
npm run test:watch
```

### Coverage Expectations

- New utility and engine code: aim for 90%+ line coverage.
- Solana integration code that wraps RPC calls: test the logic around the
  call, mock the transport layer.

## Pull Request Process

1. **Fork and branch**: Create a feature branch from `main`. Use a
   descriptive name such as `feat/pool-analyzer` or `fix/tt-eviction`.

2. **Small, focused PRs**: Each pull request should address a single concern.
   If your change touches multiple subsystems, split it into separate PRs.

3. **Describe the change**: The PR description must explain *what* changed
   and *why*. Link any related issues.

4. **Pass CI**: The GitHub Actions workflow must pass before review.

5. **Review**: At least one maintainer approval is required before merging.

6. **Squash merge**: PRs are squash-merged into `main` to keep a linear
   history.

## Commit Message Format

MNMX uses [Conventional Commits](https://www.conventionalcommits.org/).

```
<type>(<scope>): <description>

[optional body]

[optional footer(s)]
```

### Types

| Type       | Purpose                                      |
|------------|----------------------------------------------|
| `feat`     | A new feature                                |
| `fix`      | A bug fix                                    |
| `docs`     | Documentation changes only                   |
| `refactor` | Code change that neither fixes nor adds      |
| `test`     | Adding or updating tests                     |
| `chore`    | Build system, CI, or tooling changes         |
| `perf`     | Performance improvement                      |

### Scope

Use the top-level directory name: `engine`, `router`, `bridges`, `sdk`, `types`, `utils`.

### Examples

```
feat(engine): add quiescence search at leaf nodes
fix(solana): handle null account info from RPC
test(engine): add move ordering efficiency benchmarks
chore(ci): upgrade Node.js to v22
```

## Reporting Issues

Use the issue templates in this repository. If you discover a security
vulnerability, do **not** open a public issue. Instead, follow the process
described in [SECURITY.md](./SECURITY.md).

## Code of Conduct

Be respectful, constructive, and professional. Harassment and bad-faith
engagement will not be tolerated.

## Questions

If you have questions about contributing, open a discussion on the repository
or reach out on [X](https://x.com/mnmx_protocol).
