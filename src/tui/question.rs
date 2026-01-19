use crate::agent::Question;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, BorderType, List, ListItem, Paragraph, Wrap},
    Frame,
};
use std::collections::HashMap;

/// Question widget state for interactive user questions
pub struct QuestionWidget {
    pub tool_use_id: String,
    questions: Vec<Question>,
    current_question_index: usize,
    selected_option_index: usize,
    selected_options: HashMap<usize, Vec<usize>>, // question_index -> [option_indices]
    custom_input_mode: bool,
    custom_input: String,
}

impl QuestionWidget {
    /// Create a new question widget
    pub fn new(tool_use_id: String, questions: Vec<Question>) -> Self {
        Self {
            tool_use_id,
            questions,
            current_question_index: 0,
            selected_option_index: 0,
            selected_options: HashMap::new(),
            custom_input_mode: false,
            custom_input: String::new(),
        }
    }

    /// Get current question
    fn current_question(&self) -> &Question {
        &self.questions[self.current_question_index]
    }

    /// Check if current question is multi-select
    fn is_multi_select(&self) -> bool {
        self.current_question().multi_select
    }

    /// Get number of options (including "Other")
    fn num_options(&self) -> usize {
        self.current_question().options.len() + 1 // +1 for "Other"
    }

    /// Check if "Other" option is currently selected
    fn is_other_selected(&self) -> bool {
        self.selected_option_index == self.num_options() - 1
    }

    /// Toggle selection for current option (multi-select only)
    fn toggle_selection(&mut self) {
        if !self.is_multi_select() {
            return;
        }

        let selections = self
            .selected_options
            .entry(self.current_question_index)
            .or_insert_with(Vec::new);

        if let Some(pos) = selections.iter().position(|&x| x == self.selected_option_index) {
            selections.remove(pos);
        } else {
            selections.push(self.selected_option_index);
        }
    }

    /// Check if an option is selected
    fn is_option_selected(&self, option_index: usize) -> bool {
        if let Some(selections) = self.selected_options.get(&self.current_question_index) {
            selections.contains(&option_index)
        } else {
            false
        }
    }

    /// Handle keyboard input
    pub fn handle_key(&mut self, key: KeyEvent) -> QuestionWidgetAction {
        if self.custom_input_mode {
            // Custom input mode - handle text input
            match key.code {
                KeyCode::Esc => {
                    self.custom_input_mode = false;
                    self.custom_input.clear();
                    QuestionWidgetAction::Continue
                }
                KeyCode::Enter => {
                    if !self.custom_input.is_empty() {
                        let answer = self.custom_input.clone();
                        self.custom_input.clear();
                        self.custom_input_mode = false;

                        // Record custom answer
                        self.selected_options.insert(
                            self.current_question_index,
                            vec![self.num_options() - 1], // "Other" option index
                        );

                        // Move to next question or submit
                        if self.current_question_index + 1 < self.questions.len() {
                            self.current_question_index += 1;
                            self.selected_option_index = 0;
                            QuestionWidgetAction::Continue
                        } else {
                            QuestionWidgetAction::Submit(self.collect_answers_with_custom(&answer))
                        }
                    } else {
                        QuestionWidgetAction::Continue
                    }
                }
                KeyCode::Char(c) => {
                    self.custom_input.push(c);
                    QuestionWidgetAction::Continue
                }
                KeyCode::Backspace => {
                    self.custom_input.pop();
                    QuestionWidgetAction::Continue
                }
                _ => QuestionWidgetAction::Continue,
            }
        } else {
            // Normal selection mode
            match key.code {
                KeyCode::Up => {
                    if self.selected_option_index > 0 {
                        self.selected_option_index -= 1;
                    }
                    QuestionWidgetAction::Continue
                }
                KeyCode::Down => {
                    if self.selected_option_index < self.num_options() - 1 {
                        self.selected_option_index += 1;
                    }
                    QuestionWidgetAction::Continue
                }
                KeyCode::Char(' ') if self.is_multi_select() => {
                    self.toggle_selection();
                    QuestionWidgetAction::Continue
                }
                KeyCode::Enter => {
                    // Check if "Other" is selected
                    if self.is_other_selected() {
                        self.custom_input_mode = true;
                        QuestionWidgetAction::Continue
                    } else {
                        // Record selection
                        if self.is_multi_select() {
                            // For multi-select, use already toggled selections
                            if !self.is_option_selected(self.selected_option_index) {
                                self.toggle_selection();
                            }
                        } else {
                            // For single-select, just record current selection
                            self.selected_options.insert(
                                self.current_question_index,
                                vec![self.selected_option_index],
                            );
                        }

                        // Move to next question or submit
                        if self.current_question_index + 1 < self.questions.len() {
                            self.current_question_index += 1;
                            self.selected_option_index = 0;
                            QuestionWidgetAction::Continue
                        } else {
                            QuestionWidgetAction::Submit(self.collect_answers())
                        }
                    }
                }
                KeyCode::Esc => QuestionWidgetAction::Cancel,
                _ => QuestionWidgetAction::Continue,
            }
        }
    }

