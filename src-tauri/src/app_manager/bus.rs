use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricValue {
    pub value: serde_json::Value,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub struct InterAppBus {
    // app_id -> metric_key -> MetricValue
    metrics: RwLock<HashMap<String, HashMap<String, MetricValue>>>,
}

impl Default for InterAppBus {
    fn default() -> Self {
        Self::new()
    }
}

impl InterAppBus {
    pub fn new() -> Self {
        Self {
            metrics: RwLock::new(HashMap::new()),
        }
    }

    /// Register or update public metrics exported by an app
    pub fn export_metrics(&self, app_id: &str, new_metrics: HashMap<String, serde_json::Value>) {
        let mut map = self.metrics.write().unwrap();
        let app_map = map.entry(app_id.to_string()).or_default();

        let now = chrono::Utc::now();
        for (key, val) in new_metrics {
            app_map.insert(
                key,
                MetricValue {
                    value: val,
                    updated_at: now,
                },
            );
        }
    }

    /// Query a metric from a target app
    pub fn query_metric(
        &self,
        target_app_id: &str,
        metric_name: &str,
    ) -> Option<serde_json::Value> {
        let map = self.metrics.read().unwrap();
        map.get(target_app_id)
            .and_then(|app_map| app_map.get(metric_name))
            .map(|mv| mv.value.clone())
    }

    /// Get all exported metrics across all apps for the Host Inspector
    pub fn get_all_exported(&self) -> HashMap<String, HashMap<String, MetricValue>> {
        let map = self.metrics.read().unwrap();
        map.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inter_app_bus_export_and_query() {
        let bus = InterAppBus::new();
        let mut metrics = HashMap::new();
        metrics.insert("engineBhpPeak".into(), serde_json::json!(505));
        metrics.insert("finalDriveRatio".into(), serde_json::json!(3.42));

        bus.export_metrics("powertrain_analyzer_v1", metrics);

        let query_result = bus.query_metric("powertrain_analyzer_v1", "engineBhpPeak");
        assert_eq!(query_result, Some(serde_json::json!(505)));

        let missing = bus.query_metric("powertrain_analyzer_v1", "nonExistent");
        assert_eq!(missing, None);
    }
}
