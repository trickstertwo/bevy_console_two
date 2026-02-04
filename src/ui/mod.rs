//! The module that handles the user interface of the console.

use std::time::SystemTime;

use bevy::prelude::*;
use bevy_egui::egui::text::LayoutJob;
use bevy_egui::*;

use crate::config::ConsoleConfig;
use crate::logging::LogMessage;
use crate::core::{ConsoleInputEvent, ConsoleRegistry};

mod completions;
pub use completions::MAX_COMPLETION_SUGGESTIONS;

/// Prefix for log messages that show a previous command.
pub const COMMAND_MESSAGE_PREFIX: &str = "$ ";
/// Prefix for log messages that show the result of a command.
pub const COMMAND_RESULT_PREFIX: &str = "> ";
/// Identifier for log messages that show a previous command.
pub const COMMAND_MESSAGE_NAME: &str = "console_command";
/// Identifier for log messages that show the result of a command.
pub const COMMAND_RESULT_NAME: &str = "console_result";

/// A suggestion for autocomplete.
#[derive(Debug, Clone)]
pub struct CompletionSuggestion {
    /// The suggestion string
    pub suggestion: String,
    /// The character indices of the suggestion to highlight.
    pub highlighted_indices: Vec<usize>,
}

/// Resource holding current autocomplete suggestions.
#[derive(Resource, Default, Deref, DerefMut)]
pub struct AutoCompletions(pub Vec<CompletionSuggestion>);

/// Log level filter settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogFilter {
    pub show_error: bool,
    pub show_warn: bool,
    pub show_info: bool,
    pub show_debug: bool,
    pub show_trace: bool,
}

impl Default for LogFilter {
    fn default() -> Self {
        Self {
            show_error: true,
            show_warn: true,
            show_info: true,
            show_debug: true,
            show_trace: true,
        }
    }
}

impl LogFilter {
    /// Check if a log level should be shown.
    pub fn should_show(&self, level: bevy::log::Level) -> bool {
        use bevy::log::Level;
        match level {
            Level::ERROR => self.show_error,
            Level::WARN => self.show_warn,
            Level::INFO => self.show_info,
            Level::DEBUG => self.show_debug,
            Level::TRACE => self.show_trace,
        }
    }
}

#[derive(Default, Resource)]
pub struct ConsoleUiState {
    /// Whether the console is open or not.
    pub(crate) open: bool,
    /// Whether we have set focus this open or not.
    pub(crate) text_focus: bool,
    /// A list of all log messages received plus an
    /// indicator indicating if the message is new.
    pub(crate) log: Vec<(LogMessage, bool)>,
    /// The command in the text bar.
    pub(crate) command: String,
    /// The selected completion index.
    pub(crate) selected_completion: usize,
    /// Last command text that was used for autocomplete.
    pub(crate) last_autocomplete_text: String,
    /// Command history.
    pub(crate) history: Vec<String>,
    /// Current position in history (0 = current input, 1+ = history).
    pub(crate) history_index: usize,
    /// Saved current input when navigating history.
    pub(crate) history_draft: String,
    /// Log level filter.
    pub(crate) log_filter: LogFilter,
}

impl ConsoleUiState {
    /// Whether the console is currently open or not
    pub fn open(&self) -> bool {
        self.open
    }
}

/// Format a SystemTime as HH:MM string.
fn format_time(t: SystemTime) -> String {
    let duration = t.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
    let secs = duration.as_secs();
    let hours = (secs / 3600) % 24;
    let minutes = (secs / 60) % 60;
    format!("{:02}:{:02} ", hours, minutes)
}

pub(crate) fn read_logs(logs: Option<MessageReader<LogMessage>>, mut state: ResMut<ConsoleUiState>) {
    let Some(mut logs) = logs else { return };
    for log_message in logs.read() {
        state.log.push((log_message.clone(), true));
    }
}

pub(crate) fn handle_clear(
    mut clear_events: MessageReader<crate::core::ConsoleClearEvent>,
    mut state: ResMut<ConsoleUiState>,
) {
    for _ in clear_events.read() {
        state.log.clear();
    }
}

pub(crate) fn open_close_ui(
    mut state: ResMut<ConsoleUiState>,
    key: Res<ButtonInput<KeyCode>>,
    config: Res<ConsoleConfig>,
) {
    if key.just_pressed(config.open_key) {
        state.open = !state.open;
        state.text_focus = false;
    }
}

