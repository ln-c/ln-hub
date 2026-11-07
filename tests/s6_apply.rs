//! S6-apply-wallpaper: `Enter` applies the selected wallpaper as a background job and the status
//! bar shows progress. Also pins `j`/`k` routing to the focused (items) column.
//!
//! The status bar location is not fixed by the spec, so status assertions search every rendered
//! row of an 80x24 buffer for the expected text.
//!
//! Highlight observable (as in S5): the "style" of an items row is the triple (fg, bg, modifier)
//! of the cell holding the FIRST letter of the wallpaper name inside the items column
//! (x in 20..40). Exactly one row is highlighted: its style differs from the others, which all
//! share one style.

mod common;

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use common::*;
use ln_hub::{App, Config, JobState};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::style::{Color, Modifier};

const W: u16 = 80;
const H: u16 = 24;
const ITEMS_X_START: u16 = 20;
const ITEMS_X_END: u16 = 40;
const NAMES: [&str; 3] = ["img1.png", "img2.png", "img3.png"];

type Style = (Color, Color, Modifier);

fn touch(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, b"x").unwrap();
}

/// Wallpaper dir with img1..img3 and no current_wallpaper file; returns (cfg, dir).
fn setup(name: &str) -> (Config, PathBuf) {
    let home = tempdir(&format!("{name}-home"));
    let dir = tempdir(&format!("{name}-wall"));
    for n in NAMES {
        touch(&dir.join(n));
    }
    let mut cfg = Config::default_for(&home);
    cfg.wallpaper_dir = dir.clone();
    assert!(!cfg.current_wallpaper_file.exists());
    (cfg, dir)
}

fn press(app: &mut App, c: char) {
    app.handle_key(key(KeyCode::Char(c)));
}

fn enter(app: &mut App) {
    app.handle_key(key(KeyCode::Enter));
}

/// App with items focused and the cursor on img2.png (`l`, then `j`).
fn app_on_img2(name: &str, sys: &FakeSystem) -> (App, PathBuf) {
    let (cfg, dir) = setup(name);
    let mut app = App::new(cfg, Box::new(sys.clone()));
    press(&mut app, 'l');
    press(&mut app, 'j');
    (app, dir)
}

fn draw(app: &mut App) -> Buffer {
    let mut term = Terminal::new(TestBackend::new(W, H)).unwrap();
    term.draw(|f| app.render(f)).unwrap();
    term.backend().buffer().clone()
}

fn dump(buf: &Buffer) -> String {
    buffer_lines(buf).join("\n")
}

/// Asserts some rendered row contains `text`.
fn assert_status(app: &mut App, text: &str) {
    let buf = draw(app);
    assert!(
        buffer_lines(&buf).iter().any(|l| l.contains(text)),
        "status `{text}` not found anywhere; buffer:\n{}",
        dump(&buf)
    );
}

fn items_hits(buf: &Buffer, text: &str) -> Vec<(u16, u16)> {
    find_text(buf, text)
        .into_iter()
        .filter(|&(x, _)| (ITEMS_X_START..ITEMS_X_END).contains(&x))
        .collect()
}

fn name_style(buf: &Buffer, name: &str) -> Style {
    let (x, y) = *items_hits(buf, name)
        .first()
        .unwrap_or_else(|| panic!("`{name}` not found in items column; buffer:\n{}", dump(buf)));
    let c = &buf[(x, y)];
    (c.fg, c.bg, c.modifier)
}

fn assert_only_highlighted(app: &mut App, expected: &str) {
    let buf = draw(app);
    let styles: Vec<(&str, Style)> = NAMES.iter().map(|n| (*n, name_style(&buf, n))).collect();
    let others: Vec<&(&str, Style)> = styles.iter().filter(|(n, _)| *n != expected).collect();
    let hl = styles.iter().find(|(n, _)| *n == expected).unwrap().1;
    assert!(
        others.iter().all(|(_, s)| *s == others[0].1),
        "non-highlighted rows must share one style, got {styles:?}; buffer:\n{}",
        dump(&buf)
    );
    assert_ne!(
        hl,
        others[0].1,
        "`{expected}` must be the highlighted row, got {styles:?}; buffer:\n{}",
        dump(&buf)
    );
}

