use crate::support::RustLaunchOptions;
use volt_ui_kit::{
    native::{WindowBackground, WindowChrome},
    ui::prelude::*,
};
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}
pub struct Message {
    pub id: u64,
    pub role: Role,
    pub body: String,
}
pub struct Thread {
    pub id: u64,
    pub title: String,
    pub messages: Vec<Message>,
}
pub struct App {
    pub threads: Vec<Thread>,
    pub active: u64,
    pub next_id: u64,
    pub sidebar: bool,
    pub inspector: bool,
    pub theme: ThemeMode,
    pub prompt: usize,
    pub draft: String,
    pub sent: usize,
    pub saved: bool,
    pub show_code: bool,
    pub status: String,
}
pub const PROMPTS: [&str; 3] = [
    "Show me the component architecture.",
    "Review the interaction checklist.",
    "Explain the typography choices.",
];
impl Default for App {
    fn default() -> Self {
        let messages=vec![
            Message{id:10,role:Role::User,body:"Let's build a quiet, focused desktop workspace. Keep the navigation clear and the details intentional.".into()},
            Message{id:11,role:Role::Assistant,body:"A good workspace should leave room for the work. This one is built entirely with native Rust UI components: a project sidebar, a scrollable conversation, an editable composer and a details panel.".into()},
            Message{id:12,role:Role::Assistant,body:"The useful details\n\n• SF for interface text, SF Mono for code and metadata\n• Independent scrolling for each pane\n• Stable keys for conversations and messages\n• Real Rust callbacks for navigation and actions".into()},
        ];
        Self { threads:vec![Thread{id:1,title:"Design the native workspace".into(),messages},
            Thread{id:2,title:"Refine colors and typography".into(),messages:vec![Message{id:20,role:Role::Assistant,body:"The dark palette uses graphite surfaces and a restrained green accent. Switch appearance in the title bar to compare the light theme.".into()}]},
            Thread{id:3,title:"Review the component API".into(),messages:vec![Message{id:30,role:Role::Assistant,body:"Composition is Rust-first. Views return Element<App>; typed closures change local state and the host recomposes the tree.".into()}]}],
            active:1,next_id:100,sidebar:true,inspector:true,theme:ThemeMode::Dark,prompt:0,draft:PROMPTS[0].into(),sent:0,saved:false,show_code:true,
            status:"Local preview. No AI service connected.".into() }
    }
}
impl App {
    pub fn current(&self) -> &Thread {
        self.threads
            .iter()
            .find(|t| t.id == self.active)
            .expect("active thread invariant")
    }
    pub fn select(&mut self, id: u64) {
        if let Some(t) = self.threads.iter().find(|t| t.id == id) {
            self.active = id;
            self.status = format!("Opened {}", t.title);
        }
    }
    pub fn new_chat(&mut self) {
        let id = self.next_id;
        self.next_id += 1;
        if self.threads.len() == 6 {
            self.threads.remove(0);
        }
        self.threads.push(Thread {
            id,
            title: format!("Untitled conversation {}", id - 99),
            messages: vec![],
        });
        self.active = id;
        self.status = "Created a local conversation".into();
    }
    pub fn send(&mut self) {
        let question = self.draft.trim().to_owned();
        if question.is_empty() {
            return;
        }
        let answer=match self.prompt {
            0=>"Local sample response: the view is assembled from HStack, VStack, Panel, Text and Button. Canvas draws the original project illustrations; the native adapter resolves fonts and paints the result.",
            1=>"Local sample response: check sidebar selection, theme changes, hidden-pane restoration, independent scrolling, Enter/Space activation, and stable message keys after switching conversations.",
            _=>"Local sample response: SF is used for readable conversation and navigation text. SF Mono separates source snippets, file names and counters. Apple fonts are resolved from this Mac, never bundled.",
        };
        let thread = self
            .threads
            .iter_mut()
            .find(|t| t.id == self.active)
            .unwrap();
        if thread.messages.len() > 14 {
            thread.messages.drain(..2);
        }
        thread.messages.push(Message {
            id: self.next_id,
            role: Role::User,
            body: question,
        });
        self.next_id += 1;
        thread.messages.push(Message {
            id: self.next_id,
            role: Role::Assistant,
            body: answer.into(),
        });
        self.next_id += 1;
        self.sent += 1;
        self.status =
            "Sent your text; generated a fixed local demo response. Scroll to see it.".into();
    }
}
pub fn theme(app: &App, system: ThemeMode) -> UiTheme {
    let light = app.theme == ThemeMode::Light
        || (app.theme == ThemeMode::System && system == ThemeMode::Light);
    let rgba = |h: u32| {
        [
            ((h >> 16) & 255) as f32 / 255.,
            ((h >> 8) & 255) as f32 / 255.,
            (h & 255) as f32 / 255.,
            1.,
        ]
    };
    let mut p = if light {
        Palette::light()
    } else {
        Palette::dark()
    };
    p.name = if light { "Desk light" } else { "Desk graphite" }.into();
    p.base = if light {
        [rgba(0xFAFBFC), rgba(0xF1F3F5), rgba(0xE3E7EC)]
    } else {
        [rgba(0x0E1013), rgba(0x17191D), rgba(0x272A30)]
    };
    p.content = rgba(if light { 0x242A32 } else { 0xE6E8EB });
    p.muted = rgba(if light { 0x65707C } else { 0xA4AAB4 });
    p.border = rgba(if light { 0xD3D8DF } else { 0x30333B });
    p = p.with_accent(rgba(if light { 0x28674B } else { 0xA3D8B7 }));
    UiTheme::from_palette(p)
}
pub fn parse_args(
    args: impl IntoIterator<Item = String>,
) -> Result<(App, RustLaunchOptions), Box<dyn std::error::Error>> {
    let mut app = App::default();
    let mut args = args.into_iter();
    let mut forwarded = vec!["--theme".into(), "system".into()];
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--compact" => {
                app.sidebar = false;
                app.inspector = false;
            }
            "--no-inspector" => app.inspector = false,
            "--theme" => {
                app.theme = match args.next().as_deref() {
                    Some("dark") => ThemeMode::Dark,
                    Some("light") => ThemeMode::Light,
                    Some("system") => ThemeMode::System,
                    _ => return Err("--theme must be dark, light or system".into()),
                }
            }
            "--help" => {
                println!("Volt Desk — original desktop-workbench example\n--compact hides sidebar and inspector on launch\n--no-inspector hides only inspector\n--theme light|dark|system\nComposer uses selectable sample prompts, not an editable text field. No AI connection.");
                forwarded.push(arg);
            }
            _ => forwarded.push(arg),
        }
    }
    Ok((
        app,
        RustLaunchOptions {
            chrome: WindowChrome::Overlay,
            background: WindowBackground::Opaque,
            args: forwarded,
        },
    ))
}
