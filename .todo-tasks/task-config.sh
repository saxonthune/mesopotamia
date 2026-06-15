#!/usr/bin/env bash
# ─── Task System Configuration ───────────────────────────────────────────────
# Project-specific settings for the todo-task execution system.
# Sourced by execute-plan.sh and status.sh.
#
# Copy this file to your project:
#   .todo-tasks/task-config.sh
# Then edit the values below to match your project.
#
# Per-task verification commands live in each plan's ## Verification section
# (a fenced bash block). This file is for project-wide operational settings only.

# Prefix for agent worktree directories (created alongside the repo root)
WORKTREE_PREFIX="agent"

# Budget caps for headless Claude sessions (USD)
MAX_BUDGET="5.00"
RETRY_BUDGET="3.00"

# Maximum retry attempts when build/test fails
MAX_RETRIES=4
