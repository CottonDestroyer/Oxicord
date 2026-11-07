//! Profile effects: layers of animated pictures drawn over a profile card,
//! sold in the shop. The shop catalog describes them, and when each layer
//! shows is decided by the same rules the web client uses.

use serde::Deserialize;

use crate::Snowflake;

/// How long the web client waits after a card opens before the first layer
/// may start, so the card is seen before the effect covers it.
pub const INTRO_MS: i64 = 500;

/// One animated picture of an effect. Its coordinates are on a card 450 wide.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EffectLayer {
    pub src: String,
    /// Plays again and again instead of once.
    pub looping: bool,
    pub start_ms: i64,
    pub duration_ms: i64,
    /// Rest between two plays of a looping layer.
    pub loop_delay_ms: i64,
    pub z: i32,
    pub x: i32,
    pub y: i32,
}

impl EffectLayer {
    /// Whether the layer is on screen `at` milliseconds into the effect.
    pub fn visible_at(&self, at: i64) -> bool {
        if at < self.start_ms {
            return false;
        }
        if !self.looping {
            return at <= self.start_ms + self.duration_ms;
        }
        if self.loop_delay_ms <= 0 {
            return true;
        }
        self.phase(at) <= self.duration_ms
    }

    /// Time into the current play of a looping layer that rests in between.
    fn phase(&self, at: i64) -> i64 {
        (at - self.start_ms) % (self.duration_ms + self.loop_delay_ms)
    }

