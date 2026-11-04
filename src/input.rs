#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    PlayPause,
    Prev,
    Next,
    SeekBack,
    SeekForward,
    VolumeUp,
    VolumeDown,
    FocusSearch,
    Confirm,
    Quit,
    ToggleView,
    Like,
    ScrollUp,
    ScrollDown,
}

pub fn action_for(key: egui::Key, shift: bool) -> Option<Action> {
    match key {
        egui::Key::Space => Some(Action::PlayPause),
        egui::Key::ArrowRight if shift => Some(Action::SeekForward),
        egui::Key::ArrowRight => Some(Action::Next),
        egui::Key::ArrowLeft if shift => Some(Action::SeekBack),
        egui::Key::ArrowLeft => Some(Action::Prev),
        egui::Key::ArrowUp => Some(Action::VolumeUp),
        egui::Key::ArrowDown => Some(Action::VolumeDown),
        egui::Key::Q => Some(Action::Quit),
        egui::Key::Slash => Some(Action::FocusSearch),
        egui::Key::Enter => Some(Action::Confirm),
        egui::Key::Tab => Some(Action::ToggleView),
        egui::Key::PageUp => Some(Action::ScrollUp),
        egui::Key::PageDown => Some(Action::ScrollDown),
        egui::Key::F => Some(Action::Like),
        egui::Key::Escape => None,
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_toggles() {
        assert_eq!(
            action_for(egui::Key::Space, false),
            Some(Action::PlayPause)
        );
    }

    #[test]
    fn arrows_change_track_and_shift_seeks() {
        assert_eq!(
            action_for(egui::Key::ArrowRight, false),
            Some(Action::Next)
        );
        assert_eq!(
            action_for(egui::Key::ArrowRight, true),
            Some(Action::SeekForward)
        );
        assert_eq!(
            action_for(egui::Key::ArrowUp, false),
            Some(Action::VolumeUp)
        );
    }

    #[test]
    fn letters_are_mapped() {
        assert_eq!(action_for(egui::Key::Q, false), Some(Action::Quit));
        assert_eq!(
            action_for(egui::Key::Slash, false),
            Some(Action::FocusSearch)
        );
        assert_eq!(action_for(egui::Key::A, false), None);
        assert_eq!(action_for(egui::Key::Tab, false), Some(Action::ToggleView));
        assert_eq!(action_for(egui::Key::F, false), Some(Action::Like));
        assert_eq!(action_for(egui::Key::PageDown, false), Some(Action::ScrollDown));
    }
}
