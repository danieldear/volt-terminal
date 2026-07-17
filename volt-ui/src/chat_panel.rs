//! AI Chat Panel — toggleable sidebar (Cmd+Shift+A) for in-terminal AI assistance.
//!
//! The panel renders on the right side of the window (320px default width).
//! When visible, the terminal viewport width is reduced accordingly.
//! Messages and the current input buffer are stored here; the actual HTTP calls
//! are dispatched via `tokio::spawn` and results returned through the app event loop.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
        }
    }
}

/// State for the AI Chat Panel sidebar.
pub struct ChatPanel {
    /// Whether the panel is currently shown.
    pub visible: bool,
    /// Conversation history (system prompt not stored here — injected per-request).
    pub messages: Vec<ChatMessage>,
    /// Current text in the input field.
    pub input_buf: String,
    /// Whether the panel input field has keyboard focus.
    pub focused: bool,
    /// Scroll offset in the message list (in pixels, positive = scrolled down).
    pub scroll_offset: f32,
    /// True while waiting for a streaming LLM response.
    pub pending_response: bool,
    /// Panel width in physical pixels.
    pub width: f32,
}

impl Default for ChatPanel {
    fn default() -> Self {
        Self {
            visible: false,
            messages: Vec::new(),
            input_buf: String::new(),
            focused: false,
            scroll_offset: 0.0,
            pending_response: false,
            width: 320.0,
        }
    }
}

impl ChatPanel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggle visibility; clears input focus when hiding.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        self.focused = self.visible;
    }

    /// Returns the panel width to subtract from the terminal viewport, or 0 if hidden.
    pub fn terminal_width_reduction(&self) -> f32 {
        if self.visible {
            self.width
        } else {
            0.0
        }
    }

    /// Append a character to the input buffer when the panel is focused.
    pub fn push_char(&mut self, c: char) {
        if self.focused && !self.pending_response {
            self.input_buf.push(c);
        }
    }

    /// Delete last character from the input buffer.
    pub fn backspace(&mut self) {
        if self.focused {
            self.input_buf.pop();
        }
    }

    /// Consume the current input buffer and append it as a user message.
    /// Returns the input text if non-empty (caller should dispatch the LLM request).
    pub fn submit_input(&mut self) -> Option<String> {
        let text = self.input_buf.trim().to_string();
        if text.is_empty() || self.pending_response {
            return None;
        }
        self.input_buf.clear();
        self.messages.push(ChatMessage::user(&text));
        self.pending_response = true;
        Some(text)
    }

    /// Append a chunk of streaming response text to the last assistant message.
    pub fn append_chunk(&mut self, chunk: &str) {
        if let Some(last) = self.messages.last_mut() {
            if last.role == MessageRole::Assistant {
                last.content.push_str(chunk);
                return;
            }
        }
        // No assistant message yet — start one.
        self.messages.push(ChatMessage::assistant(chunk));
    }

    /// Mark the streaming response as complete.
    pub fn finish_response(&mut self) {
        self.pending_response = false;
    }

    /// Record an error in place of the pending response.
    pub fn record_error(&mut self, err: &str) {
        self.pending_response = false;
        self.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: format!("⚠ Error: {err}"),
        });
    }
}
