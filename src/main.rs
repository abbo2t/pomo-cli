use std::io::{self, Write, stdout};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::thread;

use clap::Parser;
use crossterm::{
    cursor,
    execute,
    queue,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal,
};

/// A visual Pomodoro timer for the console.
///
/// Displays a colour-coded progress bar that shrinks from 100% to 0%
/// as each session elapses.  Red = focus, green = break.
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Focus session length in minutes
    #[arg(short, long, default_value_t = 25)]
    work: u64,

    /// Short break length in minutes
    #[arg(short = 'b', long, default_value_t = 5)]
    short_break: u64,

    /// Long break length in minutes (after every N focus sessions)
    #[arg(short = 'l', long, default_value_t = 15)]
    long_break: u64,

    /// Number of focus sessions before a long break
    #[arg(short = 'n', long, default_value_t = 4)]
    sessions: u32,

    /// Disable audio notifications
    #[arg(long)]
    no_sound: bool,
}

#[derive(Clone, Copy)]
enum SessionKind {
    Work,
    ShortBreak,
    LongBreak,
}

impl SessionKind {
    fn color(self) -> Color {
        match self {
            SessionKind::Work => Color::Red,
            SessionKind::ShortBreak | SessionKind::LongBreak => Color::Green,
        }
    }

    fn label(self) -> &'static str {
        match self {
            SessionKind::Work => "Focus",
            SessionKind::ShortBreak => "Break",
            SessionKind::LongBreak => "Long Break",
        }
    }
}

/// Draw one line: a progress bar that fills `fraction` of the terminal width.
///
/// Characters *inside* the filled region are rendered as **black text on
/// the session colour**; characters *outside* show the **session colour as
/// the foreground** against the default background.
/// The `label` string is centred in the bar.
fn draw_bar(fraction: f64, label: &str, kind: SessionKind) -> io::Result<()> {
    let (term_width, _) = terminal::size()?;
    let term_width = term_width as usize;
    let fill_color = kind.color();

    // Number of cells that are "filled" (coloured background)
    let filled = ((fraction * term_width as f64).round() as usize).min(term_width);

    // Centre the label within the full terminal width
    let label_bytes = label.as_bytes();
    let label_len = label.chars().count();
    let label_start = if term_width >= label_len {
        (term_width - label_len) / 2
    } else {
        0
    };
    let label_end = label_start + label_len;

    let mut out = stdout();
    // Return cursor to column 0 so we redraw in place
    queue!(out, cursor::MoveToColumn(0))?;

    for i in 0..term_width {
        let on_bar = i < filled;

        // Determine the character at this position
        let ch = if i >= label_start && i < label_end {
            // Safety: i - label_start is within [0, label_len)
            char::from(label_bytes[i - label_start])
        } else {
            ' '
        };

        if on_bar {
            queue!(
                out,
                SetBackgroundColor(fill_color),
                SetForegroundColor(Color::Black),
                Print(ch)
            )?;
        } else {
            queue!(
                out,
                ResetColor,
                SetForegroundColor(fill_color),
                Print(ch)
            )?;
        }
    }

    queue!(out, ResetColor)?;
    out.flush()?;
    Ok(())
}

/// Run a single timed session, redrawing the progress bar every 200 ms.
///
/// Returns `Ok(true)` when the session completes normally, `Ok(false)` when
/// the user interrupts with Ctrl-C.
fn run_session(
    duration: Duration,
    kind: SessionKind,
    session_num: u32,
    total_sessions: u32,
    no_sound: bool,
    interrupted: &Arc<AtomicBool>,
) -> io::Result<bool> {
    let start = Instant::now();
    let tick = Duration::from_millis(200);

    execute!(stdout(), cursor::Hide)?;

    loop {
        if interrupted.load(Ordering::Relaxed) {
            execute!(stdout(), cursor::Show, ResetColor)?;
            println!();
            return Ok(false);
        }

        let elapsed = start.elapsed();
        if elapsed >= duration {
            break;
        }

        let remaining = duration - elapsed;
        let secs = remaining.as_secs();
        let mins = secs / 60;
        let secs = secs % 60;

        let time_str = format!("{:02}:{:02}", mins, secs);
        let label = if total_sessions > 1 {
            format!("{}  {}/{}  {}", kind.label(), session_num, total_sessions, time_str)
        } else {
            format!("{}  {}", kind.label(), time_str)
        };

        let fraction = 1.0 - elapsed.as_secs_f64() / duration.as_secs_f64();
        draw_bar(fraction.clamp(0.0, 1.0), &label, kind)?;

        thread::sleep(tick);
    }

    // Draw the final 0% state
    let label = if total_sessions > 1 {
        format!("{}  {}/{}  00:00", kind.label(), session_num, total_sessions)
    } else {
        format!("{}  00:00", kind.label())
    };
    draw_bar(0.0, &label, kind)?;
    println!();

    execute!(stdout(), cursor::Show)?;

    if !no_sound {
        play_notification();
    }

    Ok(true)
}

/// Play a short audible notification when a session ends.
///
/// Attempts to produce a 440 Hz sine-wave tone via `rodio`.  If no audio
/// device is available the function silently falls back to the terminal bell.
fn play_notification() {
    use rodio::source::{SineWave, Source};
    use rodio::{OutputStream, Sink};

    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let (_stream, handle) = OutputStream::try_default()?;
        let sink = Sink::try_new(&handle)?;

        // Three short ascending beeps
        for freq in [440.0_f32, 550.0, 660.0] {
            let wave = SineWave::new(freq)
                .take_duration(Duration::from_millis(180))
                .amplify(0.40);
            sink.append(wave);
            let silence = rodio::source::Zero::<f32>::new(1, 44_100)
                .take_duration(Duration::from_millis(60));
            sink.append(silence);
        }

        sink.sleep_until_end();
        Ok(())
    })();

    if result.is_err() {
        // Fall back to the terminal bell
        print!("\x07");
        let _ = stdout().flush();
    }
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let work_duration = Duration::from_secs(args.work * 60);
    let short_break_duration = Duration::from_secs(args.short_break * 60);
    let long_break_duration = Duration::from_secs(args.long_break * 60);
    let sessions_per_cycle = args.sessions;

    // Shared flag set by Ctrl-C handler so that run_session can exit cleanly
    // and restore the cursor before the process terminates.
    let interrupted = Arc::new(AtomicBool::new(false));
    {
        let flag = Arc::clone(&interrupted);
        ctrlc::set_handler(move || {
            flag.store(true, Ordering::Relaxed);
        })
        .expect("failed to install Ctrl-C handler");
    }

    let mut work_count: u32 = 0;

    loop {
        work_count += 1;

        let completed = run_session(
            work_duration,
            SessionKind::Work,
            work_count,
            sessions_per_cycle,
            args.no_sound,
            &interrupted,
        )?;
        if !completed {
            break;
        }

        let is_long_break = work_count.is_multiple_of(sessions_per_cycle);

        if is_long_break {
            println!("Long break — great work!");
            let completed = run_session(
                long_break_duration,
                SessionKind::LongBreak,
                work_count / sessions_per_cycle,
                0,
                args.no_sound,
                &interrupted,
            )?;
            if !completed {
                break;
            }
        } else {
            println!("Short break!");
            let completed = run_session(
                short_break_duration,
                SessionKind::ShortBreak,
                work_count,
                sessions_per_cycle,
                args.no_sound,
                &interrupted,
            )?;
            if !completed {
                break;
            }
        }
    }

    Ok(())
}

