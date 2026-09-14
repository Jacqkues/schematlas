//! Coalesce high-frequency input to one model update per animation frame.
use leptos::prelude::*;

#[derive(Clone, Copy)]
pub struct FrameValue<T: Copy + Send + Sync + 'static> {
    pending: StoredValue<Option<T>>,
    frame: StoredValue<Option<AnimationFrameRequestHandle>>,
    apply: Callback<T>,
}

impl<T: Copy + Send + Sync + 'static> FrameValue<T> {
    pub fn new(apply: impl Fn(T) + Send + Sync + 'static) -> Self {
        let value = Self {
            pending: StoredValue::new(None),
            frame: StoredValue::new(None),
            apply: Callback::new(apply),
        };
        on_cleanup(move || value.cancel());
        value
    }

    pub fn push(self, next: T) {
        self.pending.set_value(Some(next));
        if self.frame.get_value().is_none() {
            match request_animation_frame_with_handle(move || {
                self.frame.try_set_value(None);
                self.deliver();
            }) {
                Ok(handle) => self.frame.set_value(Some(handle)),
                Err(_) => self.deliver(),
            }
        }
    }

    /// Deliver the final input before saving a drag, even if its frame hasn't painted.
    pub fn flush(self) {
        self.cancel_frame();
        self.deliver();
    }

    pub fn cancel(self) {
        self.cancel_frame();
        self.pending.try_set_value(None);
    }

    fn cancel_frame(self) {
        if let Some(handle) = self.frame.try_update_value(Option::take).flatten() {
            handle.cancel();
        }
    }

    fn deliver(self) {
        if let Some(next) = self.pending.try_update_value(Option::take).flatten() {
            self.apply.run(next);
        }
    }
}
