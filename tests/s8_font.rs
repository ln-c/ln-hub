//! S8-font: the Theme view shows the current font name and size (parsed from `cfg.hypr_env_file`)
//! and `f` / `s` open a text prompt that sets them through `ln-themectl -f` / `-s`.
//!
//! Where the font and the status are drawn is not fixed by the spec, so assertions search every
//! rendered row of an 80x24 buffer. The env file is a real file in a temp dir; only `System` is faked.

mod common;

use std::fs;
use std::path::PathBuf;

use common::*;
use ln_hub::{App, Config, JobState};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;

const DEFAULT_FONT: &str = "Terminess Nerd Font Propo";
const SIZE_ERR: &str = "Font size must be 6-72";

type Call = (String, Vec<String>);

fn call(args: &[&str]) -> Call {
    (
        "ln-themectl".to_string(),
        args.iter().map(|s| s.to_string()).collect(),
    )
}

/// Wallpaper dir with img1.png / img2.png (no digit pair that could read as a font size).
/// `env_lua`: contents of the env file, or None to leave it missing. Items column focused.
fn app_with_env(name: &str, env_lua: Option<&str>, sys: &FakeSystem) -> (App, PathBuf) {
    let home = tempdir(&format!("{name}-home"));
    let dir = tempdir(&format!("{name}-wall"));
    for n in ["img1.png", "img2.png"] {
        fs::write(dir.join(n), b"x").unwrap();
    }
    let env_file = tempdir(&format!("{name}-hypr")).join("env.lua");
    if let Some(text) = env_lua {
        fs::write(&env_file, text).unwrap();
    }
    let mut cfg = Config::default_for(&home);
    cfg.wallpaper_dir = dir.clone();
    cfg.hypr_env_file = env_file;
    let mut app = App::new(cfg, Box::new(sys.clone()));
    press(&mut app, 'l');
    (app, dir)
}

/// Like `app_with_env` but also returns the env file path (so a test can rewrite it) and can leave
/// the module column focused (`focus_items == false`: `l` is not pressed).
fn app_with_env_path(
    name: &str,
    env_lua: &str,
    focus_items: bool,
    sys: &FakeSystem,
) -> (App, PathBuf, PathBuf) {
    let home = tempdir(&format!("{name}-home"));
    let dir = tempdir(&format!("{name}-wall"));
    for n in ["img1.png", "img2.png"] {
        fs::write(dir.join(n), b"x").unwrap();
    }
    let env_file = tempdir(&format!("{name}-hypr")).join("env.lua");
    fs::write(&env_file, env_lua).unwrap();
    let mut cfg = Config::default_for(&home);
    cfg.wallpaper_dir = dir.clone();
    cfg.hypr_env_file = env_file.clone();
    let mut app = App::new(cfg, Box::new(sys.clone()));
    if focus_items {
        press(&mut app, 'l');
    }
    (app, dir, env_file)
}

const OLD_ENV: &str = "hl.env(\"FONT_NAME\", \"Old Mono\")\nhl.env(\"FONT_SIZE\", \"13\")\n";
const NEW_ENV: &str = "hl.env(\"FONT_NAME\", \"Fira Code\")\nhl.env(\"FONT_SIZE\", \"21\")\n";

fn app(name: &str, sys: &FakeSystem) -> (App, PathBuf) {
    app_with_env(name, None, sys)
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        press(app, c);
    }
}

fn shows(app: &mut App, text: &str) -> bool {
    buffer_lines(&draw(app)).iter().any(|l| l.contains(text))
}

fn assert_shows(app: &mut App, text: &str) {
    assert_status(&draw(app), text);
}

fn assert_lacks(app: &mut App, text: &str) {
    let buf = draw(app);
    assert!(
        !buffer_lines(&buf).iter().any(|l| l.contains(text)),
        "`{text}` must not be shown; buffer:\n{}",
        dump(&buf)
    );
}

/// True if `token` appears in some row not adjacent to another digit (so `16` != `116`, `160`).
fn has_number(buf: &Buffer, token: &str) -> bool {
    buffer_lines(buf).iter().any(|l| {
        let b = l.as_bytes();
        l.match_indices(token).any(|(i, _)| {
            let before = i == 0 || !b[i - 1].is_ascii_digit();
            let after = i + token.len() >= b.len() || !b[i + token.len()].is_ascii_digit();
            before && after
        })
    })
}

fn assert_number(app: &mut App, token: &str) {
    let buf = draw(app);
    assert!(
        has_number(&buf, token),
        "font size `{token}` not shown as a number; buffer:\n{}",
        dump(&buf)
    );
}

