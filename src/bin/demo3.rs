//! Driftscape — terminal screensaver suite (doc03.03).
//!
//! This binary is the impure shell around the pure `driftscape` modules: it
//! puts the terminal into raw mode on the alternate screen, sizes a half-block
//! canvas to the window, and runs the frame loop — advancing the active scene
//! by real elapsed time and painting it ~30 times a second. The scenes know
//! nothing about the terminal; this file knows nothing about the drift math.
//!
//! Run with `cargo run --bin demo3`. Quit with `q`, `Esc`, or `Ctrl-C`.

use std::io::{stdout, Stdout, Write};
use std::time::{Duration, Instant};

use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, size, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{execute, QueueableCommand};

use mesopotamia::driftscape::canvas::Canvas;
use mesopotamia::driftscape::scene::Scene;
use mesopotamia::driftscape::starliner::Starliner;

const FRAME: Duration = Duration::from_millis(33);

/// Restores the terminal on the way out — including on a panic or early `?`,
/// so a crash never leaves the user staring at a frozen alternate screen.
struct TermGuard;

impl Drop for TermGuard {
    fn drop(&mut self) {
        let mut out = stdout();
        let _ = execute!(out, Show, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

fn quit_requested(ev: &Event) -> bool {
    if let Event::Key(k) = ev {
        match k.code {
            KeyCode::Char('q') | KeyCode::Esc => return true,
            KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => return true,
            _ => {}
        }
    }
    false
}

fn main() -> std::io::Result<()> {
    let seed: u64 = rand::random();

    enable_raw_mode()?;
    let mut out: Stdout = stdout();
    execute!(out, EnterAlternateScreen, Hide)?;
    let _guard = TermGuard;

    let (mut cols, mut rows) = size()?;
    let mut scene = Starliner::new(cols as usize, rows as usize, seed);
    let mut canvas = Canvas::new(cols as usize, rows as usize);
    let mut frame = String::new();
    let mut last = Instant::now();

    loop {
        // Rebuild on resize so the field always fills the window.
        let (nc, nr) = size()?;
        if (nc, nr) != (cols, rows) {
            cols = nc;
            rows = nr;
            canvas = Canvas::new(cols as usize, rows as usize);
            scene = Starliner::new(cols as usize, rows as usize, seed);
        }

        let now = Instant::now();
        let dt = (now - last).as_secs_f32().min(0.1); // clamp so a stall doesn't teleport everything
        last = now;

        scene.step(dt);
        scene.draw(&mut canvas);

        frame.clear();
        canvas.render(&mut frame);
        out.queue(crossterm::cursor::MoveTo(0, 0))?;
        out.write_all(frame.as_bytes())?;
        out.flush()?;

        // Pace to the frame budget while staying responsive to quit keys.
        let spent = now.elapsed();
        if spent < FRAME && event::poll(FRAME - spent)? && quit_requested(&event::read()?) {
            break;
        } else if event::poll(Duration::ZERO)? && quit_requested(&event::read()?) {
            break;
        }
    }

    Ok(())
}
