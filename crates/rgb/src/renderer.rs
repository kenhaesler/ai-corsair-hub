use std::time::Instant;

use crate::effect::EffectContext;
use crate::flow::FlowConfig;
use crate::frame::RgbFrame;
use crate::layer::{Layer, LayerStack};
use crate::layout::LedLayout;
use crate::transition::CrossFade;
use crate::Rgb;

/// Central RGB renderer — manages zones, layers, and frame output.
pub struct RgbRenderer {
    zones: Vec<ZoneState>,
    start_time: Instant,
    brightness: f32,
}

struct ZoneState {
    zone_id: String,
    devices: Vec<DeviceTarget>,
    layer_stack: LayerStack,
    brightness: f32,
    flow: Option<FlowConfig>,
    transition: Option<CrossFade>,
}

struct DeviceTarget {
    hub_serial: String,
    channel: u8,
    /// Stable device identity. `None` when the target came from a V1 config
    /// that only knows (hub_serial, channel). Populated at the `apply_rgb_config`
    /// construction site (in `apps/gui`) by looking up the channel in the
    /// runtime registry. Stays `None` during transition when the device is
    /// orphaned (referenced in config but not currently enumerated).
    device_id: Option<String>,
    layout: LedLayout,
}

impl RgbRenderer {
    pub fn new() -> Self {
        Self {
            zones: Vec::new(),
            start_time: Instant::now(),
            brightness: 1.0,
        }
    }

    /// Update the renderer configuration. Triggers crossfade if effects changed.
    pub fn update_config(&mut self, zones: &[ZoneConfig], brightness: f32) {
        let elapsed = self.start_time.elapsed().as_secs_f64();

        // Snapshot old frames for crossfade
        let old_frames: Vec<Option<Vec<Rgb>>> = self
            .zones
            .iter()
            .map(|z| {
                // Quick render to get current state for crossfade
                let ctx = EffectContext::default();
                let positions = z
                    .devices
                    .first()
                    .map(|d| d.layout.positions())
                    .unwrap_or_default();
                if positions.is_empty() {
                    None
                } else {
                    Some(z.layer_stack.render(&positions, elapsed, &ctx))
                }
            })
            .collect();

        self.brightness = brightness;
        let new_zones: Vec<ZoneState> = zones
            .iter()
            .enumerate()
            .map(|(i, cfg)| {
                let layer_stack = LayerStack::from_configs(&cfg.layers);
                let devices = cfg
                    .devices
                    .iter()
                    .map(|d| DeviceTarget {
                        hub_serial: d.hub_serial.clone(),
                        channel: d.channel,
                        device_id: d.device_id.clone(),
                        layout: d.layout.clone(),
                    })
                    .collect();

                let transition = old_frames
                    .get(i)
                    .and_then(|f| f.as_ref())
                    .map(|old| CrossFade::new(old.clone(), elapsed, 0.5));

                ZoneState {
                    zone_id: cfg.name.clone(),
                    devices,
                    layer_stack,
                    brightness: cfg.brightness as f32 / 100.0,
                    flow: cfg.flow.clone(),
                    transition,
                }
            })
            .collect();

        self.zones = new_zones;
    }

    /// Render one frame for all zones. Returns frames for each device.
    pub fn tick(&mut self, ctx: &EffectContext) -> Vec<RgbFrame> {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        let mut frames = Vec::new();

        for zone in &mut self.zones {
            let device_count = zone.devices.len();

            for (dev_idx, device) in zone.devices.iter().enumerate() {
                let positions = device.layout.positions();
                if positions.is_empty() {
                    continue;
                }

                // Apply flow timing offset
                let dev_elapsed = match &zone.flow {
                    Some(flow) => elapsed - flow.time_offset(dev_idx, device_count),
                    None => elapsed,
                };

                let mut leds = zone.layer_stack.render(&positions, dev_elapsed, ctx);

                // Apply crossfade transition if active
                if let Some(ref transition) = zone.transition {
                    if !transition.is_done(elapsed) {
                        leds = transition.blend(&leds, elapsed);
                    }
                }

                // Apply zone brightness then master brightness
                let total_brightness = zone.brightness * self.brightness;
                if total_brightness < 1.0 {
                    for led in &mut leds {
                        *led = led.dim(total_brightness);
                    }
                }

                frames.push(RgbFrame {
                    hub_serial: device.hub_serial.clone(),
                    channel: device.channel,
                    device_id: device.device_id.clone().unwrap_or_default(),
                    leds,
                });
            }

            // Clear completed transitions
            if zone
                .transition
                .as_ref()
                .is_some_and(|t| t.is_done(elapsed))
            {
                zone.transition = None;
            }
        }

        frames
    }
}