/// System that updates autocomplete suggestions based on current input.
pub(crate) fn update_completions(
    mut state: ResMut<ConsoleUiState>,
    mut completions: ResMut<AutoCompletions>,
    registry: Res<ConsoleRegistry>,
) {
    // Only update if the command text changed
    if state.command == state.last_autocomplete_text {
        return;
    }
    state.last_autocomplete_text = state.command.clone();

    // Get the keyword being typed (last word)
    let keyword = state.command.split_whitespace().last().unwrap_or("");

    if keyword.is_empty() {
        completions.0.clear();
        return;
    }

    // Use our fuzzy matcher to find matches
    let matches = registry.fuzzy_find(keyword);

    completions.0 = matches
        .into_iter()
        .take(MAX_COMPLETION_SUGGESTIONS)
        .map(|(name, _, result)| CompletionSuggestion {
            suggestion: name.to_string(),
            highlighted_indices: result.indices,
        })
        .collect();
}

pub(crate) fn render_ui_system(
    mut contexts: EguiContexts,
    mut state: ResMut<ConsoleUiState>,
    key: Res<ButtonInput<KeyCode>>,
    config: Res<ConsoleConfig>,
    completions: Res<AutoCompletions>,
    mut input_events: MessageWriter<ConsoleInputEvent>,
) -> Result<(), BevyError> {
    egui::Window::new("Developer Console")
        .collapsible(false)
        .default_width(900.)
        .show(contexts.ctx_mut()?, |ui| {
            render_ui(
                ui,
                &mut state,
                &key,
                &config,
                &completions,
                &mut input_events,
            )
        });
    Ok(())
}

/// The function that renders the UI of the developer console.
pub fn render_ui(
    ui: &mut egui::Ui,
    state: &mut ConsoleUiState,
    key: &ButtonInput<KeyCode>,
    config: &ConsoleConfig,
    completions: &AutoCompletions,
    input_events: &mut MessageWriter<ConsoleInputEvent>,
) {
    fn submit_command(state: &mut ConsoleUiState, input_events: &mut MessageWriter<ConsoleInputEvent>) {
        let command = state.command.trim();
        if !command.is_empty() {
            info!(name: COMMAND_MESSAGE_NAME, "{COMMAND_MESSAGE_PREFIX}{}", command);

            // Add to history (avoid duplicates at the top)
            if state.history.first().map(|s| s.as_str()) != Some(command) {
                state.history.insert(0, command.to_string());
            }

            let cmd = std::mem::take(&mut state.command);
            input_events.write(ConsoleInputEvent::new(cmd));

            // Reset history navigation
            state.history_index = 0;
            state.history_draft.clear();
        }
    }

    if key.just_pressed(config.submit_key) {
        submit_command(state, input_events);
    }

    // History navigation with up/down arrows (only when completions popup is closed)
    if completions.is_empty() {
        if key.just_pressed(KeyCode::ArrowUp) && !state.history.is_empty() {
            if state.history_index == 0 {
                // Save current input before navigating
                state.history_draft = state.command.clone();
            }
            if state.history_index < state.history.len() {
                state.history_index += 1;
                state.command = state.history[state.history_index - 1].clone();
            }
        }
        if key.just_pressed(KeyCode::ArrowDown) {
            if state.history_index > 0 {
                state.history_index -= 1;
                if state.history_index == 0 {
                    state.command = std::mem::take(&mut state.history_draft);
                } else {
                    state.command = state.history[state.history_index - 1].clone();
                }
            }
        }
    }

    completions::change_selected_completion(ui, state, &completions);

    // Log filter controls
    egui::TopBottomPanel::top("filter panel")
        .frame(egui::Frame::NONE.outer_margin(egui::Margin::symmetric(5, 2)))
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Filter:");
                ui.checkbox(&mut state.log_filter.show_error, "Error");
                ui.checkbox(&mut state.log_filter.show_warn, "Warn");
                ui.checkbox(&mut state.log_filter.show_info, "Info");
                ui.checkbox(&mut state.log_filter.show_debug, "Debug");
                ui.checkbox(&mut state.log_filter.show_trace, "Trace");
            });
        });

    egui::TopBottomPanel::bottom("bottom panel")
        .frame(egui::Frame::NONE.outer_margin(egui::Margin {
            left: 5,
            right: 5,
            top: 11,
            bottom: 5,
        }))
        .show_inside(ui, |ui| {
            let text_edit_id = egui::Id::new("text_edit");

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Submit").clicked() {
                    submit_command(state, input_events);
                    ui.ctx().memory_mut(|mem| mem.request_focus(text_edit_id));
                }

                let text_edit = egui::TextEdit::singleline(&mut state.command)
                    .id(text_edit_id)
                    .desired_width(ui.available_width())
                    .margin(egui::Vec2::splat(4.0))
                    .font(config.theme.font.clone())
                    .lock_focus(true)
                    .show(ui);

                completions::completions(
                    text_edit,
                    text_edit_id,
                    state,
                    ui,
                    &completions,
                    config,
                );

                if !state.text_focus {
                    state.text_focus = true;
                    ui.ctx().memory_mut(|mem| mem.request_focus(text_edit_id));
                }
            });
        });

    egui::ScrollArea::new([false, true])
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.vertical(|ui| {
                for (id, (message, is_new)) in state.log.iter_mut().enumerate() {
                    // Apply log filter (always show command messages)
                    if message.name != COMMAND_MESSAGE_NAME
                        && message.name != COMMAND_RESULT_NAME
                        && !state.log_filter.should_show(message.level)
                    {
                        continue;
                    }
                    add_log(ui, id, message, is_new, config);
                }
            });
        });
}

