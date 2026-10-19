//! Opt-in bounded measurements of application update work, not input or GPU latency.
use std::{collections::VecDeque, time::Duration};
use eframe::egui;

const CAPACITY: usize = 256;
#[derive(Default)]
pub struct Metrics {
    pub enabled: bool,
    samples: VecDeque<(f64, bool)>,
}
impl Metrics {
    pub fn record(&mut self, elapsed: Duration, busy: bool) {
        if !self.enabled { return; }
        if self.samples.len() == CAPACITY { self.samples.pop_front(); }
        self.samples.push_back((elapsed.as_secs_f64() * 1000.0, busy));
    }
    fn summary(&self, busy: Option<bool>) -> serde_json::Value {
        let mut values: Vec<_> = self.samples.iter()
            .filter(|(_, active)| busy.is_none_or(|b| *active == b))
            .map(|(ms, _)| *ms).collect();
        values.sort_by(f64::total_cmp);
        let n = values.len();
        if n == 0 { return serde_json::json!({"samples":0}); }
        serde_json::json!({"samples":n,"mean_ms":values.iter().sum::<f64>() / n as f64,
            "p95_ms":values[(n * 95).div_ceil(100) - 1],"max_ms":values[n - 1]})
    }
    fn report(&self) -> serde_json::Value {
        serde_json::json!({"schema_version":1,"metric":"application_update_work",
            "tracked_job_flags":"speech, export, diagnostics, sequence preparation","capacity":CAPACITY,"os":std::env::consts::OS,"arch":std::env::consts::ARCH,
            "build_profile":if cfg!(debug_assertions) {"debug"} else {"release"},
            "all":self.summary(None),"tracked_jobs_active":self.summary(Some(true)),
            "tracked_jobs_idle":self.summary(Some(false)),
            "limitations":"Elapsed application update wall time only; excludes event queue delay, GPU presentation, audio latency and process RSS"})
    }
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.enabled, "Measure application update work");
        ui.weak("Last 256 updates. This measures elapsed update wall time, not input-to-display or audio latency. No automatic performance approval.");
        let summary = self.summary(None);
        ui.label(format!("{} retained samples", self.samples.len()));
        if !self.samples.is_empty() {
            ui.label(format!("Mean {:.2} ms · p95 {:.2} ms · max {:.2} ms",
                summary["mean_ms"].as_f64().unwrap_or(0.0),
                summary["p95_ms"].as_f64().unwrap_or(0.0),
                summary["max_ms"].as_f64().unwrap_or(0.0)));
        }
        ui.horizontal(|ui| {
            if ui.button("Copy timing report").clicked() { ui.ctx().copy_text(self.report().to_string()); }
            if ui.button("Clear measurements").clicked() { self.samples.clear(); }
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_window_evicts_old_outlier_and_separates_background_work() {
        let mut metrics = Metrics::default();
        metrics.record(Duration::from_secs(10), true);
        assert!(metrics.samples.is_empty());
        metrics.enabled = true;
        metrics.record(Duration::from_secs(10), true);
        for i in 0..CAPACITY { metrics.record(Duration::from_millis(2), i % 2 == 0); }
        let report = metrics.report();
        assert_eq!(report["all"]["samples"], CAPACITY);
        assert_eq!(report["all"]["max_ms"], 2.0);
        assert_eq!(report["tracked_jobs_active"]["samples"], CAPACITY / 2);
        assert_eq!(report["tracked_jobs_idle"]["samples"], CAPACITY / 2);
    }
}
