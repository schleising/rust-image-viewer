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
html, body { margin: 0; height: 100%; background: #111; }
#root { height: 100vh; outline: none; color-scheme: light dark; background: light-dark(#f3f3f3, #161616); color: light-dark(#111, #eee); font: 13px -apple-system, BlinkMacSystemFont, sans-serif; position: relative; }
.browser { display: grid; grid-template-columns: repeat(8, minmax(0, 1fr)); align-content: start; overflow: auto; height: 100%; }
.cell { display: flex; flex-direction: column; gap: 4px; margin: 0; padding: 8px; border: 0; background: transparent; color: inherit; text-align: center; min-width: 0; }
.cell.selected { background: rgba(30, 144, 255, 0.5); }
.thumb-box { aspect-ratio: 1; display: flex; align-items: center; justify-content: center; overflow: hidden; }
.thumb { width: 100%; height: 100%; object-fit: contain; }
.thumb.loading { width: 40%; height: 40%; opacity: 0.2; }
.thumb-error { font-size: 11px; padding: 4px; }
.label { white-space: nowrap; overflow: hidden; }
.stage { position: relative; width: 100%; height: 100%; overflow: hidden; background: light-dark(#ececec, #101010); }
.stage.busy .frame, .stage.busy #player { opacity: 0.55; }
.frame, #player { user-select: none; -webkit-user-drag: none; }
.selection { position: absolute; box-sizing: border-box; background: rgba(30, 144, 255, 0.5); border: 1px solid rgb(30, 144, 255); pointer-events: none; }
.scrubber { position: absolute; left: 100px; right: 100px; bottom: 100px; height: 4px; background: rgba(30, 144, 255, 0.5); }
.scrubber .mark { position: absolute; top: -2px; width: 8px; height: 8px; background: white; }
.dialog-back { position: absolute; inset: 0; background: rgba(0, 0, 0, 0.35); display: flex; align-items: center; justify-content: center; }
.dialog { background: light-dark(#fff, #2c2c2c); color: inherit; padding: 16px 18px; min-width: 280px; border-radius: 8px; }
.dialog h2 { margin: 0 0 12px; font-size: 16px; }
.info-row, .slider { display: flex; justify-content: space-between; gap: 16px; margin: 6px 0; }
.slider input { width: 160px; }
.dialog-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 14px; }
.message { position: absolute; left: 12px; right: 12px; top: 12px; background: rgba(0, 0, 0, 0.78); color: white; padding: 8px 10px; border-radius: 6px; }
"#;

pub fn app() -> Element {
    let mut state = use_signal(AppState::new);
    let window = use_window();
    use_hook(|| {
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
            if let Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } = event
            {
                let scale = window.scale_factor().max(1.0);
                state.write().set_view_size(
                    (size.width as f64 / scale, size.height as f64 / scale),
                    scale,
                );
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
    use_effect({
        let window = window.clone();
        move || {
            if state.read().pending_restore.is_some() {
                state.write().commit_restore();
            }
            window.set_title(&state.read().title());
            window.set_fullscreen(state.read().fullscreen);
            menu::sync(&state.read().availability());
            protocol::publish(&state.read());
            crate::browser::schedule(&state.read());
            let epoch = state.read().video_epoch;
            if epoch != synced() {
                synced.set(epoch);
                if let Some(video) = state.read().video().cloned() {
                    viewer::sync_video(&video);
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
        }
    });

    let mode = state.read().mode;
    let message = state.read().message.clone();
    let window_keys = window.clone();
    rsx! {
        style { "{CSS}" }
        div {
            id: "root",
            tabindex: "0",
            onmounted: move |_| {
                dioxus::document::eval("document.getElementById('root')?.focus()");
            },
            onkeydown: move |event| on_key(state, window_keys.clone(), event),
            if mode == Mode::Browser {
                {crate::browser::view(state, window.clone())}
            } else {
                {viewer::view(state, window.clone())}
            }
            {crate::dialogs::view(state, window.clone())}
            if let Some(message) = message {
                button {
                    class: "message",
                    onclick: move |_| state.write().dismiss_message(),
                    "{message}"
                }
            }
        }
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
        } else {
            let effect = state.write().dispatch(Command::Quit, Instant::now());
            effects::run(state, window, effect);
        }
        return;
    }
    if state.read().dialog.is_some() {
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
