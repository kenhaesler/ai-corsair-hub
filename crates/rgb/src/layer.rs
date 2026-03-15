use serde::{Deserialize, Serialize};

use crate::color::BlendMode;
use crate::effect::{EffectConfig, EffectContext, RgbEffect};
use crate::Rgb;

/// A single layer in the compositing stack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    pub effect_config: EffectConfig,
    pub blend_mode: BlendMode,
    pub opacity: f32,
    pub enabled: bool,
}

/// Runtime state for an active layer (compiled from Layer config).
pub struct ActiveLayer {
    pub effect: Box<dyn RgbEffect>,
    pub blend_mode: BlendMode,
    pub opacity: f32,
    pub enabled: bool,
}

impl ActiveLayer {
    pub fn from_config(layer: &Layer) -> Self {
        ActiveLayer {
            effect: layer.effect_config.create_effect(),
            blend_mode: layer.blend_mode,
            opacity: layer.opacity,
            enabled: layer.enabled,
        }
    }
}

/// Composable layer stack — renders multiple effects with blend modes.
pub struct LayerStack {
    layers: Vec<ActiveLayer>,
}

impl LayerStack {
    pub fn new(layers: Vec<ActiveLayer>) -> Self {
        Self { layers }
    }

    pub fn from_configs(configs: &[Layer]) -> Self {
        let layers = configs.iter().map(ActiveLayer::from_config).collect();
        Self { layers }
    }

    /// Render the full stack: start with black, composite each layer bottom-to-top.
    pub fn render(&self, positions: &[f32], elapsed: f64, ctx: &EffectContext) -> Vec<Rgb> {
        let count = positions.len();
        let mut result = vec![Rgb::BLACK; count];

        for layer in &self.layers {
            if !layer.enabled || layer.opacity <= 0.0 {
                continue;
            }

            let layer_colors = layer.effect.render(positions, elapsed, ctx);

            for (i, pixel) in result.iter_mut().enumerate() {
                if let Some(&layer_color) = layer_colors.get(i) {
                    *pixel = layer.blend_mode.apply(*pixel, layer_color, layer.opacity);
                }
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect::EffectConfig;

    #[test]
    fn single_layer_static() {
        let stack = LayerStack::from_configs(&[Layer {
            effect_config: EffectConfig::Static {
                color: Rgb::new(255, 0, 0),
            },
            blend_mode: BlendMode::Normal,
            opacity: 1.0,
            enabled: true,
        }]);

        let positions = vec![0.0, 0.5, 1.0];
        let ctx = EffectContext::default();
        let result = stack.render(&positions, 0.0, &ctx);

        assert_eq!(result.len(), 3);
        for c in &result {
            assert_eq!(*c, Rgb::new(255, 0, 0));
        }
    }

    #[test]
    fn disabled_layer_ignored() {
        let stack = LayerStack::from_configs(&[
            Layer {
                effect_config: EffectConfig::Static {
                    color: Rgb::new(0, 0, 255),
                },
                blend_mode: BlendMode::Normal,
                opacity: 1.0,
                enabled: true,
            },
            Layer {
                effect_config: EffectConfig::Static {
                    color: Rgb::new(255, 0, 0),
                },
                blend_mode: BlendMode::Normal,
                opacity: 1.0,
                enabled: false, // disabled
            },
        ]);

        let positions = vec![0.5];
        let ctx = EffectContext::default();
        let result = stack.render(&positions, 0.0, &ctx);
        assert_eq!(result[0], Rgb::new(0, 0, 255));
    }

    #[test]
    fn two_layer_add_blend() {
        let stack = LayerStack::from_configs(&[
            Layer {
                effect_config: EffectConfig::Static {
                    color: Rgb::new(100, 0, 0),
                },
                blend_mode: BlendMode::Normal,
                opacity: 1.0,
                enabled: true,
            },
            Layer {
                effect_config: EffectConfig::Static {
                    color: Rgb::new(0, 100, 0),
                },
                blend_mode: BlendMode::Add,
                opacity: 1.0,
                enabled: true,
            },
        ]);

        let positions = vec![0.5];
        let ctx = EffectContext::default();
        let result = stack.render(&positions, 0.0, &ctx);
        assert_eq!(result[0], Rgb::new(100, 100, 0));
    }

    #[test]
    fn zero_opacity_no_effect() {
        let stack = LayerStack::from_configs(&[
            Layer {
                effect_config: EffectConfig::Static {
                    color: Rgb::new(50, 50, 50),
                },
                blend_mode: BlendMode::Normal,
                opacity: 1.0,
                enabled: true,
            },
            Layer {
                effect_config: EffectConfig::Static {
                    color: Rgb::new(255, 0, 0),
                },
                blend_mode: BlendMode::Normal,
                opacity: 0.0,
                enabled: true,
            },
        ]);

        let positions = vec![0.5];
        let ctx = EffectContext::default();
        let result = stack.render(&positions, 0.0, &ctx);
        assert_eq!(result[0], Rgb::new(50, 50, 50));
    }
}
