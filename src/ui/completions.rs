//! Autocomplete UI widget.

use bevy_egui::egui;

use crate::config::ConsoleConfig;

use super::{AutoCompletions, CompletionSuggestion, ConsoleUiState};

/// The max amount of completion suggestions shown at once.
pub const MAX_COMPLETION_SUGGESTIONS: usize = 6;

pub fn completions(
    text_edit: egui::text_edit::TextEditOutput,
    text_edit_id: egui::Id,
    state: &mut ConsoleUiState,
    ui: &mut egui::Ui,
    completions: &AutoCompletions,
    config: &ConsoleConfig,
) {
    let text_edit_complete_id = ui.make_persistent_id("text_edit_complete");

    if let Some(cursor_range) = text_edit.state.cursor.char_range() {
        let [primary, secondary] = cursor_range.sorted_cursors();

        fn non_keyword(character: char) -> bool {
            !(character.is_alphanumeric() || character == '_')
        }

        let cursor_index = (|| {
            let (primary_index, char) = state
                .command
                .char_indices()
                .nth(primary.index.saturating_sub(1))?;

            if non_keyword(char) {
                return None;
            }

            Some(primary_index)
        })();

        if text_edit.response.changed() {
            state.selected_completion = 0;
        }

        if cursor_index.is_some() {
            if !completions.is_empty() {
                egui::Popup::open_id(ui.ctx(), text_edit_complete_id);
            }
        } else if egui::Popup::is_id_open(ui.ctx(), text_edit_complete_id) {
            egui::Popup::close_id(ui.ctx(), text_edit_complete_id);
        }

        if let Some(cursor_index) = cursor_index {
            // Accept completion with Tab or ArrowRight (when popup is open)
            let accept_completion = ui.input(|i| i.key_pressed(egui::Key::Tab))
                || (!completions.is_empty() && ui.input(|i| i.key_pressed(egui::Key::ArrowRight)));

            if accept_completion {
                // Remove the old text
                let before_cursor = &state.command[..=cursor_index];
                let index_before = match before_cursor.rfind(non_keyword) {
                    Some(index) => index + 1,
                    None => 0,
                };
                let after_cursor = &state.command[cursor_index..];
                match after_cursor.find(non_keyword) {
                    Some(characters_after) => state
                        .command
                        .drain(index_before..cursor_index + characters_after),
                    None => state.command.drain(index_before..),
                };

                // Add the completed text
                if let Some(completion) = completions.0.get(state.selected_completion) {
                    let completed_text = &completion.suggestion;
                    state.command.insert_str(index_before, completed_text);

                    // Set the cursor position
                    let mut text_edit_state = text_edit.state;
                    let mut cursor_range = egui::text::CCursorRange::two(primary, secondary);

                    cursor_range.primary.index +=
                        completed_text.len() - (cursor_index - index_before) - 1;
                    cursor_range.secondary.index +=
                        completed_text.len() - (cursor_index - index_before) - 1;

                    text_edit_state.cursor.set_char_range(Some(cursor_range));
                    egui::TextEdit::store_state(ui.ctx(), text_edit_id, text_edit_state);
                }
            }
        }
    }

    egui::Popup::from_response(&text_edit.response)
        .id(text_edit_complete_id)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .align(egui::RectAlign::TOP_START)
        .show(|ui| {
            ui.vertical(|ui| {
                for (
                    i,
                    CompletionSuggestion {
                        suggestion,
                        highlighted_indices,
                    },
                ) in completions.iter().take(MAX_COMPLETION_SUGGESTIONS).enumerate()
                {
                    let mut layout = egui::text::LayoutJob::default();
                    for (i, _) in suggestion.char_indices() {
                        layout.append(
                            &suggestion[i..=i],
                            0.0,
                            if highlighted_indices.contains(&i) {
                                config.theme.format_bold()
                            } else {
                                config.theme.format_text()
                            },
                        );
                    }
                    let res = ui.label(layout);
                    if i == state.selected_completion {
                        res.highlight();
                    }
                }
            })
        });
}

/// Also consumes the up and down arrow keys.
pub fn change_selected_completion(
    ui: &mut egui::Ui,
    state: &mut ConsoleUiState,
    completions: &[CompletionSuggestion],
) {
    if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)) {
        state.selected_completion = state.selected_completion.saturating_sub(1);
    }
    if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)) {
        state.selected_completion = state
            .selected_completion
            .saturating_add(1)
            .min(completions.len().saturating_sub(1));
    }
}
