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

const W: u16 = 80;
const H: u16 = 24;
const ITEMS_X_START: u16 = 20;
const ITEMS_X_END: u16 = 40;
const NAMES: [&str; 3] = ["img1.png", "img2.png", "img3.png"];

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

/// Asserts some rendered row contains `text`.
fn assert_status(app: &mut App, text: &str) {
    common::assert_status(&draw(app), text);
}

fn assert_only_highlighted(app: &mut App, expected: &str) {
    let buf = draw(app);
    common::assert_only_highlighted(&buf, &NAMES, expected, |b, n| {
        items_hits(b, n, ITEMS_X_START, ITEMS_X_END)
            .first()
            .copied()
    });
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

/// App whose current_wallpaper file names img1 at construction; items focused, cursor on img2.
/// Returns (app, dir, current-file path).
fn app_current_img1_cursor_img2(name: &str, sys: &FakeSystem) -> (App, PathBuf, PathBuf) {
    let (mut cfg, dir) = setup(name);
    let cur = tempdir(&format!("{name}-cache")).join("current_wallpaper");
    fs::write(&cur, format!("{}/img1.png\n", dir.display())).unwrap();
    cfg.current_wallpaper_file = cur.clone();
    let mut app = App::new(cfg, Box::new(sys.clone()));
    press(&mut app, 'l');
    press(&mut app, 'j');
    (app, dir, cur)
}

/// y rows of every `●` in the items column.
fn mark_rows(buf: &Buffer) -> Vec<u16> {
    items_hits(buf, "●", ITEMS_X_START, ITEMS_X_END)
        .into_iter()
        .map(|(_, y)| y)
        .collect()
}

#[test]
fn done_job_moves_current_mark_to_wallpaper_in_file_and_does_not_move_cursor() {
    // Catches: the app never re-reading current_wallpaper after a Done job (the `●` stays on img1),
    // or the reload re-running preselect and yanking the cursor onto the new current (img3) when
    // it was on img2. The file deliberately names img3, not the applied img2, so the cursor
    // staying on img2 cannot be explained by a preselect landing there.
    let sys = FakeSystem::new();
    sys.set_job_state(JobState::Running);
    let (mut app, dir, cur) = app_current_img1_cursor_img2("s6-reload", &sys);
    let buf = draw(&mut app);
    assert_eq!(
        mark_rows(&buf),
        vec![row_of(&buf, "img1.png", ITEMS_X_START, ITEMS_X_END)],
        "{}",
        dump(&buf)
    );

    enter(&mut app);
    app.tick();
    // Simulate ln-themectl recording a different wallpaper (img3) as current.
    fs::write(&cur, format!("{}/img3.png\n", dir.display())).unwrap();
    sys.set_job_state(JobState::Done);
    app.tick();

    let buf = draw(&mut app);
    assert_eq!(
        mark_rows(&buf),
        vec![row_of(&buf, "img3.png", ITEMS_X_START, ITEMS_X_END)],
        "exactly one `●`, on the img3.png row (none on img1/img2); buffer:\n{}",
        dump(&buf)
    );
    assert_only_highlighted(&mut app, "img2.png");
}

#[test]
fn failed_job_does_not_reread_current_wallpaper() {
    // Catches: re-reading current_wallpaper on Failed (the `●` must stay on the old wallpaper
    // because nothing was applied). Pins a guard; passes before the Done-reload is implemented.
    let sys = FakeSystem::new();
    sys.set_job_state(JobState::Running);
    let (mut app, dir, cur) = app_current_img1_cursor_img2("s6-noreload", &sys);

    enter(&mut app);
    app.tick();
    fs::write(&cur, format!("{}/img2.png\n", dir.display())).unwrap();
    sys.set_job_state(JobState::Failed("exit 1".to_string()));
    app.tick();

    let buf = draw(&mut app);
    assert_eq!(
        mark_rows(&buf),
        vec![row_of(&buf, "img1.png", ITEMS_X_START, ITEMS_X_END)],
        "`●` must stay on img1.png after a failed job; buffer:\n{}",
        dump(&buf)
    );
    assert_only_highlighted(&mut app, "img2.png");
}
