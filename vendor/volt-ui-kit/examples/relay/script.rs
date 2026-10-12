//! Sample threads and the offline "agent": canned work steps, file changes
//! and an answer chosen from the prompt's wording. No network, no model.
use super::{Effort, FileChange, Message, Step, Thread};

pub struct Reply {
    /// (step label, completion detail, simulated milliseconds)
    pub steps: Vec<(&'static str, &'static str, u64)>,
    pub files: Vec<FileChange>,
    pub answer: String,
}

fn file(path: &str, added: u32, removed: u32) -> FileChange {
    FileChange {
        path: path.into(),
        added,
        removed,
    }
}

pub fn title_for(prompt: &str) -> String {
    let line = prompt.lines().next().unwrap_or(prompt).trim();
    let mut title: String = line.chars().take(36).collect();
    if line.chars().count() > 36 {
        // Break at the last whole word.
        if let Some(space) = title.rfind(' ') {
            title.truncate(space);
        }
        title = format!("{}…", title.trim_end());
    }
    let mut chars = title.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_else(|| "New chat".into())
}

pub fn reply_for(prompt: &str, effort: Effort) -> Reply {
    let p = prompt.to_lowercase();
    let mut reply = if p.contains("test") {
        Reply {
            steps: vec![
                ("Read Cargo.toml", "4 feature sets", 420),
                ("Ran cargo test --all-features", "441 passed · 12 ignored", 1400),
                ("Ran cargo clippy -- -D warnings", "No warnings", 900),
            ],
            files: vec![],
            answer: "All green. The full suite passes with every feature enabled: 441 tests, with 12 hardware tests skipped because they need a real GPU or a display.\n\nClippy is clean with warnings denied.\n\n- Default features: 40 passed\n- Rust 1.87 minimum: builds\n- Linux cross-check: builds".into(),
        }
    } else if p.contains("fix") || p.contains("bug") || p.contains("border") {
        Reply {
            steps: vec![
                ("Searched for paint_border", "3 matches in 2 files", 600),
                ("Read src/native/gpu_scene.rs", "Lines 560–620", 500),
                ("Reproduced on the GPU", "Interior filled at y < 16", 1100),
                ("Edited src/native/gpu_scene.rs", "+14 −0", 700),
                ("Ran cargo test --features native-gpu", "Regression test passes", 1200),
            ],
            files: vec![
                file("src/native/gpu_scene.rs", 14, 0),
                file("src/native/gpu_tests.rs", 31, 0),
            ],
            answer: "Fixed. A bordered sheet flush with the top of the window lost its background in the first 16 pixels on the GPU path only.\n\nThe border is four butt-capped lines. Stroked by the GPU rasterizer, a long horizontal line lying exactly on the surface's top edge filled its whole first tile row. Axis-aligned lines are now filled as exact rectangles instead, which is also cheaper.\n\n```\nShape::Line(a, b, w, c) if a[0] == b[0] || a[1] == b[1] => {\n    scene.fill(Fill::NonZero, IDENTITY, color(*c), None, &rect(r));\n}\n```\n\nI added a GPU regression test that fails without the change.".into(),
        }
    } else if p.contains("design") || p.contains("ui") || p.contains("glass") {
        Reply {
            steps: vec![
                ("Read examples/catalog/shell.rs", "562 lines", 500),
                ("Rendered snapshots", "Light and dark, 1× and 2×", 1300),
                ("Edited examples/catalog/shell.rs", "+96 −41", 800),
            ],
            files: vec![
                file("examples/catalog/shell.rs", 96, 41),
                file("examples/catalog.rs", 9, 2),
            ],
            answer: "The window now reads as two materials. The sidebar sits on the frosted window background, and the documentation lives on a solid sheet with rounded leading corners, so text never sits on top of the blur.\n\n- Traffic lights sit over the sidebar, above the brand row\n- The header is part of the sheet and stays draggable\n- Opaque mode keeps the same layout with a tinted sidebar".into(),
        }
    } else if p.contains("commit") {
        Reply {
            steps: vec![
                ("Ran git status", "6 files changed", 400),
                ("Ran git commit", "419725f", 700),
            ],
            files: vec![],
            answer: "Committed as 419725f — feat: add application foundation and polish catalog native UI.\n\nWorking tree is clean. Not pushed.".into(),
        }
    } else {
        Reply {
            steps: vec![
                ("Explored the workspace", "128 files", 700),
                ("Read README.md", "Overview and features", 500),
                ("Read src/ui.rs", "Component builders", 600),
            ],
            files: vec![],
            answer: format!(
                "Here's the short version of \"{}\".\n\nVolt UI is a declarative Rust UI toolkit. Your view is an ordinary function of your state; the runtime diffs, lays out and paints it with either a CPU rasterizer or a GPU renderer.\n\n- Components: buttons, inputs, menus, dialogs, tables, trees\n- Accessibility through the platform screen reader\n- Native windows on macOS, Windows and Linux\n\nAsk me to fix something, run the tests or polish the design.",
                title_for(prompt)
            ),
        }
    };
    if effort == Effort::Low {
        reply.steps.truncate(1);
    }
    reply
}

