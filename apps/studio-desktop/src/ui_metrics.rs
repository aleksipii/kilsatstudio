//! Opt-in bounded measurements of application update work, not input or GPU latency.
use crate::input_latency::InputLatency;
use eframe::egui;
use std::{collections::VecDeque, time::Duration};

const CAPACITY: usize = 256;
#[derive(Default)]
pub struct Metrics {
    pub enabled: bool,
    samples: VecDeque<(f64, bool)>,
    pub latency: InputLatency,
}
impl Metrics {
    pub fn record(&mut self, elapsed: Duration, busy: bool) {
        if !self.enabled {
            return;
        }
        if self.samples.len() == CAPACITY {
            self.samples.pop_front();
        }
        self.samples
            .push_back((elapsed.as_secs_f64() * 1000.0, busy));
    }
    fn summary(&self, busy: Option<bool>) -> serde_json::Value {
        let mut values: Vec<_> = self
            .samples
            .iter()
            .filter(|(_, active)| busy.is_none_or(|b| *active == b))
            .map(|(ms, _)| *ms)
            .collect();
        values.sort_by(f64::total_cmp);
        let n = values.len();
        if n == 0 {
            return serde_json::json!({"samples":0});
        }
        serde_json::json!({"samples":n,"mean_ms":values.iter().sum::<f64>() / n as f64,
            "p95_ms":values[(n * 95).div_ceil(100) - 1],"max_ms":values[n - 1]})
    }
    fn report(&self) -> serde_json::Value {
        serde_json::json!({"schema_version":2,"input_latency":self.latency.report(),"metric":"application_update_work",
            "tracked_job_flags":"speech, export, diagnostics, sequence preparation","capacity":CAPACITY,"os":std::env::consts::OS,"arch":std::env::consts::ARCH,
            "build_profile":if cfg!(debug_assertions) {"debug"} else {"release"},
            "all":self.summary(None),"tracked_jobs_active":self.summary(Some(true)),
            "tracked_jobs_idle":self.summary(Some(false)),
            "limitations":"Vain sovelluksen päivityskierrokseen kulunut aika. Tapahtumajonon odotus, GPU:n kuvan esitys, ääniviive ja prosessin RSS-muistinkäyttö eivät sisälly mittaukseen."})
    }
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(
            &mut self.enabled,
            "Mittaa sovelluksen päivityskierroksen kestoa",
        );
        ui.weak("Viimeiset 256 päivityskierrosta. Mittaus kertoo kierroksiin kuluneen ajan. Syötteen ja kuvan välistä viivettä tai ääniviivettä ei mitata. Tulos ei anna automaattista suorituskykyhyväksyntää.");
        let summary = self.summary(None);
        ui.label(format!("{} mittausnäytettä", self.samples.len()));
        if !self.samples.is_empty() {
            ui.label(format!(
                "Keskiarvo {:.2} ms · p95 {:.2} ms · suurin {:.2} ms",
                summary["mean_ms"].as_f64().unwrap_or(0.0),
                summary["p95_ms"].as_f64().unwrap_or(0.0),
                summary["max_ms"].as_f64().unwrap_or(0.0)
            ));
        }
        ui.checkbox(
            &mut self.latency.enabled,
            "Mittaa syötteestä seuraavan päivityskierroksen alkuun",
        );
        ui.weak("Viimeiset 128 näppäin-, teksti-, napsautus- ja vierityssyötettä. Mittaus tehdään sovelluksessa: aika syötteen huomioivan kuvaruudun rakentumiseen ja seuraavan päivityskierroksen alkuun. Käyttöjärjestelmän tapahtumaviive ja kuvan piirtyminen fyysiselle näytölle eivät sisälly mittaukseen; valoviivettä ei mitata. Mittaus pyytää yhden ylimääräisen päivityskierroksen kutakin mitattua syötettä kohden.");
        if self.latency.enabled && !self.latency.is_empty() {
            let r = self.latency.report();
            ui.label(format!(
                "{} syötettä · seuraavan kierroksen alkuun p95 {:.2} ms · suurin {:.2} ms",
                self.latency.len(),
                r["input_to_next_frame_start"]["p95_ms"]
                    .as_f64()
                    .unwrap_or(0.0),
                r["input_to_next_frame_start"]["max_ms"]
                    .as_f64()
                    .unwrap_or(0.0)
            ));
        }
        ui.horizontal(|ui| {
            if ui.button("Kopioi ajoitusraportti").clicked() {
                ui.ctx().copy_text(self.report().to_string());
            }
            if ui.button("Tyhjennä mittaukset").clicked() {
                self.samples.clear();
                self.latency.clear();
            }
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
        for i in 0..CAPACITY {
            metrics.record(Duration::from_millis(2), i % 2 == 0);
        }
        let report = metrics.report();
        assert_eq!(report["all"]["samples"], CAPACITY);
        assert_eq!(report["all"]["max_ms"], 2.0);
        assert_eq!(report["tracked_jobs_active"]["samples"], CAPACITY / 2);
        assert_eq!(report["tracked_jobs_idle"]["samples"], CAPACITY / 2);
    }
}