fn assert_no_number(app: &mut App, token: &str) {
    let buf = draw(app);
    assert!(
        !has_number(&buf, token),
        "`{token}` must not be shown as a number; buffer:\n{}",
        dump(&buf)
    );
}

#[test]
fn env_lua_font_name_and_size_are_shown() {
    // Catches: no font display at all, or FONT_NAME / FONT_SIZE not parsed from `hl.env(...)` lines
    // (decoy vars sit around them; the size line precedes the name line).
    let sys = FakeSystem::new();
    let env = "hl.env(\"XCURSOR_SIZE\", \"24\")\n\
               hl.env(\"FONT_SIZE\", \"16\")\n\
               hl.env(\"FONT_NAME\", \"Terminess Nerd Font Propo\")\n\
               hl.env(\"TERMINAL\", \"kitty\")\n";
    let (mut a, _dir) = app_with_env("s8-env", Some(env), &sys);
    assert_shows(&mut a, DEFAULT_FONT);
    assert_number(&mut a, "16");
    assert_no_number(&mut a, "24");
}

#[test]
fn non_default_env_lua_values_are_shown_instead_of_defaults() {
    // Catches: always rendering the hard-coded defaults instead of what env.lua says.
    let sys = FakeSystem::new();
    let env = "hl.env(\"FONT_NAME\", \"JetBrainsMono Nerd Font\")\n\
               hl.env(\"FONT_SIZE\", \"13\")\n";
    let (mut a, _dir) = app_with_env("s8-nondefault", Some(env), &sys);
    assert_shows(&mut a, "JetBrainsMono Nerd Font");
    assert_number(&mut a, "13");
    assert_lacks(&mut a, DEFAULT_FONT);
    assert_no_number(&mut a, "16");
}

#[test]
fn missing_env_lua_shows_default_font_and_size() {
    // Catches: no defaults when env.lua is absent (blank display), or a panic on the missing file.
    let sys = FakeSystem::new();
    let (mut a, _dir) = app("s8-missing", &sys);
    assert_shows(&mut a, DEFAULT_FONT);
    assert_number(&mut a, "16");
}

#[test]
fn f_then_typed_name_then_enter_spawns_font_name_change() {
    // Catches: `f` not opening a prompt, typed chars (incl. space and letters like `s`/`n`) leaking
    // into other keys, or the spawn using the wrong flag/args.
    let sys = FakeSystem::new();
    let (mut a, _dir) = app("s8-name", &sys);
    press(&mut a, 'f');
    type_text(&mut a, "JetBrainsMono Nerd Font");
    assert!(sys.spawn_calls().is_empty(), "no spawn before Enter");
    enter(&mut a);
    assert_eq!(
        sys.spawn_calls(),
        vec![call(&["-f", "JetBrainsMono Nerd Font"])]
    );
}

#[test]
fn s_then_18_then_enter_spawns_font_size_change() {
    // Catches: `s` not opening a prompt, or spawning with the wrong flag (`-f`) / wrong value.
    let sys = FakeSystem::new();
    let (mut a, _dir) = app("s8-size", &sys);
    press(&mut a, 's');
    type_text(&mut a, "18");
    enter(&mut a);
    assert_eq!(sys.spawn_calls(), vec![call(&["-s", "18"])]);
}

#[test]
fn sizes_at_the_bounds_6_and_72_are_accepted() {
    // Catches: off-by-one range check (`6 < n` or `n < 72`) rejecting the inclusive bounds.
    for size in ["6", "72"] {
        let sys = FakeSystem::new();
        let (mut a, _dir) = app(&format!("s8-bound-{size}"), &sys);
        press(&mut a, 's');
        type_text(&mut a, size);
        enter(&mut a);
        assert_eq!(sys.spawn_calls(), vec![call(&["-s", size])], "size {size}");
        assert_lacks(&mut a, SIZE_ERR);
    }
}

#[test]
fn invalid_sizes_do_not_spawn_and_show_the_range_error() {
    // Catches: spawning ln-themectl with a non-numeric or out-of-range size (`abc`, `5`, `73`), or
    // failing silently without the `Font size must be 6-72` status.
    for bad in ["abc", "5", "73"] {
        let sys = FakeSystem::new();
        let (mut a, _dir) = app(&format!("s8-bad-{bad}"), &sys);
        press(&mut a, 's');
        type_text(&mut a, bad);
        enter(&mut a);
        assert!(
            sys.spawn_calls().is_empty(),
            "`{bad}` must not spawn: {:?}",
            sys.spawn_calls()
        );
        assert_shows(&mut a, SIZE_ERR);
    }
}

