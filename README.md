# Frankie Goes to Code Review 🎸

> Making code review easy and fun in an agentic world

[![License: ISC](https://img.shields.io/badge/License-ISC-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2024-orange.svg)](
rust-toolchain.toml)
[![Phase](https://img.shields.io/badge/phase-2%2F5-green.svg)](docs/roadmap.md)

## What is Frankie?

Frankie is a terminal-based code review assistant that brings your GitHub pull
requests to life in your favourite environment: the command line. Built for
developers who live in the terminal and dream of a world where AI helps us
write better code together.

**Current Status:** 🚧 Work in Progress (Phases 1-2 Complete)

Frankie already delivers:

- 🔍 Interactive TUI for navigating PR comments with keyboard-driven filters
- 💾 Smart local caching with automatic background refresh
- 🏠 Auto-discovery of your local Git repositories
- ⚡ Fast, offline-friendly workflow with SQLite persistence

**Coming Soon:**

- 📝 Comment detail view with syntax-highlighted code context
- 🤖 AI-assisted review workflows via Codex CLI integration
- 💬 Template-based reply automation
- 🔄 Full offline mode with operation queuing

## Why Frankie?

Code review in an agentic world should be:

- **Fast**: Local-first execution with smart caching
- **Fun**: Terminal UI that respects your workflow, not browser tabs
- **Powerful**: AI assistance for context, not replacement for judgement
- **Private**: Your code stays local; you control what goes where

Frankie is the experiment in making code review feel less like homework and
more like collaboration. It's built to be the GitHub PR adapter in the larger
[Corbusier](https://github.com/leynos/corbusier) architecture project.

## Quick Start

### Installation

```bash
# Clone and build from source (binary releases coming soon)
git clone https://github.com/leynos/frankie.git
cd frankie
make build
```

### Basic Usage

```bash
# Review a specific PR in interactive TUI mode
export FRANKIE_TOKEN="your_github_token"
frankie --tui --pr-url https://github.com/owner/repo/pull/123

# Auto-discover repository from your current directory
cd /path/to/your/repo
frankie --tui

# List all PRs for a repository
frankie --owner octocat --repo hello-world
```

### First Time Setup

1. **Get a GitHub token**:
   [Create a personal access token](https://github.com/settings/tokens) with
   `repo` access
2. **Set your token**: Export as `FRANKIE_TOKEN` or `GITHUB_TOKEN` environment
   variable
3. **Optional - Enable caching**:

   ```bash
   frankie --migrate-db --database-url frankie.sqlite
   ```

That's it! You're ready to start reviewing.

## Features

### ✅ Available Now (Phases 1-2)

#### Interactive Terminal UI

Navigate pull request comments with vim-style keybindings (`j`/`k`), filters
(unresolved, by file, by reviewer), and automatic 30-second background refresh.

#### Local Repository Discovery

Frankie auto-detects GitHub repositories from your local Git config—no need to
copy-paste URLs when you're already in the right directory.

#### Smart Caching

SQLite-based local cache with TTL expiry and HTTP ETag validation. Work
offline, sync when online, never wait for GitHub API limits.

#### Multiple Operation Modes

- Single PR deep-dive
- Repository-wide PR listing
- Interactive auto-discovery
- Full-screen TUI review interface

### 🚧 In Progress (Phases 3-5)

- **Syntax-highlighted code context**: See the exact code being discussed
- **AI-assisted workflows**: Codex CLI integration for automated comment
  resolution
- **Reply templates**: Quick responses to common review patterns
- **Offline-first mode**: Queue operations when disconnected
- **Accessibility theming**: WCAG AA compliant colour schemes

See the [full roadmap](docs/roadmap.md) for detailed timeline and completion
criteria.

## Documentation

- **[User Guide](docs/users-guide.md)**: Complete reference for all operation
  modes, configuration, and keyboard shortcuts
- **[Roadmap](docs/roadmap.md)**: Detailed 5-phase delivery plan with
  measurable completion criteria
- **[Design Document](docs/frankie-design.md)**: Full technical design and
  architectural decisions

Additional guides for contributors:

- [Building Terminal UIs with bubbletea-rs](docs/building-idiomatic-terminal-uis-with-bubbletea-rs.md)
- [Testing Strategy](docs/two-tier-testing-strategy-for-an-octocrab-github-client.md)
- [Development Standards](AGENTS.md)

## Building from Source

Frankie requires Rust 2024 edition (specified in `rust-toolchain.toml`).

```bash
# Build
make build

# Run tests
make test

# Run all quality checks (lint, format, typecheck)
make all
```

See [AGENTS.md](AGENTS.md) for complete development guidelines and quality
gates.

## Contributing

Frankie is an active experiment in making code review better. Contributions,
ideas, and feedback are welcome!

**Current Focus:** We're working on Phase 3 (AI-assisted workflows). Check the
[roadmap](docs/roadmap.md) for specific tasks.

Before submitting PRs:

- Run `make all` to ensure all quality gates pass
- Read [AGENTS.md](AGENTS.md) for code standards and testing requirements
- Consult the [design document](docs/frankie-design.md) for architectural
  context

**Not sure where to start?** Open an issue to discuss ideas or improvements.

## Project Context

Frankie serves as the GitHub PR adapter component in the broader
[Corbusier](https://github.com/leynos/corbusier) architecture project,
exploring how to build developer tools for an AI-assisted future.

**Architecture Stack:**

- **Language**: Rust (2024 edition)
- **TUI Framework**: [bubbletea-rs](https://github.com/unrenamed/bubbletea-rs)
- **GitHub API**: [octocrab](https://github.com/XAMPPRocky/octocrab)
- **Persistence**: SQLite with Diesel ORM
- **Configuration**: [ortho-config](https://crates.io/crates/ortho_config) for
  unified CLI/env/file config

## Licence

Copyright © 2025 [df12 Productions](https://df12.studio)

Licenced under the ISC Licence. See [LICENCE](LICENSE) for details.

______________________________________________________________________

**Frankie Goes to Code Review** is developed by df12 Productions with ❤️ for
developers who believe code review can be better.
