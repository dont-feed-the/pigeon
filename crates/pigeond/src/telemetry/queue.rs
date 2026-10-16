//! Actual batch depths, including current work, not invented future
//! scheduler queues. Guards clear on every error/return path (12 §12.2).
pub(crate) struct Queue {
    group: String,
    class: &'static str,
    stage: &'static str,
    remaining: usize,
}
impl Queue {
    pub(crate) fn start(
        group: &str,
        class: &'static str,
        stage: &'static str,
        remaining: usize,
    ) -> Self {
        let queue = Self {
            group: group.into(),
            class,
            stage,
            remaining,
        };
        queue.record();
        queue
    }
    pub(crate) fn complete(&mut self, n: usize) {
        self.remaining = self.remaining.saturating_sub(n);
        self.record();
    }
    fn record(&self) {
        pigeon_telemetry::gauge(
            "pigeon.queue.depth",
            self.remaining as f64,
            &[
                ("group", &self.group),
                ("class", self.class),
                ("stage", self.stage),
            ],
        );
    }
}
impl Drop for Queue {
    fn drop(&mut self) {
        self.remaining = 0;
        self.record();
    }
}