#[test]
fn esc_closes_the_prompt_without_spawning() {
    // Catches: Esc leaving the prompt open (typed text still shown, Enter later submits `-f Zed`,
    // `j` swallowed by the prompt) or Esc submitting the text.
    let sys = FakeSystem::new();
    let (mut a, dir) = app("s8-esc", &sys);
    press(&mut a, 'f');
    type_text(&mut a, "Zed");
    assert_shows(&mut a, "Zed"); // the prompt is open and echoes what was typed
    a.handle_key(key(KeyCode::Esc));
    assert!(sys.spawn_calls().is_empty(), "Esc must not spawn");
    assert_lacks(&mut a, "Zed");

    // Items are focused again: `j` moves the cursor and Enter applies img2, not a font.
    press(&mut a, 'j');
    enter(&mut a);
    assert_eq!(
        sys.spawn_calls(),
        vec![call(&["-w", &format!("{}/img2.png", dir.display())])]
    );
    assert!(!shows(&mut a, SIZE_ERR));
}

#[test]
fn done_font_job_reloads_the_display_from_env_file() {
    // Catches: tick not re-reading env.lua after a Done font job (display stays on the old font).
    // The typed name (`Zed`) differs from the env values so the status text cannot fake a pass.
    let sys = FakeSystem::new();
    let (mut a, _dir, env) = app_with_env_path("s8-reload", OLD_ENV, true, &sys);
    assert_shows(&mut a, "Old Mono");
    sys.set_job_state(JobState::Running);
    press(&mut a, 'f');
    type_text(&mut a, "Zed");
    enter(&mut a);
    fs::write(&env, NEW_ENV).unwrap();
    sys.set_job_state(JobState::Done);
    a.tick();
    assert_shows(&mut a, "Fira Code");
    assert_number(&mut a, "21");
    assert_lacks(&mut a, "Old Mono");
    assert_no_number(&mut a, "13");
}

#[test]
fn failed_font_job_keeps_the_old_display() {
    // Catches: tick re-reading env.lua after a Failed job (shows a font that was not applied).
    let sys = FakeSystem::new();
    let (mut a, _dir, env) = app_with_env_path("s8-failed", OLD_ENV, true, &sys);
    sys.set_job_state(JobState::Running);
    press(&mut a, 'f');
    type_text(&mut a, "Zed");
    enter(&mut a);
    fs::write(&env, NEW_ENV).unwrap();
    sys.set_job_state(JobState::Failed("exit 1".to_string()));
    a.tick();
    assert_shows(&mut a, "Old Mono");
    assert_number(&mut a, "13");
    assert_lacks(&mut a, "Fira Code");
    assert_no_number(&mut a, "21");
}

#[test]
fn quit_key_typed_into_a_prompt_is_text_not_quit() {
    // Catches: the quit arm running before the prompt check (typing `q` in a font name quits).
    for text in ["q", "aq"] {
        let sys = FakeSystem::new();
        let (mut a, _dir) = app(&format!("s8-quit-{text}"), &sys);
        press(&mut a, 'f');
        type_text(&mut a, text);
        assert!(!a.should_quit(), "`{text}` typed in a prompt must not quit");
        enter(&mut a);
        assert_eq!(sys.spawn_calls(), vec![call(&["-f", text])]);
        assert!(!a.should_quit());
    }
}

#[test]
fn f_and_s_do_nothing_while_the_module_column_is_focused() {
    // Catches: `f` / `s` opening a prompt without the items column focused (guard dropped).
    let sys = FakeSystem::new();
    let (mut a, _dir, _env) = app_with_env_path("s8-unfocused", OLD_ENV, false, &sys);
    press(&mut a, 'f');
    assert_lacks(&mut a, "Font name:");
    type_text(&mut a, "Zed");
    enter(&mut a);
    press(&mut a, 's');
    assert_lacks(&mut a, "Font size:");
    type_text(&mut a, "18");
    enter(&mut a);
    assert!(
        sys.spawn_calls().is_empty(),
        "no font spawn expected: {:?}",
        sys.spawn_calls()
    );
    assert_lacks(&mut a, "Zed");
}

