use std::sync::{Arc, Mutex};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
use winit::window::{Window, WindowId};

use volt_config::{Config, Theme};
use volt_core::events::CoreEvent;
use volt_core::performer::Performer;
use volt_core::pty::Pty;
use volt_renderer::Renderer;

// State once the window is created
struct WindowState {
    window: Arc<Window>,
    renderer: Renderer,
    pty: Pty,
    performer: Arc<Mutex<Performer>>,
    event_rx: tokio::sync::mpsc::UnboundedReceiver<CoreEvent>,
    theme: Theme,
    modifiers: ModifiersState,
}

pub struct App {
    config: Config,
    state: Option<WindowState>,
    rt: tokio::runtime::Runtime,
}

impl App {
    pub fn new(config: Config) -> Self {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        Self { config, state: None, rt }
    }

    pub fn run(mut self) {
        let event_loop = EventLoop::new().unwrap();
        event_loop.run_app(&mut self).unwrap();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("Volt")
            .with_inner_size(winit::dpi::PhysicalSize::new(1200u32, 800u32));
        let window = Arc::new(event_loop.create_window(window_attrs).unwrap());

        let font_size = self.config.font.size;
        let renderer = self.rt.block_on(Renderer::new(window.clone(), font_size));

        let (cols, rows) = renderer.grid_size();
        let (pty, performer, event_rx) = Pty::spawn(
            &self.config.shell.program,
            &self.config.shell.args,
            cols as u16,
            rows as u16,
        )
        .expect("failed to spawn PTY");

        self.state = Some(WindowState {
            window,
            renderer,
            pty,
            performer,
            event_rx,
            theme: Theme::dark(),
            modifiers: ModifiersState::default(),
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = &mut self.state else {
            return;
        };

        // Drain any pending CoreEvents (non-blocking)
        while let Ok(ev) = state.event_rx.try_recv() {
            if matches!(ev, CoreEvent::GridUpdated) {
                state.window.request_redraw();
            }
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(size) => {
                state.renderer.resize(size.width, size.height);
                let (cols, rows) = state.renderer.grid_size();
                let _ = state.pty.resize(cols as u16, rows as u16);
                state.performer.lock().unwrap().grid.resize(cols, rows);
            }

            WindowEvent::ModifiersChanged(mods) => {
                state.modifiers = mods.state();
            }

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key,
                        state: key_state,
                        text,
                        ..
                    },
                ..
            } => {
                if key_state != ElementState::Pressed {
                    return;
                }

                // Special keys → escape sequences
                let special: Option<&[u8]> = match physical_key {
                    PhysicalKey::Code(KeyCode::Enter) => Some(b"\r"),
                    PhysicalKey::Code(KeyCode::Backspace) => Some(b"\x7f"),
                    PhysicalKey::Code(KeyCode::Tab) => Some(b"\t"),
                    PhysicalKey::Code(KeyCode::Escape) => Some(b"\x1b"),
                    PhysicalKey::Code(KeyCode::ArrowUp) => Some(b"\x1b[A"),
                    PhysicalKey::Code(KeyCode::ArrowDown) => Some(b"\x1b[B"),
                    PhysicalKey::Code(KeyCode::ArrowRight) => Some(b"\x1b[C"),
                    PhysicalKey::Code(KeyCode::ArrowLeft) => Some(b"\x1b[D"),
                    PhysicalKey::Code(KeyCode::Home) => Some(b"\x1b[H"),
                    PhysicalKey::Code(KeyCode::End) => Some(b"\x1b[F"),
                    PhysicalKey::Code(KeyCode::PageUp) => Some(b"\x1b[5~"),
                    PhysicalKey::Code(KeyCode::PageDown) => Some(b"\x1b[6~"),
                    PhysicalKey::Code(KeyCode::Delete) => Some(b"\x1b[3~"),
                    _ => None,
                };

                if let Some(bytes) = special {
                    let _ = state.pty.write(bytes);
                    return;
                }

                // Ctrl+key → send control byte
                if state.modifiers.control_key() {
                    if let PhysicalKey::Code(code) = physical_key {
                        if let Some(b) = ctrl_code(code) {
                            let _ = state.pty.write(&[b]);
                            return;
                        }
                    }
                }

                // Printable text
                if let Some(text) = text {
                    let _ = state.pty.write(text.as_str().as_bytes());
                }
            }

            WindowEvent::RedrawRequested => {
                let p = state.performer.lock().unwrap();
                state.renderer.render(&p.grid, &state.theme);
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &mut self.state {
            let mut needs_redraw = false;
            while let Ok(ev) = state.event_rx.try_recv() {
                if matches!(ev, CoreEvent::GridUpdated) {
                    needs_redraw = true;
                }
            }
            if needs_redraw {
                state.window.request_redraw();
            }
        }
    }
}

/// Map KeyCode to Ctrl+key byte (Ctrl+A = 0x01, Ctrl+C = 0x03, etc.)
fn ctrl_code(code: KeyCode) -> Option<u8> {
    let letter = match code {
        KeyCode::KeyA => b'A',
        KeyCode::KeyB => b'B',
        KeyCode::KeyC => b'C',
        KeyCode::KeyD => b'D',
        KeyCode::KeyE => b'E',
        KeyCode::KeyF => b'F',
        KeyCode::KeyG => b'G',
        KeyCode::KeyH => b'H',
        KeyCode::KeyI => b'I',
        KeyCode::KeyJ => b'J',
        KeyCode::KeyK => b'K',
        KeyCode::KeyL => b'L',
        KeyCode::KeyM => b'M',
        KeyCode::KeyN => b'N',
        KeyCode::KeyO => b'O',
        KeyCode::KeyP => b'P',
        KeyCode::KeyQ => b'Q',
        KeyCode::KeyR => b'R',
        KeyCode::KeyS => b'S',
        KeyCode::KeyT => b'T',
        KeyCode::KeyU => b'U',
        KeyCode::KeyV => b'V',
        KeyCode::KeyW => b'W',
        KeyCode::KeyX => b'X',
        KeyCode::KeyY => b'Y',
        KeyCode::KeyZ => b'Z',
        _ => return None,
    };
    Some(letter - b'@') // A=0x01, C=0x03, etc.
}