/// Config input for the renderer (matches the config.rs types).
pub struct ZoneConfig {
    pub name: String,
    pub devices: Vec<DeviceConfig>,
    pub layers: Vec<Layer>,
    pub brightness: u8,
    pub flow: Option<FlowConfig>,
}

pub struct DeviceConfig {
    pub hub_serial: String,
    pub channel: u8,
    /// Stable device identity when known. `None` during V1→V2 transition
    /// when the source config only had (hub_serial, channel).
    pub device_id: Option<String>,
    pub layout: LedLayout,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::BlendMode;
    use crate::effect::EffectConfig;

    #[test]
    fn renderer_produces_frames() {
        let mut renderer = RgbRenderer::new();
        renderer.update_config(
            &[ZoneConfig {
                name: "test".into(),
                devices: vec![DeviceConfig {
                    hub_serial: "HUB1".into(),
                    channel: 1,
                    device_id: None,
                    layout: LedLayout::qx_fan(),
                }],
                layers: vec![Layer {
                    effect_config: EffectConfig::Static {
                        color: Rgb::new(255, 0, 0),
                    },
                    blend_mode: BlendMode::Normal,
                    opacity: 1.0,
                    enabled: true,
                }],
                brightness: 100,
                flow: None,
            }],
            1.0,
        );

        let ctx = EffectContext::default();
        let frames = renderer.tick(&ctx);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].hub_serial, "HUB1");
        assert_eq!(frames[0].channel, 1);
        assert_eq!(frames[0].leds.len(), 34);
        assert_eq!(frames[0].leds[0], Rgb::new(255, 0, 0));
    }

    #[test]
    fn brightness_scales_output() {
        let mut renderer = RgbRenderer::new();
        renderer.update_config(
            &[ZoneConfig {
                name: "test".into(),
                devices: vec![DeviceConfig {
                    hub_serial: "HUB1".into(),
                    channel: 1,
                    device_id: None,
                    layout: LedLayout::FanRing { led_count: 4 },
                }],
                layers: vec![Layer {
                    effect_config: EffectConfig::Static {
                        color: Rgb::new(200, 200, 200),
                    },
                    blend_mode: BlendMode::Normal,
                    opacity: 1.0,
                    enabled: true,
                }],
                brightness: 50, // zone at 50%
                flow: None,
            }],
            0.5, // master at 50%
        );

        let ctx = EffectContext::default();
        let frames = renderer.tick(&ctx);
        // 200 * 0.5 * 0.5 = 50
        assert_eq!(frames[0].leds[0], Rgb::new(50, 50, 50));
    }

    #[test]
    fn multiple_zones_multiple_devices() {
        let mut renderer = RgbRenderer::new();
        renderer.update_config(
            &[
                ZoneConfig {
                    name: "zone1".into(),
                    devices: vec![
                        DeviceConfig {
                            hub_serial: "HUB1".into(),
                            channel: 1,
                            device_id: None,
                            layout: LedLayout::FanRing { led_count: 4 },
                        },
                        DeviceConfig {
                            hub_serial: "HUB1".into(),
                            channel: 2,
                            device_id: None,
                            layout: LedLayout::FanRing { led_count: 4 },
                        },
                    ],
                    layers: vec![Layer {
                        effect_config: EffectConfig::Static {
                            color: Rgb::new(0, 255, 0),
                        },
                        blend_mode: BlendMode::Normal,
                        opacity: 1.0,
                        enabled: true,
                    }],
                    brightness: 100,
                    flow: None,
                },
                ZoneConfig {
                    name: "zone2".into(),
                    devices: vec![DeviceConfig {
                        hub_serial: "HUB2".into(),
                        channel: 1,
                        device_id: None,
                        layout: LedLayout::ls350(),
                    }],
                    layers: vec![Layer {
                        effect_config: EffectConfig::Static {
                            color: Rgb::new(0, 0, 255),
                        },
                        blend_mode: BlendMode::Normal,
                        opacity: 1.0,
                        enabled: true,
                    }],
                    brightness: 100,
                    flow: None,
                },
            ],
            1.0,
        );

        let frames = renderer.tick(&EffectContext::default());
        assert_eq!(frames.len(), 3); // 2 from zone1 + 1 from zone2
    }

    #[test]
    fn empty_config_no_frames() {
        let mut renderer = RgbRenderer::new();
        renderer.update_config(&[], 1.0);
        let frames = renderer.tick(&EffectContext::default());
        assert!(frames.is_empty());
    }
}