#[test]
fn j_k_move_the_items_cursor_and_clamp_when_items_are_focused() {
    // Catches: `j`/`k` still driving the module column when items are focused (cursor stuck on
    // img1), or the items cursor running past the last item / before the first.
    let (cfg, _dir) = setup("s6-nav");
    let mut app = App::new(cfg, Box::new(FakeSystem::new()));
    press(&mut app, 'l');
    assert_only_highlighted(&mut app, "img1.png");
    press(&mut app, 'j');
    assert_only_highlighted(&mut app, "img2.png");
    press(&mut app, 'k');
    assert_only_highlighted(&mut app, "img1.png");
    press(&mut app, 'j');
    press(&mut app, 'j');
    press(&mut app, 'j');
    press(&mut app, 'j');
    assert_only_highlighted(&mut app, "img3.png");
    press(&mut app, 'k');
    press(&mut app, 'k');
    press(&mut app, 'k');
    assert_only_highlighted(&mut app, "img1.png");
}

#[test]
fn enter_spawns_ln_themectl_once_and_shows_applying() {
    // Catches: Enter doing nothing, spawning the wrong program/args/path (e.g. the first item or
    // a relative path), or the status bar missing `Applying img2.png…` (with the single-char `…`).
    let sys = FakeSystem::new();
    let (mut app, dir) = app_on_img2("s6-enter", &sys);
    enter(&mut app);

    assert_eq!(
        sys.spawn_calls(),
        vec![(
            "ln-themectl".to_string(),
            vec!["-w".to_string(), format!("{}/img2.png", dir.display())]
        )]
    );
    assert_status(&mut app, "Applying img2.png…");
}

#[test]
fn second_enter_while_running_does_not_spawn_again() {
    // Catches: no in-flight guard, so a second Enter starts a duplicate ln-themectl job.
    let sys = FakeSystem::new();
    sys.set_job_state(JobState::Running);
    let (mut app, _dir) = app_on_img2("s6-twice", &sys);
    enter(&mut app);
    app.tick();
    enter(&mut app);

    assert_eq!(sys.spawn_calls().len(), 1, "calls: {:?}", sys.spawn_calls());
    assert_status(&mut app, "Applying img2.png…");
}

#[test]
fn enter_after_job_finished_spawns_again() {
    // Catches: the in-flight guard never being released once the job is Done (Enter dead forever).
    let sys = FakeSystem::new();
    sys.set_job_state(JobState::Running);
    let (mut app, _dir) = app_on_img2("s6-again", &sys);
    enter(&mut app);
    sys.set_job_state(JobState::Done);
    app.tick();
    enter(&mut app);

    assert_eq!(sys.spawn_calls().len(), 2, "calls: {:?}", sys.spawn_calls());
}

#[test]
fn done_job_shows_applied_after_tick() {
    // Catches: tick not polling the job, or the status stuck on `Applying` / not naming the file.
    let sys = FakeSystem::new();
    sys.set_job_state(JobState::Running);
    let (mut app, _dir) = app_on_img2("s6-done", &sys);
    enter(&mut app);
    app.tick();
    assert_status(&mut app, "Applying img2.png…");

    sys.set_job_state(JobState::Done);
    app.tick();
    assert_status(&mut app, "Applied img2.png");
}

#[test]
fn failed_job_shows_apply_failed_with_reason_after_tick() {
    // Catches: a failed job reported as success, or the failure reason (`exit 1`) dropped.
    let sys = FakeSystem::new();
    sys.set_job_state(JobState::Running);
    let (mut app, _dir) = app_on_img2("s6-failed", &sys);
    enter(&mut app);

    sys.set_job_state(JobState::Failed("exit 1".to_string()));
    app.tick();
    assert_status(&mut app, "Apply failed: exit 1");
}

#[test]
fn spawn_error_shows_not_found_status_without_panicking() {
    // Catches: unwrapping the spawn result (panic) or swallowing the io::Error so the status
    // bar never says `ln-themectl not found: not found`.
    let sys = FakeSystem::new();
    sys.set_spawn_error(ErrorKind::NotFound, "not found");
    let (mut app, _dir) = app_on_img2("s6-notfound", &sys);
    enter(&mut app);

    assert_status(&mut app, "ln-themectl not found: not found");
}
