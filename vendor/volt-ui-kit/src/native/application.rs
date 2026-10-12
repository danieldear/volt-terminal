//! Additive standalone application configuration. No shell or layout is imposed.
use super::{
    messages::{self, Messages},
    window, ApplicationRequest, CloseDecision, CloseReason, CommandRegistry, NativeOptions,
    SurfaceOptions, UiSender, WindowBackground, WindowChrome,
};
use crate::{ui::Element, Palette, ThemeMode, UiTheme};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RendererPreference {
    /// Prefer GPU when compiled in, with opaque software fallback.
    #[default]
    Auto,
    /// Fail rather than silently falling back when GPU is unavailable.
    GpuRequired,
    Software,
}
impl RendererPreference {
    pub(super) fn policy(self, available: bool) -> Result<(bool, bool), String> {
        match self {
            Self::Auto => Ok((available, false)),
            Self::Software => Ok((false, false)),
            Self::GpuRequired if available => Ok((true, true)),
            Self::GpuRequired => Err("GPU required but native-gpu was not compiled in".into()),
        }
    }
}
pub(super) type CommandCallback<C> = Box<dyn FnMut(&mut C, &str)>;
pub(super) type CloseCallback<C> = Box<dyn FnMut(&mut C, CloseReason) -> CloseDecision>;
pub(super) type BackgroundFn<C> = Box<dyn Fn(&C) -> WindowBackground>;
pub(super) type RequestPump = Box<dyn Messages<Vec<ApplicationRequest>>>;
pub(super) struct HostExtensions<C> {
    pub renderer: Option<RendererPreference>,
    pub desktop: Option<super::system::DesktopConfig>,
    pub command: Option<CommandCallback<C>>,
    pub commands: CommandRegistry<C>,
    pub close: Option<CloseCallback<C>>,
    pub requests: Option<RequestPump>,
    pub background: Option<BackgroundFn<C>>,
}
impl<C> Default for HostExtensions<C> {
    fn default() -> Self {
        Self {
            renderer: None,
            desktop: None,
            command: None,
            commands: CommandRegistry::new(),
            close: None,
            requests: None,
            background: None,
        }
    }
}
impl<C> HostExtensions<C> {
    pub fn shortcut_dispatch(
        &mut self,
        context: &mut C,
        key: char,
        modifiers: winit::keyboard::ModifiersState,
        repeat: bool,
    ) -> Option<super::CommandDispatch> {
        let id = self.commands.shortcut_id(key, modifiers)?;
        if repeat {
            // A matching repeat is consumed without invoking either callback path.
            return Some(super::CommandDispatch::Disabled);
        }
        let id = id.to_owned();
        Some(self.dispatch(context, &id))
    }
    pub fn dispatch(&mut self, context: &mut C, id: &str) -> super::CommandDispatch {
        if self
            .desktop
            .as_ref()
            .and_then(|desktop| desktop.command_enabled(id))
            == Some(false)
        {
            return super::CommandDispatch::Disabled;
        }
        let result = self.commands.dispatch(id, context);
        if result != super::CommandDispatch::Unknown {
            return result;
        }
        if self.desktop.as_ref().is_some_and(|c| c.accepts(id)) {
            if let Some(callback) = self.command.as_mut() {
                callback(context, id);
                return super::CommandDispatch::Handled(None);
            }
        }
        super::CommandDispatch::Unknown
    }
    pub fn decide_close(&mut self, context: &mut C, reason: CloseReason) -> CloseDecision {
        self.close
            .as_mut()
            .map_or(CloseDecision::Close, |guard| guard(context, reason))
    }
}
type View<C> = Box<dyn Fn(&C) -> Element<C>>;
/// Configuration plus app-owned state/view. Existing `run_ui` APIs remain unchanged.
pub struct Application<C> {
    options: SurfaceOptions,
    context: C,
    view: View<C>,
    theme: Option<window::ThemeFn<C>>,
    messages: Option<Box<dyn Messages<C>>>,
    extensions: HostExtensions<C>,
}
impl<C: 'static> Application<C> {
    pub fn new(
        title: impl Into<String>,
        state: C,
        view: impl Fn(&C) -> Element<C> + 'static,
    ) -> Self {
        Self {
            options: NativeOptions {
                title: title.into(),
                ..NativeOptions::default()
            }
            .with_theme(ThemeMode::System),
            context: state,
            view: Box::new(view),
            theme: None,
            messages: None,
            extensions: HostExtensions {
                renderer: Some(RendererPreference::Auto),
                ..HostExtensions::default()
            },
        }
    }
    pub fn size(mut self, width: u32, height: u32) -> Self {
        self.options.window.native.size = [width, height];
        self
    }
    /// Advanced surface configuration; validated before creating OS resources.
    pub fn with_options(mut self, options: SurfaceOptions) -> Self {
        self.options = options;
        self
    }
    /// State-aware Rust commands for native shortcuts and custom palettes/buttons.
    /// Registered IDs take precedence over the legacy `desktop` callback.
    pub fn commands(mut self, commands: CommandRegistry<C>) -> Self {
        self.extensions.commands = commands;
        self
    }
    /// Guard native window closes and explicit quit/close requests. KeepOpen may
    /// update state to display Save/Discard/Cancel. Forced OS/Dock termination is
    /// not vetoable through this hook; use an owned desktop menu for guarded Quit.
    pub fn on_close_request(
        mut self,
        close: impl FnMut(&mut C, CloseReason) -> CloseDecision + 'static,
    ) -> Self {
        self.extensions.close = Some(Box::new(close));
        self
    }
    /// A separate bounded UI-request queue; works alongside background `messages`.
    /// Unknown/disabled command IDs never run callbacks. Close requests always
    /// re-evaluate the guard, including requests from a confirmation dialog.
    pub fn requests(
        mut self,
        initialize: impl FnOnce(&mut C, UiSender<ApplicationRequest>),
    ) -> Self {
        let (sender, pump) =
            messages::channel(|requests: &mut Vec<ApplicationRequest>, request| {
                requests.push(request)
            });
        initialize(&mut self.context, sender);
        self.extensions.requests = Some(pump);
        self
    }
    pub fn renderer(mut self, preference: RendererPreference) -> Self {
        self.extensions.renderer = Some(preference);
        self
    }
    pub fn background(mut self, background: WindowBackground) -> Self {
        self.options.background = background;
        self.extensions.background = None;
        self
    }
    /// Derive the window background from application state after UI updates.
    /// This changes the root clear only, never text/control opacity. Invalid
    /// updates are rejected; the previous surface configuration remains active.
    /// Switching between opaque and translucent modes recreates the GPU surface.
    pub fn background_from_state(
        mut self,
        background: impl Fn(&C) -> WindowBackground + 'static,
    ) -> Self {
        self.options.background = background(&self.context);
        self.extensions.background = Some(Box::new(background));
        self
    }
    pub fn chrome(mut self, chrome: WindowChrome) -> Self {
        self.options.window.chrome = chrome;
        self
    }
    pub fn theme(mut self, mode: ThemeMode) -> Self {
        self.options.theme = mode;
        self
    }
    pub fn palette(mut self, palette: Palette) -> Self {
        self.options.palette = Some(palette);
        self
    }
    pub fn reduced_motion(mut self, value: bool) -> Self {
        self.options.window.reduced_motion = value;
        self
    }
    pub fn theme_with(mut self, theme: impl Fn(&C, ThemeMode) -> UiTheme + 'static) -> Self {
        self.theme = Some(Box::new(theme));
        self
    }
    /// Install bounded background messages before starting the window.
    pub fn messages<M: Send + 'static>(
        mut self,
        initialize: impl FnOnce(&mut C, UiSender<M>),
        update: impl FnMut(&mut C, M) + 'static,
    ) -> Self {
        let (sender, pump) = messages::channel(update);
        initialize(&mut self.context, sender);
        self.messages = Some(pump);
        self
    }
    /// Optional native menus/tray. Requires `native-desktop` on macOS.
    pub fn desktop(
        mut self,
        config: super::system::DesktopConfig,
        command: impl FnMut(&mut C, &str) + 'static,
    ) -> Self {
        self.extensions.desktop = Some(config);
        self.extensions.command = Some(Box::new(command));
        self
    }
    pub fn options(&self) -> &SurfaceOptions {
        &self.options
    }
    pub fn renderer_preference(&self) -> RendererPreference {
        self.extensions.renderer.unwrap_or_default()
    }
    /// Configuration validation only; never invokes the view or creates OS resources.
    pub fn validate(&self) -> Result<(), String> {
        if self.options.native.title.trim().is_empty()
            || self.options.native.title.len() > 1024
            || self.options.native.title.chars().any(char::is_control)
            || self
                .options
                .native
                .size
                .iter()
                .any(|&n| n == 0 || n > 16384)
        {
            return Err("invalid application title or window size".into());
        }
        window::validate_options(&self.options)?;
        self.renderer_preference()
            .policy(cfg!(feature = "native-gpu"))?;
        self.extensions.commands.validate()?;
        if let Some(desktop) = &self.extensions.desktop {
            desktop.validate()?;
            if !super::system::capabilities().native_menu {
                return Err("native desktop integration requires native-desktop on macOS".into());
            }
        }
        Ok(())
    }
    pub fn run(mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Message/request initializers may have changed state after the builder.
        if let Some(background) = &self.extensions.background {
            self.options.background = background(&self.context);
        }
        self.validate()?;
        window::run_ui_extended(
            self.options,
            self.context,
            self.view,
            self.theme,
            self.messages,
            self.extensions,
        )
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use crate::native::{Command, CommandDispatch};
    #[derive(Default)]
    struct State {
        dirty: bool,
        prompts: usize,
        saves: usize,
    }
    #[test]
    fn state_background_callback_is_additive_and_keeps_public_defaults() {
        use crate::ui::spacer;
        let app =
            Application::new("Glass", 72u8, |_| spacer().into()).background_from_state(|opacity| {
                WindowBackground::CompositorBlur {
                    opacity: *opacity as f32 / 100.,
                }
            });
        assert_eq!(
            app.options().background,
            WindowBackground::CompositorBlur { opacity: 0.72 }
        );
        let callback = app.extensions.background.as_ref().unwrap();
        assert_eq!(
            callback(&100),
            WindowBackground::CompositorBlur { opacity: 1. }
        );
        assert_eq!(
            callback(&10),
            WindowBackground::CompositorBlur { opacity: 0.1 }
        );
        let fixed = app.background(WindowBackground::Opaque);
        assert!(fixed.extensions.background.is_none());
        assert_eq!(
            Application::new("Default", (), |_| spacer().into())
                .options()
                .background,
            WindowBackground::Opaque
        );
    }
    #[test]
    fn close_veto_is_rechecked_after_save_and_default_legacy_close_is_unchanged() {
        let mut extensions: HostExtensions<State> = HostExtensions::default();
        let mut state = State {
            dirty: true,
            ..State::default()
        };
        assert_eq!(
            extensions.decide_close(&mut state, CloseReason::Window),
            CloseDecision::Close
        );
        extensions.close = Some(Box::new(|state, _| {
            if state.dirty {
                state.prompts += 1;
                CloseDecision::KeepOpen
            } else {
                CloseDecision::Close
            }
        }));
        for reason in [CloseReason::Window, CloseReason::Quit] {
            assert_eq!(
                extensions.decide_close(&mut state, reason),
                CloseDecision::KeepOpen
            );
        }
        assert_eq!(state.prompts, 2);
        state.dirty = false;
        assert_eq!(
            extensions.decide_close(&mut state, CloseReason::Quit),
            CloseDecision::Close
        );
    }
    #[test]
    fn registered_disabled_commands_never_fall_through_to_desktop_callbacks() {
        use crate::native::system::{DesktopCommand, DesktopConfig, DesktopMenu};
        let mut extensions = HostExtensions::<State> {
            commands: CommandRegistry::new().command(
                Command::new("save", "Save", |s: &mut State| s.saves += 1)
                    .enabled_when(|s| s.dirty),
            ),
            desktop: Some(DesktopConfig::new("App").menu(DesktopMenu::new(
                "File",
                [
                    DesktopCommand::new("save", "Save"),
                    DesktopCommand::new("legacy", "Legacy"),
                ],
            ))),
            command: Some(Box::new(|s, _| s.saves += 100)),
            ..HostExtensions::default()
        };
        let mut state = State::default();
        assert_eq!(
            extensions.dispatch(&mut state, "save"),
            CommandDispatch::Disabled
        );
        assert_eq!(
            extensions.dispatch(&mut state, "invented"),
            CommandDispatch::Unknown
        );
        assert_eq!(state.saves, 0);
        state.dirty = true;
        assert_eq!(
            extensions.dispatch(&mut state, "save"),
            CommandDispatch::Handled(None)
        );
        assert_eq!(state.saves, 1);
        assert_eq!(
            extensions.dispatch(&mut state, "legacy"),
            CommandDispatch::Handled(None)
        );
        assert_eq!(state.saves, 101);
    }
    #[test]
    fn static_disabled_menu_definitions_also_gate_keyboard_and_explicit_requests() {
        use crate::native::system::{DesktopCommand, DesktopConfig, DesktopMenu};
        let mut extensions = HostExtensions::<State> {
            commands: CommandRegistry::new().command(Command::new(
                "save",
                "Save",
                |s: &mut State| s.saves += 1,
            )),
            desktop: Some(DesktopConfig::new("App").menu(DesktopMenu::new(
                "File",
                [DesktopCommand::new("save", "Save").enabled(false)],
            ))),
            ..HostExtensions::default()
        };
        let mut state = State::default();
        assert_eq!(
            extensions.dispatch(&mut state, "save"),
            CommandDispatch::Disabled
        );
        assert_eq!(state.saves, 0);
    }
    #[test]
    fn counted_shortcut_dispatch_consumes_repeat_and_exact_modifiers() {
        use crate::native::Shortcut;
        use winit::keyboard::ModifiersState;
        let mut extensions = HostExtensions::<State> {
            commands: CommandRegistry::new().command(
                Command::new("probe", "Probe", |s: &mut State| s.saves += 1)
                    .shortcut(Shortcut::primary('p').shift(true)),
            ),
            ..HostExtensions::default()
        };
        let primary = if cfg!(target_os = "macos") {
            ModifiersState::SUPER
        } else {
            ModifiersState::CONTROL
        };
        let modifiers = primary | ModifiersState::SHIFT;
        let mut state = State::default();
        assert_eq!(
            extensions.shortcut_dispatch(&mut state, 'p', modifiers, false),
            Some(CommandDispatch::Handled(None))
        );
        for _ in 0..50 {
            assert_eq!(
                extensions.shortcut_dispatch(&mut state, 'p', modifiers, true),
                Some(CommandDispatch::Disabled)
            );
        }
        assert_eq!(
            extensions.shortcut_dispatch(&mut state, 'p', primary, false),
            None
        );
        assert_eq!(state.saves, 1);
        extensions.shortcut_dispatch(&mut state, 'p', modifiers, false);
        assert_eq!(state.saves, 2);
    }
    #[test]
    fn requests_use_the_bounded_queue_and_veto_does_not_close_it() {
        let mut extensions = HostExtensions::<()> {
            close: Some(Box::new(|_, _| CloseDecision::KeepOpen)),
            ..HostExtensions::default()
        };
        let (sender, mut pump) =
            messages::channel(|requests: &mut Vec<ApplicationRequest>, r| requests.push(r));
        for _ in 0..256 {
            sender
                .send(ApplicationRequest::Close(CloseReason::Window))
                .unwrap();
        }
        assert_eq!(
            sender.send(ApplicationRequest::Close(CloseReason::Window)),
            Err(super::super::UiSendError::Full)
        );
        let mut requests = vec![];
        assert!(pump.drain(&mut requests));
        assert_eq!(requests.len(), 64);
        assert_eq!(
            extensions.decide_close(&mut (), CloseReason::Window),
            CloseDecision::KeepOpen
        );
        assert!(!sender.is_closed());
        pump.close();
        assert_eq!(
            sender.send(ApplicationRequest::Close(CloseReason::Window)),
            Err(super::super::UiSendError::Closed)
        );
    }
}