#[test]
fn prompt_submit_waits_while_a_job_is_in_flight() {
    // Catches: submit_prompt spawning a second ln-themectl while one is running (no in-flight
    // guard), or dropping the typed text instead of keeping the prompt open.
    let sys = FakeSystem::new();
    let (mut a, dir) = app("s8-inflight", &sys);
    sys.set_job_state(JobState::Running);
    enter(&mut a); // applies img1.png
    let wall = call(&["-w", &format!("{}/img1.png", dir.display())]);
    assert_eq!(sys.spawn_calls(), vec![wall.clone()]);
    a.tick();
    press(&mut a, 'f');
    type_text(&mut a, "Zed");
    enter(&mut a);
    assert_eq!(sys.spawn_calls(), vec![wall.clone()], "no second spawn");
    assert_shows(&mut a, "Zed");
    sys.set_job_state(JobState::Done);
    a.tick();
    enter(&mut a);
    assert_eq!(sys.spawn_calls(), vec![wall, call(&["-f", "Zed"])]);
}

#[test]
fn backspace_removes_the_last_typed_character() {
    // Catches: Backspace ignored in a prompt (`189` would be rejected as out of range).
    let sys = FakeSystem::new();
    let (mut a, _dir) = app("s8-bksp", &sys);
    press(&mut a, 's');
    type_text(&mut a, "189");
    a.handle_key(key(KeyCode::Backspace));
    enter(&mut a);
    assert_eq!(sys.spawn_calls(), vec![call(&["-s", "18"])]);
}

#[test]
fn empty_font_name_does_not_spawn() {
    // Catches: spawning `ln-themectl -f ""` for an empty or whitespace-only name.
    for text in ["", "   "] {
        let sys = FakeSystem::new();
        let (mut a, _dir) = app("s8-empty", &sys);
        press(&mut a, 'f');
        type_text(&mut a, text);
        enter(&mut a);
        assert!(
            sys.spawn_calls().is_empty(),
            "`{text}` must not spawn: {:?}",
            sys.spawn_calls()
        );
    }
}

#[test]
fn font_name_is_trimmed_before_spawning() {
    // Catches: passing the raw prompt text (leading/trailing spaces) as the font name.
    let sys = FakeSystem::new();
    let (mut a, _dir) = app("s8-trim-name", &sys);
    press(&mut a, 'f');
    type_text(&mut a, "  Zed  ");
    enter(&mut a);
    assert_eq!(sys.spawn_calls(), vec![call(&["-f", "Zed"])]);
}

#[test]
fn font_size_is_trimmed_before_parsing() {
    // Catches: parsing the untrimmed size text (` 18 ` rejected as non-numeric).
    let sys = FakeSystem::new();
    let (mut a, _dir) = app("s8-trim-size", &sys);
    press(&mut a, 's');
    type_text(&mut a, " 18 ");
    enter(&mut a);
    assert_eq!(sys.spawn_calls(), vec![call(&["-s", "18"])]);
}

#[test]
fn prompts_are_labelled() {
    // Catches: a prompt that echoes only the typed text, with no `Font name:` / `Font size:` label.
    let sys = FakeSystem::new();
    let (mut a, _dir) = app("s8-labels", &sys);
    press(&mut a, 'f');
    assert_shows(&mut a, "Font name:");
    a.handle_key(key(KeyCode::Esc));
    press(&mut a, 's');
    assert_shows(&mut a, "Font size:");
    assert_lacks(&mut a, "Font name:");
}

#[test]
fn font_job_statuses_show_applying_then_applied() {
    // Catches: no status feedback for font jobs (or the wrong label/ellipsis) while applying and
    // after the job finishes.
    let sys = FakeSystem::new();
    let (mut a, _dir) = app("s8-status", &sys);
    sys.set_job_state(JobState::Running);
    press(&mut a, 's');
    type_text(&mut a, "18");
    enter(&mut a);
    assert_shows(&mut a, "Applying font size 18\u{2026}");
    sys.set_job_state(JobState::Done);
    a.tick();
    assert_shows(&mut a, "Applied font size 18");
    assert_lacks(&mut a, "Applying");

    sys.set_job_state(JobState::Running);
    press(&mut a, 'f');
    type_text(&mut a, "Zed");
    enter(&mut a);
    assert_shows(&mut a, "Applying font name Zed\u{2026}");
    sys.set_job_state(JobState::Done);
    a.tick();
    assert_shows(&mut a, "Applied font name Zed");
    assert_lacks(&mut a, "Applying");
}

#[test]
fn font_line_is_hidden_while_the_module_column_is_focused() {
    // Catches: drawing the font line regardless of focus (it belongs to the Theme items view).
    let sys = FakeSystem::new();
    let (mut a, _dir, _env) = app_with_env_path("s8-hidden", OLD_ENV, false, &sys);
    assert_lacks(&mut a, "Old Mono");
    assert_no_number(&mut a, "13");
    press(&mut a, 'l');
    assert_shows(&mut a, "Old Mono");
}
