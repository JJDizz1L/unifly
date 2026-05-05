use super::{AuthMode, OnboardingScreen, WizardStep};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::action::Action;

impl OnboardingScreen {
    pub(super) fn handle_key_input(&mut self, key: KeyEvent) -> Option<Action> {
        if key.code != KeyCode::Enter {
            self.error = None;
        }

        match self.step {
            WizardStep::Welcome => {
                if key.code == KeyCode::Enter {
                    self.advance();
                }
            }
            WizardStep::Url | WizardStep::Site => self.handle_text_step_key(key),
            WizardStep::AuthMode => self.handle_auth_mode_key(key),
            WizardStep::Credentials => self.handle_credentials_key(key),
            WizardStep::Testing => {
                if key.code == KeyCode::Esc {
                    self.go_back();
                }
            }
            WizardStep::Done => match key.code {
                KeyCode::Enter => self.send_completion(),
                KeyCode::Esc => self.go_back(),
                _ => {}
            },
        }

        None
    }

    pub(super) fn apply_action(&mut self, action: &Action) {
        match action {
            Action::OnboardingTestResult(result) => {
                self.testing = false;
                match result {
                    Ok(()) => {
                        self.test_error = None;
                        self.step = WizardStep::Done;
                    }
                    Err(message) => {
                        self.test_error = Some(message.clone());
                    }
                }
            }
            Action::Tick if self.testing => {
                self.throbber_state.calc_next();
            }
            _ => {}
        }
    }

    fn handle_text_step_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => self.advance(),
            KeyCode::Esc => self.go_back(),
            KeyCode::Backspace => {
                if let Some(input) = self.active_input_mut() {
                    input.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(input) = self.active_input_mut() {
                    input.push(c);
                }
            }
            _ => {}
        }
    }

    fn handle_auth_mode_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') if self.auth_mode_index > 0 => {
                self.auth_mode_index -= 1;
            }
            KeyCode::Down | KeyCode::Char('j')
                if self.auth_mode_index < AuthMode::ALL.len() - 1 =>
            {
                self.auth_mode_index += 1;
            }
            KeyCode::Enter => self.advance(),
            KeyCode::Esc => self.go_back(),
            _ => {}
        }
    }

    fn handle_credentials_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Tab => self.next_cred_field(),
            KeyCode::Enter => self.advance(),
            KeyCode::Esc => self.go_back(),
            KeyCode::Backspace => {
                if let Some(input) = self.active_input_mut() {
                    input.pop();
                }
            }
            KeyCode::Char(c) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'u' {
                    self.show_password = !self.show_password;
                } else if let Some(input) = self.active_input_mut() {
                    input.push(c);
                }
            }
            _ => {}
        }
    }
}