fn done(label: &str, detail: &str) -> Step {
    Step {
        label: label.into(),
        detail: detail.into(),
        done: true,
    }
}

pub fn sample_threads() -> Vec<Thread> {
    let mut analysis = Message::agent(
        "The catalog's opaque mode showed a grey band across the top of the content. It only happens on the GPU renderer: software output is correct.\n\nThe content sheet's border is drawn as four stroked lines, and the top one lies on the window's top edge. On the GPU that stroke fills the entire first 16-pixel tile row with the border colour.\n\n- Software renderer: correct\n- GPU, rounded corners: correct\n- GPU, square and flush with the top: band",
    );
    analysis.steps = vec![
        done("Rendered both renderers", "Software correct, GPU wrong"),
        done(
            "Read src/declarative/runtime.rs",
            "paint_border emits lines",
        ),
        done("Bisected geometry", "Only y = 0, square corners"),
    ];
    analysis.worked = Some(25);
    let mut fix = Message::agent(
        "Fixed and committed. Axis-aligned lines are now filled as rectangles on the GPU, and a regression test reproduces the original band.",
    );
    fix.steps = vec![
        done("Edited src/native/gpu_scene.rs", "+14 −0"),
        done("Ran cargo test --all-features", "442 passed"),
    ];
    fix.files = vec![
        FileChange {
            path: "src/native/gpu_scene.rs".into(),
            added: 14,
            removed: 0,
        },
        FileChange {
            path: "src/native/gpu_tests.rs".into(),
            added: 31,
            removed: 0,
        },
    ];
    fix.worked = Some(41);
    let thread = |id, project: &str, title: &str, pinned, messages| Thread {
        id,
        project: project.into(),
        title: title.into(),
        pinned,
        unread: false,
        messages,
    };
    let mut tab = Message::agent("Tab strip layout is now cached per window width; dragging a tab no longer re-measures every title.");
    tab.worked = Some(18);
    let mut scanner = Message::agent("The scanner view is wired to the camera session; barcodes resolve against the local food database first.");
    scanner.worked = Some(33);
    let mut frosted = Message::agent("The sidebar now sits on the window's blurred material and the content is a solid sheet. Pass --background opaque for the previous look.");
    frosted.worked = Some(52);
    let mut compare = Message::agent("Closest peers are iced and Slint. Volt is smaller, but its accessibility, damage tracking and GPU text are already competitive; the gaps are text shaping breadth, platform packaging and mobile.");
    compare.worked = Some(12);
    vec![
        thread(
            1,
            "volt-ui-kit",
            "Standalone desktop UI toolkit",
            true,
            vec![
                Message::user("Why does the catalog have a grey band at the top in opaque mode?"),
                analysis,
                Message::user("Fix it and commit the code"),
                fix,
            ],
        ),
        thread(
            2,
            "volt-ui-kit",
            "Frosted catalog window",
            false,
            vec![
                Message::user("Make the catalog sidebar frosted like a native Mac app"),
                frosted,
            ],
        ),
        thread(
            3,
            "glass-browser",
            "Review tab strip performance",
            true,
            vec![
                Message::user("Dragging tabs stutters with 40 tabs open"),
                tab,
            ],
        ),
        thread(
            4,
            "mealfit-app",
            "Add barcode scanner",
            false,
            vec![
                Message::user("Add a barcode scanner to the food log"),
                scanner,
            ],
        ),
        thread(
            5,
            "volt-ui-kit",
            "Compare Rust UI toolkits",
            false,
            vec![
                Message::user("Where do we stand against other Rust UI toolkits?"),
                compare,
            ],
        ),
    ]
}
