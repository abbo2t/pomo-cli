# pomo-cli

A visual Pomodoro timer for the console.

## Features

- **Colour-coded progress bar** that shrinks from 100 % → 0 % as each session
  elapses
  - 🔴 Red bar for **focus** sessions
  - 🟢 Green bar for **break** sessions
- **Split-colour text**: while the bar covers the label text, it appears as
  **black on the session colour**; once the shrinking bar passes a character,
  that character flips to **coloured text on the default background** — giving
  a dynamic countdown feel
- **Audio notification** (three ascending beeps via `rodio`) at the end of
  every session, with automatic fallback to the terminal bell when no audio
  device is available
- Fully configurable durations and session count via CLI flags
- Single-line display — perfect for a **tmux / screen** panel

## Installation

```bash
cargo install --path .
```

Or build manually:

```bash
cargo build --release
# binary is at ./target/release/pomo
```

On Linux `libasound2-dev` (ALSA) must be present for audio support:

```bash
sudo apt-get install libasound2-dev   # Debian/Ubuntu
```

## Usage

```
pomo [OPTIONS]
```

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--work` | `-w` | `25` | Focus session length in minutes |
| `--short-break` | `-b` | `5` | Short break length in minutes |
| `--long-break` | `-l` | `15` | Long break length (after every *N* sessions) |
| `--sessions` | `-n` | `4` | Sessions before a long break |
| `--no-sound` | | | Disable audio notifications |

### Examples

```bash
# Default Pomodoro: 25 min work / 5 min break / 15 min long break
pomo

# 50/10 style
pomo --work 50 --short-break 10

# Quiet mode (no beep)
pomo --no-sound

# Quick test with 1-minute sessions
pomo -w 1 -b 1 -n 2
```

## Session cycle

```
Work ──► Short break ──► Work ──► … ──► Work (Nth) ──► Long break ──► (repeat)
```

The counter shown in the label resets after each long break.