    /// The next moment after `at` when the layer shows up or goes away, if
    /// it ever will again.
    pub fn next_change(&self, at: i64) -> Option<i64> {
        if at < self.start_ms {
            return Some(self.start_ms);
        }
        if !self.looping {
            return (at <= self.start_ms + self.duration_ms).then_some(self.start_ms + self.duration_ms + 1);
        }
        if self.loop_delay_ms <= 0 {
            return None;
        }
        let phase = self.phase(at);
        Some(if phase <= self.duration_ms {
            at + (self.duration_ms - phase) + 1
        } else {
            at + (self.duration_ms + self.loop_delay_ms - phase)
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProfileEffect {
    pub sku: Snowflake,
    /// What the effect looks like, for people who cannot see it.
    pub label: String,
    /// A single picture for those who turned motion off.
    pub still: Option<String>,
    /// In the order they are drawn, lowest first.
    pub layers: Vec<EffectLayer>,
}

impl ProfileEffect {
    /// The effect `sku` from a `/collectibles-products/{sku}` answer. Some
    /// effects pick one of several pictures for each layer at random, all
    /// layers taking the same turn: `pick` says which.
    pub fn from_product(body: serde_json::Value, sku: Snowflake, pick: usize) -> Option<Self> {
        let product: Product = serde_json::from_value(body).ok()?;
        let item = product
            .items
            .into_iter()
            .filter_map(|item| serde_json::from_value::<Item>(item).ok())
            .find(|item| item.kind == 1 && item.sku_id == sku)?;

        // The turn can only be one every layer has.
        let choices = item
            .effects
            .iter()
            .map(|layer| layer.randomized_sources.len())
            .filter(|n| *n > 0)
            .min()
            .unwrap_or(0);
        let turn = if choices > 1 { pick % choices } else { 0 };

        let mut layers: Vec<EffectLayer> = item
            .effects
            .into_iter()
            .map(|layer| {
                let src = layer
                    .randomized_sources
                    .get(turn)
                    .map(|s| s.src.clone())
                    .unwrap_or(layer.src);
                EffectLayer {
                    src,
                    looping: layer.looping,
                    start_ms: layer.start,
                    duration_ms: layer.duration,
                    loop_delay_ms: layer.loop_delay,
                    z: layer.z_index,
                    x: layer.position.as_ref().map_or(0, |p| p.x),
                    y: layer.position.as_ref().map_or(0, |p| p.y),
                }
            })
            .filter(|layer| !layer.src.is_empty())
            .collect();
        layers.sort_by_key(|layer| layer.z);

        let effect = Self {
            sku,
            label: item.accessibility_label,
            still: item.reduced_motion_src.filter(|s| !s.is_empty()),
            layers,
        };
        (!effect.layers.is_empty() || effect.still.is_some()).then_some(effect)
    }

    /// Layers on screen `at` milliseconds in, in drawing order.
    pub fn visible_at(&self, at: i64) -> impl Iterator<Item = &EffectLayer> {
        self.layers.iter().filter(move |layer| layer.visible_at(at))
    }

    /// When the set of layers on screen changes next, if it does.
    pub fn next_change(&self, at: i64) -> Option<i64> {
        self.layers.iter().filter_map(|layer| layer.next_change(at)).min()
    }
}

#[derive(Deserialize)]
struct Product {
    #[serde(default)]
    items: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct Item {
    #[serde(rename = "type")]
    kind: u8,
    sku_id: Snowflake,
    #[serde(default, rename = "accessibilityLabel")]
    accessibility_label: String,
    #[serde(default, rename = "reducedMotionSrc")]
    reduced_motion_src: Option<String>,
    #[serde(default)]
    effects: Vec<Layer>,
}

#[derive(Deserialize)]
struct Layer {
    #[serde(default)]
    src: String,
    #[serde(default, rename = "loop")]
    looping: bool,
    #[serde(default)]
    start: i64,
    #[serde(default)]
    duration: i64,
    #[serde(default, rename = "loopDelay")]
    loop_delay: i64,
    #[serde(default, rename = "zIndex")]
    z_index: i32,
    #[serde(default)]
    position: Option<Position>,
    #[serde(default, rename = "randomizedSources")]
    randomized_sources: Vec<Source>,
}

#[derive(Deserialize)]
struct Position {
    x: i32,
    y: i32,
}

#[derive(Deserialize)]
struct Source {
    src: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(looping: bool, start: i64, duration: i64, loop_delay: i64) -> EffectLayer {
        EffectLayer { src: "a".into(), looping, start_ms: start, duration_ms: duration, loop_delay_ms: loop_delay, ..Default::default() }
    }

    #[test]
    fn a_single_play_shows_from_its_start_until_it_ends() {
        let one = layer(false, 0, 5000, 0);
        assert!(one.visible_at(0));
        assert!(one.visible_at(5000));
        assert!(!one.visible_at(5001));
        assert_eq!(one.next_change(100), Some(5001));
        assert_eq!(one.next_change(6000), None);
    }

    #[test]
    fn nothing_shows_before_the_start() {
        let late = layer(true, 5500, 5042, 3500);
        assert!(!late.visible_at(5499));
        assert!(late.visible_at(5500));
        assert_eq!(late.next_change(0), Some(5500));
    }

    #[test]
    fn a_loop_with_a_rest_comes_and_goes() {
        let looping = layer(true, 5500, 5000, 3500);
        // On for 5000 ms, off for 3500 ms, then again.
        assert!(looping.visible_at(10500));
        assert!(!looping.visible_at(10501));
        assert!(!looping.visible_at(13999));
        assert!(looping.visible_at(14000));
        assert_eq!(looping.next_change(11000), Some(14000));
        assert_eq!(looping.next_change(12000), Some(14000));
        assert_eq!(looping.next_change(14000), Some(19001));
    }

    #[test]
    fn a_loop_without_a_rest_never_changes() {
        let endless = layer(true, 0, 3000, 0);
        assert!(endless.visible_at(1_000_000));
        assert_eq!(endless.next_change(10), None);
    }

    const PRODUCT: &str = r#"{"sku_id":"9","items":[
        {"type":1,"sku_id":"9","accessibilityLabel":"A red sky","reducedMotionSrc":"https://cdn/still",
         "effects":[
            {"src":"https://cdn/b","loop":true,"duration":5042,"start":5500,"loopDelay":3500,"zIndex":101,"randomizedSources":[]},
            {"src":"https://cdn/a","loop":false,"duration":5000,"start":0,"loopDelay":0,"zIndex":100,"position":{"x":4,"y":-8},"randomizedSources":[]}
         ]}]}"#;

    fn parse(body: &str, sku: u64, pick: usize) -> Option<ProfileEffect> {
        ProfileEffect::from_product(serde_json::from_str(body).unwrap_or_default(), Snowflake(sku), pick)
    }

    #[test]
    fn the_catalog_entry_gives_layers_in_drawing_order() {
        let effect = parse(PRODUCT, 9, 0).unwrap();
        assert_eq!(effect.label, "A red sky");
        assert_eq!(effect.still.as_deref(), Some("https://cdn/still"));
        assert_eq!(effect.layers.len(), 2);
        assert_eq!(effect.layers[0].src, "https://cdn/a");
        assert_eq!((effect.layers[0].x, effect.layers[0].y), (4, -8));
        assert!(effect.layers[1].looping);
    }

    #[test]
    fn only_the_layers_on_screen_are_listed() {
        let effect = parse(PRODUCT, 9, 0).unwrap();
        let srcs = |at| effect.visible_at(at).map(|l| l.src.as_str()).collect::<Vec<_>>();
        assert_eq!(srcs(1000), ["https://cdn/a"]);
        assert_eq!(srcs(5250), Vec::<&str>::new());
        assert_eq!(srcs(6000), ["https://cdn/b"]);
        assert_eq!(effect.next_change(1000), Some(5001));
        assert_eq!(effect.next_change(5250), Some(5500));
    }

    #[test]
    fn every_layer_takes_the_same_random_turn() {
        let body = r#"{"items":[{"type":1,"sku_id":"9","effects":[
            {"src":"x","randomizedSources":[{"src":"x0"},{"src":"x1"},{"src":"x2"}]},
            {"src":"y","randomizedSources":[{"src":"y0"},{"src":"y1"}]},
            {"src":"z"}]}]}"#;
        let effect = parse(body, 9, 3).unwrap();
        // Two choices are common to the layers that have any, 3 % 2 is 1.
        let srcs: Vec<_> = effect.layers.iter().map(|l| l.src.as_str()).collect();
        assert_eq!(srcs, ["x1", "y1", "z"]);
    }

    #[test]
    fn a_product_that_is_not_that_effect_gives_nothing() {
        assert!(parse(PRODUCT, 10, 0).is_none());
        assert!(parse("not json", 9, 0).is_none());
        assert!(parse(r#"{"items":[{"type":2,"sku_id":"9"}]}"#, 9, 0).is_none());
    }
}
