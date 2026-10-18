use crate::wake::{self, Note};
use block2::RcBlock;
use objc2_app_kit::{NSEvent, NSEventMask, NSEventPhase};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};

const SWIPE_PIXELS: f64 = 80.0;

static INSTALLED: AtomicBool = AtomicBool::new(false);
static GESTURE: std::sync::Mutex<Gesture> = std::sync::Mutex::new(Gesture {
    travel: 0.0,
    used: false,
});

pub fn install() {
    if INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }
    let block = RcBlock::new(|event: NonNull<NSEvent>| -> *mut NSEvent {
        let event_ref = unsafe { event.as_ref() };
        watch(event_ref);
        event.as_ptr()
    });
    let monitor = unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::ScrollWheel, &block)
    };
    if monitor.is_none() {
        tracing::error!("could not watch trackpad swipes");
    }
    std::mem::forget(block);
    if let Some(monitor) = monitor {
        std::mem::forget(monitor);
    }
}

fn watch(event: &NSEvent) {
    let sample = Sample {
        precise: event.hasPreciseScrollingDeltas(),
        momentum: !event.momentumPhase().is_empty(),
        began: event.phase().contains(NSEventPhase::Began),
        moved: event.phase().contains(NSEventPhase::Changed)
            || event.phase().contains(NSEventPhase::Stationary),
        ended: event.phase().contains(NSEventPhase::Ended)
            || event.phase().contains(NSEventPhase::Cancelled),
        delta_x: -event.scrollingDeltaX(),
        delta_y: event.scrollingDeltaY(),
    };
    let mut gesture = wake::lock(&GESTURE);
    let Some(direction) = gesture.observe(sample) else {
        return;
    };
    drop(gesture);
    wake::post(Note::Swipe(direction));
}

struct Gesture {
    travel: f64,
    used: bool,
}

#[derive(Clone, Copy)]
struct Sample {
    precise: bool,
    momentum: bool,
    began: bool,
    moved: bool,
    ended: bool,
    delta_x: f64,
    delta_y: f64,
}

impl Gesture {
    fn observe(&mut self, sample: Sample) -> Option<i8> {
        if !sample.delta_x.is_finite() || !sample.delta_y.is_finite() {
            return None;
        }
        if !sample.precise {
            return direction(sample.delta_x);
        }
        if sample.momentum {
            return None;
        }
        if sample.began {
            self.travel = 0.0;
            self.used = false;
        }
        let tracking = sample.began || sample.moved || sample.ended;
        if !tracking || sample.delta_x.abs() <= sample.delta_y.abs() {
            if sample.ended {
                self.travel = 0.0;
                self.used = false;
            }
            return None;
        }
        let mut fired = None;
        if !self.used {
            self.travel += sample.delta_x;
            if self.travel.abs() >= SWIPE_PIXELS {
                self.used = true;
                fired = direction(self.travel);
            }
        }
        if sample.ended {
            self.travel = 0.0;
            self.used = false;
        }
        fired
    }
}

fn direction(delta: f64) -> Option<i8> {
    if delta > 0.0 {
        Some(1)
    } else if delta < 0.0 {
        Some(-1)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finger(delta_x: f64, began: bool, ended: bool) -> Sample {
        Sample {
            precise: true,
            momentum: false,
            began,
            moved: !began && !ended,
            ended,
            delta_x,
            delta_y: 0.0,
        }
    }

    #[test]
    fn one_gesture_moves_once_and_the_coast_does_not() {
        let mut gesture = Gesture {
            travel: 0.0,
            used: false,
        };
        assert_eq!(gesture.observe(finger(40.0, true, false)), None);
        assert_eq!(gesture.observe(finger(50.0, false, false)), Some(1));
        assert_eq!(gesture.observe(finger(90.0, false, false)), None);
        assert_eq!(
            gesture.observe(Sample {
                precise: true,
                momentum: true,
                began: false,
                moved: false,
                ended: false,
                delta_x: 200.0,
                delta_y: 0.0,
            }),
            None
        );
        assert_eq!(gesture.observe(finger(40.0, false, true)), None);

        assert_eq!(gesture.observe(finger(-90.0, true, false)), Some(-1));
        assert_eq!(gesture.observe(finger(-80.0, false, true)), None);
        assert_eq!(gesture.observe(finger(90.0, true, false)), Some(1));
    }

    #[test]
    fn a_vertical_gesture_does_not_move_and_a_notch_does() {
        let mut gesture = Gesture {
            travel: 0.0,
            used: false,
        };
        assert_eq!(
            gesture.observe(Sample {
                precise: true,
                momentum: false,
                began: true,
                moved: false,
                ended: false,
                delta_x: 30.0,
                delta_y: -80.0,
            }),
            None
        );
        assert_eq!(
            gesture.observe(Sample {
                precise: false,
                momentum: false,
                began: false,
                moved: false,
                ended: false,
                delta_x: -1.0,
                delta_y: 0.0,
            }),
            Some(-1)
        );
    }
}
