# Roadmap

## Now

- **Build a real app with the kit.** Rebuild the mdstar Markdown reader and
  editor on Volt UI Kit, and fix every gap it uncovers in the kit itself.
- **Rich text.** Styled runs within one paragraph (bold, italic, code, links),
  selectable and copyable read-only text, and syntax colouring in the editor.
- **API documentation** for every public item, written once the APIs above
  settle.
- **Public repository, CI and a project website.**

## Next

- Upgrade to current Vello and wgpu.
- Glass over app content: blur and tint the app's own content behind panels on
  the GPU, not just the desktop behind the window.
- Validate the native runtime on Windows and Linux, and screen readers
  (VoiceOver, Narrator, Orca), IME and trackpads on each platform.
- Keyboard navigation through combobox and command palette results;
  independent range-slider thumbs for assistive technology.
- Multiple windows, state restoration and the full OS quit lifecycle.
- File panels and notifications on Windows and Linux.

## Later

- A web host: draw in a browser canvas through WebGPU (the core already
  compiles to WebAssembly).
- Finer-grained updates and partial GPU redraws.
- Layout animation, springs and gestures; variable-height virtual lists.
- Embedded web views, maps and on-device AI on macOS.
- Publish to crates.io once the APIs have held up in real applications.