    /// Collect answers from selections
    fn collect_answers(&self) -> HashMap<String, String> {
        let mut answers = HashMap::new();

        for (q_idx, option_indices) in &self.selected_options {{
                let question = &self.questions[*q_idx];
                let selected_labels: Vec<String> = option_indices
                    .iter()
                    .filter_map(|&opt_idx| {
                        if opt_idx < question.options.len() {
                            Some(question.options[opt_idx].label.clone())
                        } else {
                            Some("Other".to_string())
                        }
                    })
                    .collect();

                let answer_value = if selected_labels.len() == 1 {
                    selected_labels[0].clone()
                } else {
                    selected_labels.join(", ")
                };

                answers.insert(format!("q{}", q_idx), answer_value);
            }
        }

        answers
    }

    /// Collect answers with custom text for "Other" option
    fn collect_answers_with_custom(&self, custom_text: &str) -> HashMap<String, String> {
        let mut answers = self.collect_answers();
        answers.insert(
            format!("q{}", self.current_question_index),
            format!("Other: {}", custom_text),
        );
        answers
    }

    /// Render the question widget
    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();

        // Create centered dialog
        let dialog_width = 80.min(area.width.saturating_sub(4));
        let dialog_height = 25.min(area.height.saturating_sub(4));

        let dialog_area = Rect {
            x: (area.width.saturating_sub(dialog_width)) / 2,
            y: (area.height.saturating_sub(dialog_height)) / 2,
            width: dialog_width,
            height: dialog_height,
        };

        // Clear background
        frame.render_widget(
            Block::default().style(Style::default().bg(Color::Black)),
            area,
        );

        // Main dialog block
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(Span::styled(
                format!(
                    " Question {}/{} ",
                    self.current_question_index + 1,
                    self.questions.len()
                ),
                Style::default()
                    .fg(Color::LightBlue)
                    .add_modifier(Modifier::BOLD),
            ))
            .border_style(Style::default().fg(Color::Cyan));

        frame.render_widget(block.clone(), dialog_area);

        // Split into question, options, and help sections
        let inner = block.inner(dialog_area);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4), // Question
                Constraint::Min(10),   // Options
                Constraint::Length(3), // Help text
            ])
            .split(inner);

        // Render question
        self.render_question(frame, chunks[0]);

        // Render options or custom input
        if self.custom_input_mode {
            self.render_custom_input(frame, chunks[1]);
        } else {
            self.render_options(frame, chunks[1]);
        }

        // Render help text
        self.render_help(frame, chunks[2]);
    }

    /// Render question text
    fn render_question(&self, frame: &mut Frame, area: Rect) {
        let question = self.current_question();
        let text = vec![
            Line::from(Span::styled(
                question.header.as_str(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(question.question.as_str()),
        ];

        let paragraph = Paragraph::new(text)
            .wrap(Wrap { trim: true })
            .block(Block::default());

        frame.render_widget(paragraph, area);
    }

    /// Render options list
    fn render_options(&self, frame: &mut Frame, area: Rect) {
        let question = self.current_question();
        let mut items = Vec::new();

        // Add regular options
        for (idx, option) in question.options.iter().enumerate() {
            let is_selected = idx == self.selected_option_index;
            let is_checked = self.is_option_selected(idx);

            let checkbox = if self.is_multi_select() {
                if is_checked {
                    "[✓] "
                } else {
                    "[ ] "
                }
            } else {
                if is_selected {
                    "(*) "
                } else {
                    "( ) "
                }
            };

            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if is_checked {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };

            items.push(ListItem::new(vec![
                Line::from(Span::styled(
                    format!("{}{}", checkbox, option.label),
                    style,
                )),
                Line::from(Span::styled(
                    format!("    {}", option.description),
                    Style::default().fg(Color::DarkGray),
                )),
            ]));
        }

        // Add "Other" option
        let other_idx = self.num_options() - 1;
        let is_other_selected = other_idx == self.selected_option_index;
        let checkbox = if self.is_multi_select() {
            if self.is_option_selected(other_idx) {
                "[✓] "
            } else {
                "[ ] "
            }
        } else {
            if is_other_selected {
                "(*) "
            } else {
                "( ) "
            }
        };

        let style = if is_other_selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        items.push(ListItem::new(vec![
            Line::from(Span::styled(format!("{}Other", checkbox), style)),
            Line::from(Span::styled(
                "    Provide custom text input",
                Style::default().fg(Color::DarkGray),
            )),
        ]));

        let list = List::new(items).block(Block::default());

        frame.render_widget(list, area);
    }

    /// Render custom input field
    fn render_custom_input(&self, frame: &mut Frame, area: Rect) {
        let text = vec![
            Line::from(Span::styled(
                "Enter your custom answer:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                format!("> {}_", self.custom_input),
                Style::default().fg(Color::Cyan),
            )),
        ];

        let paragraph = Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .block(Block::default());

        frame.render_widget(paragraph, area);
    }

    /// Render help text
    fn render_help(&self, frame: &mut Frame, area: Rect) {
        let help_text = if self.custom_input_mode {
            "Enter=submit │ Esc=cancel"
        } else if self.is_multi_select() {
            "↑↓=navigate │ Space=toggle │ Enter=confirm │ Esc=cancel"
        } else {
            "↑↓=navigate │ Enter=select │ Esc=cancel"
        };

        let paragraph = Paragraph::new(Line::from(Span::styled(
            help_text,
            Style::default().fg(Color::DarkGray),
        )))
        .block(Block::default());

        frame.render_widget(paragraph, area);
    }
}

/// Actions returned by question widget
#[derive(Debug)]
pub enum QuestionWidgetAction {
    /// Continue showing the widget
    Continue,
    /// User submitted answers
    Submit(HashMap<String, String>),
    /// User cancelled
    Cancel,
}