fn add_log(
    ui: &mut egui::Ui,
    id: usize,
    event: &LogMessage,
    is_new: &mut bool,
    config: &ConsoleConfig,
) {
    ui.push_id(id, |ui| {
        let time_str = format_time(event.time);

        let text = format_line(&time_str, config, event);
        let label = ui.label(text);

        if *is_new {
            label.scroll_to_me(Some(egui::Align::Max));
            *is_new = false;
        }

        // Copy message to clipboard on click
        if label.clicked() {
            ui.ctx().copy_text(event.message.clone());
        }

        label.on_hover_ui(|ui| {
            ui.label("Click to copy message");
            ui.separator();

            let mut text = LayoutJob::default();
            text.append("Name: ", 0.0, config.theme.format_text());
            text.append(event.name, 0.0, config.theme.format_dark());
            text.append("\nTarget: ", 0.0, config.theme.format_text());
            text.append(event.target, 0.0, config.theme.format_dark());
            text.append("\nModule Path: ", 0.0, config.theme.format_text());
            if let Some(module_path) = event.module_path {
                text.append(module_path, 0.0, config.theme.format_dark());
            } else {
                text.append("(Unknown)", 0.0, config.theme.format_dark());
            }
            text.append("\nFile: ", 0.0, config.theme.format_text());
            if let (Some(file), Some(line)) = (event.file, event.line) {
                text.append(&format!("{file}:{line}"), 0.0, config.theme.format_dark());
            } else {
                text.append("(Unknown)", 0.0, config.theme.format_dark());
            }

            ui.label(text);
        });
    });
}

fn format_line(
    time_str: &str,
    config: &ConsoleConfig,
    LogMessage {
        message,
        name,
        level,
        ..
    }: &LogMessage,
) -> LayoutJob {
    let mut text = LayoutJob::default();
    text.append(
        time_str,
        0.0,
        config.theme.format_dark(),
    );

    match *name {
        COMMAND_MESSAGE_NAME => {
            let message_stripped = message
                .strip_prefix(COMMAND_MESSAGE_PREFIX)
                .unwrap_or(message);
            text.append(COMMAND_MESSAGE_PREFIX, 0.0, config.theme.format_dark());
            text.append(message_stripped, 0.0, config.theme.format_text());
            text
        }
        COMMAND_RESULT_NAME => {
            text.append(COMMAND_RESULT_PREFIX, 0.0, config.theme.format_dark());
            text.append(
                message
                    .strip_prefix(COMMAND_RESULT_PREFIX)
                    .unwrap_or(message),
                0.0,
                config.theme.format_text(),
            );
            text
        }
        _ => {
            text.append(level.as_str(), 0.0, config.theme.format_level(*level));
            text.append(&format!(" {message}"), 0.0, config.theme.format_text());
            text
        }
    }
}
