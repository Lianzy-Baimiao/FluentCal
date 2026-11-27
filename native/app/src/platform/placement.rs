#[derive(Default)]
pub(super) struct UserPlacement {
    generation: u64,
    active: bool,
    restore_pending: bool,
}

impl UserPlacement {
    pub(super) fn position_notice_due(&self) -> bool { !self.active }

    pub(super) fn begin(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.active = true;
    }

    pub(super) fn display_due(&mut self) -> bool {
        if self.active {
            self.restore_pending = true;
            return false;
        }
        true
    }

    pub(super) fn end_message(&mut self) -> u64 {
        if !self.active { self.begin(); }
        self.generation
    }

    pub(super) fn finish(&mut self, generation: u64) -> Option<bool> {
        if !self.active || self.generation != generation { return None; }
        self.active = false;
        Some(std::mem::take(&mut self.restore_pending))
    }

    pub(super) fn reset(&mut self) {
        *self = Self { generation: self.generation.wrapping_add(1), ..Self::default() };
    }
}

#[cfg(test)]
mod tests {
    use super::UserPlacement;
    use fluentcal_core::layout::{target, Home, Mon, Rect};

    #[test]
    fn a_native_gesture_does_not_schedule_monitor_enumeration_per_position() {
        let mut placement = UserPlacement::default();
        assert!(placement.position_notice_due());
        placement.begin();
        let notices = (0..240).filter(|_| placement.position_notice_due()).count();
        assert_eq!(notices, 0, "ordinary drag and resize ticks must not queue monitor scans");
        let end = placement.end_message();
        assert!(!placement.position_notice_due());
        assert_eq!(placement.finish(end), Some(false));
        assert!(placement.position_notice_due());
    }

    #[test]
    fn delayed_display_restore_does_not_pull_a_drag_back_home() {
        let screen = Rect { x: 0, y: 0, w: 1920, h: 1080 };
        let monitors = [Mon { name: "fixture".into(), rect: screen, work: screen, primary: true, scale: 1.0 }];
        let home = Home { x: 80, y: 80, w: 1000, h: 720, monitor: None };
        let dragged = Rect { x: 200, y: 110, ..home.rect() };
        let mut position = dragged;
        let mut placement = UserPlacement::default();
        assert_eq!(target(&monitors, Some(&home), Some(position)), Some(home.rect()));
        placement.begin();
        for _ in 0..2 {
            if placement.display_due() {
                position = target(&monitors, Some(&home), Some(position)).unwrap();
            }
            assert_eq!(position, dragged, "a delayed display check interrupted the gesture");
        }
        let end = placement.end_message();
        assert!(!placement.display_due(), "the end message is queued but the new home is not yet saved");
        assert_eq!(placement.finish(end), Some(true));
        let saved = Home::from_user_rect(&monitors, position).unwrap();
        assert!(placement.display_due());
        assert_eq!(target(&monitors, Some(&saved), Some(position)), Some(dragged));
    }

    #[test]
    fn stale_end_does_not_finish_a_new_gesture_or_recreated_window() {
        let mut placement = UserPlacement::default();
        placement.begin();
        let first = placement.end_message();
        assert!(!placement.display_due());
        placement.begin();
        assert_eq!(placement.finish(first), None);
        assert_eq!(placement.finish(placement.generation), Some(true));
        placement.begin();
        let before_recreation = placement.end_message();
        assert!(!placement.display_due());
        placement.reset();
        assert_eq!(placement.finish(before_recreation), None);
        assert!(placement.display_due());
        let end_without_begin = placement.end_message();
        assert_eq!(placement.finish(end_without_begin), Some(false));
    }
}
