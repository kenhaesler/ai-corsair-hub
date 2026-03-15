use serde::{Deserialize, Serialize};

/// Cross-device synchronization: offsets time per device for cascade/flow effects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowConfig {
    /// Delay between each device in milliseconds.
    pub delay_per_device_ms: f32,
    /// Direction of the flow.
    pub direction: FlowDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowDirection {
    Forward,
    Reverse,
    CenterOut,
    EdgeIn,
}

impl Default for FlowDirection {
    fn default() -> Self {
        FlowDirection::Forward
    }
}

impl FlowConfig {
    /// Calculate the time offset (in seconds) for a device at the given index.
    /// `device_count` is the total number of devices in the zone.
    pub fn time_offset(&self, device_index: usize, device_count: usize) -> f64 {
        if device_count <= 1 {
            return 0.0;
        }

        let delay_sec = self.delay_per_device_ms as f64 / 1000.0;
        let idx = device_index as f64;
        let count = device_count as f64;
        let center = (count - 1.0) / 2.0;

        match self.direction {
            FlowDirection::Forward => idx * delay_sec,
            FlowDirection::Reverse => (count - 1.0 - idx) * delay_sec,
            FlowDirection::CenterOut => (idx - center).abs() * delay_sec,
            FlowDirection::EdgeIn => (center - (idx - center).abs()) * delay_sec,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_flow() {
        let flow = FlowConfig {
            delay_per_device_ms: 100.0,
            direction: FlowDirection::Forward,
        };
        assert!((flow.time_offset(0, 5) - 0.0).abs() < 1e-6);
        assert!((flow.time_offset(1, 5) - 0.1).abs() < 1e-6);
        assert!((flow.time_offset(4, 5) - 0.4).abs() < 1e-6);
    }

    #[test]
    fn reverse_flow() {
        let flow = FlowConfig {
            delay_per_device_ms: 100.0,
            direction: FlowDirection::Reverse,
        };
        assert!((flow.time_offset(0, 5) - 0.4).abs() < 1e-6);
        assert!((flow.time_offset(4, 5) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn center_out_flow() {
        let flow = FlowConfig {
            delay_per_device_ms: 100.0,
            direction: FlowDirection::CenterOut,
        };
        // 5 devices: center is index 2
        assert!((flow.time_offset(2, 5) - 0.0).abs() < 1e-6);
        assert!((flow.time_offset(0, 5) - 0.2).abs() < 1e-6);
        assert!((flow.time_offset(4, 5) - 0.2).abs() < 1e-6);
    }

    #[test]
    fn single_device_no_offset() {
        let flow = FlowConfig {
            delay_per_device_ms: 100.0,
            direction: FlowDirection::Forward,
        };
        assert!((flow.time_offset(0, 1) - 0.0).abs() < 1e-6);
    }
}
