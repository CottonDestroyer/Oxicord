//! Profile frames: the decorative borders sold in the shop, drawn around a
//! profile card. A frame is a list of layers, each a picture anchored to the
//! top, bottom or centre of the card, drawn in front of or behind it.

use serde::Deserialize;

use crate::Snowflake;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileFrame {
    pub sku: Snowflake,
    pub label: String,
    pub layers: Vec<FrameLayer>,
    /// Width the overflow values below were measured for.
    pub inner_width: u32,
    pub overflow_top: u32,
    pub overflow_bottom: u32,
    pub overflow_horizontal: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameLayer {
    pub id: Snowflake,
    pub kind: String,
    pub order: String,
    pub anchor: String,
    pub responsive: bool,
}

impl ProfileFrame {
    pub fn from_product(body: serde_json::Value, sku: Snowflake) -> Option<Self> {
        let product: Product = serde_json::from_value(body).ok()?;
        let item = product.items.into_iter().find(|item| item.kind == 3 && item.sku_id == sku)?;
        Some(Self {
            sku,
            label: item.label,
            layers: item.layers.into_iter().map(Into::into).collect(),
            inner_width: item.inner_width,
            overflow_top: item.overflow_top,
            overflow_bottom: item.overflow_bottom,
            overflow_horizontal: item.overflow_horizontal,
        })
    }
}

#[derive(Deserialize)]
struct Product {
    #[serde(default)]
    items: Vec<Item>,
}

#[derive(Deserialize)]
struct Item {
    #[serde(rename = "type")]
    kind: u8,
    sku_id: Snowflake,
    #[serde(default)]
    label: String,
    #[serde(default)]
    layers: Vec<FrameLayerRaw>,
    #[serde(default, rename = "inner_width")]
    inner_width: u32,
    #[serde(default, rename = "overflow_top")]
    overflow_top: u32,
    #[serde(default, rename = "overflow_bottom")]
    overflow_bottom: u32,
    #[serde(default, rename = "overflow_horizontal")]
    overflow_horizontal: u32,
}

#[derive(Deserialize)]
struct FrameLayerRaw {
    id: Snowflake,
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    order: String,
    #[serde(default)]
    anchor: String,
    #[serde(default)]
    responsive: bool,
}

impl From<FrameLayerRaw> for FrameLayer {
    fn from(raw: FrameLayerRaw) -> Self {
        Self {
            id: raw.id,
            kind: raw.kind,
            order: raw.order,
            anchor: raw.anchor,
            responsive: raw.responsive,
        }
    }
}

// Make FrameLayer deserializable directly too.
impl<'de> Deserialize<'de> for FrameLayer {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        FrameLayerRaw::deserialize(d).map(Into::into)
    }
}