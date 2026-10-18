use crate::command::{self, Command, Mods};
use crate::effects;
use crate::platform::menu;
use crate::protocol;
use crate::state::{AppState, Mode, ScrollAction};
use crate::transforms::ShortcutKey;
use crate::transitions::TransitionRegistry;
use crate::viewer;
use crate::wake::{self, Note};
use dioxus::desktop::tao::event::{Event, WindowEvent};
use dioxus::desktop::{use_muda_event_handler, use_window, use_wry_event_handler};
use dioxus::prelude::*;
use std::time::{Duration, Instant};

const CSS: &str = r#"
html, body { margin: 0; height: 100%; overflow: hidden; overscroll-behavior: none; background: #111; -webkit-user-select: none; user-select: none; }
#root { height: 100vh; overflow: hidden; overscroll-behavior: none; outline: none; color-scheme: light dark; background: light-dark(#c4c4c2, #161616); color: light-dark(#111, #eee); font: 13px -apple-system, BlinkMacSystemFont, sans-serif; position: relative; -webkit-user-select: none; user-select: none; }
img, button, .label, .stage { -webkit-user-select: none; user-select: none; -webkit-user-drag: none; }
.dialog, .dialog * { -webkit-user-select: text; user-select: text; }
.browser { display: grid; grid-template-columns: repeat(8, minmax(0, 1fr)); align-content: start; overflow: auto; height: 100%; }
.cell { display: flex; flex-direction: column; gap: 4px; margin: 0; padding: 8px; border: 0; background: transparent; color: inherit; text-align: center; min-width: 0; }
.cell.selected { background: rgba(30, 144, 255, 0.5); }
.thumb-box { width: 100%; aspect-ratio: 1; position: relative; overflow: hidden; }
.thumb { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: contain; }
.thumb.loading { width: 40%; height: 40%; margin: auto; opacity: 0.2; }
.video-badge { position: absolute; z-index: 1; left: 6px; bottom: 6px; width: 20px; height: 20px; border-radius: 50%; background: rgba(20, 20, 20, 0.72); box-shadow: 0 1px 2px rgba(0, 0, 0, 0.45); pointer-events: none; }
.video-badge::after { content: ""; position: absolute; left: 7px; top: 5px; border-style: solid; border-width: 5px 0 5px 8px; border-color: transparent transparent transparent white; }
.thumb-error { font-size: 11px; padding: 4px; }
.label { white-space: nowrap; overflow: hidden; }
.stage { position: relative; width: 100%; height: 100%; overflow: hidden; overscroll-behavior: none; background: light-dark(#aeaeac, #101010); touch-action: none; }
.frame, video { -webkit-user-select: none; user-select: none; -webkit-user-drag: none; pointer-events: none; will-change: transform; background: transparent; }
.selection { position: absolute; z-index: 4; box-sizing: border-box; background: rgba(30, 144, 255, 0.5); border: 1px solid rgb(30, 144, 255); pointer-events: none; }
.scrubber { position: absolute; z-index: 6; left: 84px; right: 84px; bottom: 90px; padding: 12px 16px 10px; border-radius: 12px; background: rgba(0, 0, 0, 0.38); box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.14), 0 6px 18px rgba(0, 0, 0, 0.18); pointer-events: none; overflow: visible; opacity: 0; transition: opacity 240ms ease; }
.scrubber.shown { opacity: 1; }
@starting-style { .scrubber.shown { opacity: 0; } }
.scrubber .track { position: relative; height: 4px; border-radius: 999px; background: rgba(255, 255, 255, 0.18); box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.35); }
.scrubber-fill { position: absolute; left: 0; top: 0; bottom: 0; border-radius: 999px; background: rgb(30, 144, 255); }
.scrubber .mark { position: absolute; top: 50%; width: 12px; height: 12px; margin-top: -6px; border-radius: 50%; background: rgba(16, 16, 16, 0.55); box-shadow: 0 0 0 1.5px rgba(255, 255, 255, 0.75), 0 1px 2px rgba(0, 0, 0, 0.4); }
.scrubber .loop-in, .scrubber .loop-out { position: absolute; top: -8px; width: 8px; height: 12px; box-sizing: border-box; border: 2px solid rgb(255, 204, 0); filter: drop-shadow(0 1px 1px rgba(0, 0, 0, 0.65)); pointer-events: none; }
.scrubber .loop-in { border-right: 0; margin-left: -2px; }
.scrubber .loop-out { border-left: 0; margin-left: -6px; }
.scrubber .loop-span { position: absolute; top: -8px; height: 2px; background: rgb(255, 204, 0); pointer-events: none; }
.dialog-back { position: absolute; z-index: 8; inset: 0; background: rgba(0, 0, 0, 0.35); display: flex; align-items: center; justify-content: center; }
.dialog { background: light-dark(#fff, #2c2c2c); color: inherit; padding: 16px 18px; min-width: 280px; border-radius: 8px; }
.dialog h2 { margin: 0 0 12px; font-size: 16px; }
.info-row, .slider { display: flex; justify-content: space-between; gap: 16px; margin: 6px 0; }
.slider input { width: 160px; }
.dialog-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 14px; }
.message { position: absolute; z-index: 7; left: 12px; right: 12px; top: 12px; background: rgba(0, 0, 0, 0.78); color: white; padding: 8px 10px; border-radius: 6px; }
.place { position: absolute; z-index: 7; right: 16px; bottom: 16px; padding: 4px 8px; border-radius: 6px; background: rgba(0, 0, 0, 0.55); color: white; font-variant-numeric: tabular-nums; pointer-events: none; opacity: 0; transition: opacity 240ms ease; }
.place.shown { opacity: 1; }
@starting-style { .place.shown { opacity: 0; } }
"#;

pub fn app() -> Element {
    let mut state = use_signal(AppState::new);
    let window = use_window();
    use_hook(|| {
        #[cfg(target_os = "macos")]
        crate::platform::swipe::install();
        wake::bind_runtime();
        let mut notes = wake::install();
        let window = window.clone();
        spawn(async move {
            while let Some(note) = notes.recv().await {
                deliver(state, window.clone(), note);
            }
        });
        spawn(async move {
            loop {
                let due = state.read().place_is_due(Instant::now());
                if due {
                    state.write().hide_place_if_due(Instant::now());
                }
                let (token, delay) = {
                    let current = state.read();
                    (current.anim_token, current.animation_delay())
                };
                match delay {
                    Some(delay) => {
                        tokio::time::sleep(delay).await;
                        state.write().advance_animation(token);
                    }
                    None => tokio::time::sleep(Duration::from_millis(40)).await,
                }
            }
        });
        viewer::bind_player(state);
    });

    use_wry_event_handler({
        let window = window.clone();
        move |event, _| {
            if let Event::WindowEvent { event, .. } = event {
                match event {
                    WindowEvent::Resized(size) => {
                        let scale = window.scale_factor().max(1.0);
                        state.write().set_view_size(
                            (size.width as f64 / scale, size.height as f64 / scale),
                            scale,
                        );
                        restore_keyboard(&window, &mut state);
                    }
                    WindowEvent::Focused(true) => restore_keyboard(&window, &mut state),
                    _ => {}
                }
            }
        }
    });

    use_muda_event_handler({
        let window = window.clone();
        move |event| {
            if let Some(command) = menu::command_for(&event.id.0) {
                let effect = state.write().dispatch(command, Instant::now());
                effects::run(state, window.clone(), effect);
            }
        }
    });

    let mut synced = use_signal(|| 0_u64);
    let mut seen_shell = use_signal(|| None::<crate::state::Shell>);
    use_effect({
        let window = window.clone();
        move || {
            let pending =
                state.read().pending_restore.is_some() || state.read().scroll_action.is_some();
            let shell = state.read().shell();
            if !pending && seen_shell() == Some(shell) {
                return;
            }
            if state.read().pending_restore.is_some() {
                state.write().commit_restore();
            }
            window.set_title(&state.read().title());
            window.set_fullscreen(state.read().fullscreen);
            menu::sync(&state.read().availability());
            crate::browser::schedule(&state.read());
            let epoch = state.read().video_epoch;
            if epoch != synced() {
                synced.set(epoch);
                let precise = state.read().precise_seek;
                let video = state.read().video().cloned();
                if let Some(video) = video {
                    viewer::sync_video(&video, precise);
                    if precise {
                        state.write().precise_seek = false;
                    }
                }
            }
            if state.read().scroll_action.is_some() {
                if let Some(action) = state.write().scroll_action.take() {
                    match action {
                        ScrollAction::Restore(top) => {
                            dioxus::document::eval(&format!(
                                "const grid = document.getElementById('browser'); if (grid) grid.scrollTop = {top};"
                            ));
                        }
                        ScrollAction::Reveal => {
                            dioxus::document::eval(
                                "const cell = document.querySelector('.cell.selected'); if (cell) cell.scrollIntoView({block:'nearest', inline:'nearest'});",
                            );
                        }
                    }
                }
            }
            seen_shell.set(Some(state.read().shell()));
        }
    });

    protocol::publish(&state.read());
    let mode = state.read().mode;
    let message = state.read().message.clone();
    let long_operation = state.read().long_operation();
    let window_keys = window.clone();
    rsx! {
        style { "{CSS}" }
        div {
            id: "root",
            tabindex: "0",
            onmounted: move |_| {
                dioxus::document::eval(
                    "(() => { document.getElementById('root')?.focus(); if (document.documentElement.dataset.noselect) return; document.documentElement.dataset.noselect = '1'; const block = (event) => { if (event.target && event.target.closest && event.target.closest('.dialog')) return; event.preventDefault(); }; document.addEventListener('selectstart', block); document.addEventListener('dragstart', block); document.addEventListener('mouseup', () => window.getSelection()?.removeAllRanges()); })()",
                );
            },
            onkeydown: move |event| on_key(state, window_keys.clone(), event),
            if mode == Mode::Browser {
                {crate::browser::view(state, window.clone())}
            } else {
                {viewer::view(state, window.clone())}
            }
            {crate::dialogs::view(state, window.clone())}
            if let Some(message) = message {
                if long_operation {
                    div { class: "message", "{message}" }
                } else {
                    button {
                        class: "message",
                        onclick: move |_| state.write().dismiss_message(),
                        "{message}"
                    }
                }
            }
        }
    }
}

fn restore_keyboard(window: &dioxus::desktop::DesktopContext, state: &mut Signal<AppState>) {
    let Some(until) = state.read().keyboard_until else {
        return;
    };
    if Instant::now() < until {
        effects::focus_page(window);
    } else {
        state.write().keyboard_until = None;
    }
}

fn deliver(mut state: Signal<AppState>, window: dioxus::desktop::DesktopContext, note: Note) {
    let effect = state
        .write()
        .handle_note(note, &TransitionRegistry::builtin());
    effects::run(state, window, effect);
}

fn on_key(
    mut state: Signal<AppState>,
    window: dioxus::desktop::DesktopContext,
    event: KeyboardEvent,
) {
    let code = event.code();
    let modifiers = event.modifiers();
    if code == Code::Escape {
        event.prevent_default();
        if state.read().availability().adjusting {
            let effect = state
                .write()
                .dispatch(Command::CancelPreview, Instant::now());
            effects::run(state, window, effect);
        } else if state.read().dialog.is_some() {
            state.write().dialog = None;
        } else if state.read().long_operation() {
            state.write().cancel_long_operation();
        } else {
            let effect = state.write().dispatch(Command::Quit, Instant::now());
            effects::run(state, window, effect);
        }
        return;
    }
    if state.read().dialog.is_some() || state.read().long_operation() {
        return;
    }
    let meta = modifiers.contains(Modifiers::META);
    if meta
        && !modifiers.contains(Modifiers::ALT)
        && !modifiers.contains(Modifiers::CONTROL)
        && code == Code::KeyQ
    {
        event.prevent_default();
        let effect = state.write().dispatch(Command::Quit, Instant::now());
        effects::run(state, window, effect);
        return;
    }
    if state.read().mode == Mode::Browser
        && !meta
        && !modifiers.contains(Modifiers::ALT)
        && !modifiers.contains(Modifiers::CONTROL)
    {
        let effect = match code {
            Code::ArrowLeft => {
                state.write().move_highlight(-1);
                event.prevent_default();
                None
            }
            Code::ArrowRight => {
                state.write().move_highlight(1);
                event.prevent_default();
                None
            }
            Code::ArrowUp => {
                state.write().move_highlight(-8);
                event.prevent_default();
                None
            }
            Code::ArrowDown => {
                state.write().move_highlight(8);
                event.prevent_default();
                None
            }
            Code::Enter => {
                event.prevent_default();
                Some(state.write().activate_highlighted())
            }
            _ => None,
        };
        if let Some(effect) = effect {
            effects::run(state, window, effect);
        }
        return;
    }
    let Some(key) = shortcut_key(code) else {
        return;
    };
    let mods = Mods {
        command: meta,
        option: modifiers.contains(Modifiers::ALT),
        control: modifiers.contains(Modifiers::CONTROL),
        shift: modifiers.contains(Modifiers::SHIFT),
    };
    let flags = state.read().availability();
    if let Some(command) = command::commands_for_key(mods, key)
        .into_iter()
        .find(|command| command.available(&flags))
    {
        event.prevent_default();
        let effect = state.write().dispatch(command, Instant::now());
        effects::run(state, window, effect);
    }
}

fn shortcut_key(code: Code) -> Option<ShortcutKey> {
    Some(match code {
        Code::ArrowLeft => ShortcutKey::Left,
        Code::ArrowRight => ShortcutKey::Right,
        Code::ArrowUp => ShortcutKey::Up,
        Code::ArrowDown => ShortcutKey::Down,
        Code::Space => ShortcutKey::Space,
        Code::Enter | Code::NumpadEnter => ShortcutKey::Enter,
        Code::KeyA => ShortcutKey::Char('a'),
        Code::KeyB => ShortcutKey::Char('b'),
        Code::KeyC => ShortcutKey::Char('c'),
        Code::KeyD => ShortcutKey::Char('d'),
        Code::KeyE => ShortcutKey::Char('e'),
        Code::KeyF => ShortcutKey::Char('f'),
        Code::KeyI => ShortcutKey::Char('i'),
        Code::KeyM => ShortcutKey::Char('m'),
        Code::KeyN => ShortcutKey::Char('n'),
        Code::KeyO => ShortcutKey::Char('o'),
        Code::KeyR => ShortcutKey::Char('r'),
        Code::KeyS => ShortcutKey::Char('s'),
        Code::KeyU => ShortcutKey::Char('u'),
        Code::KeyW => ShortcutKey::Char('w'),
        Code::KeyZ => ShortcutKey::Char('z'),
        _ => return None,
    })
}
