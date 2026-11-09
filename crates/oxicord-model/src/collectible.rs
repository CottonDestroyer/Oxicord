//! Collectibles the profile editor draws: nameplates and avatar decorations.
//!
//! The client reads these from the collectibles catalog
//! (`COLLECTIBLES_SHOP`, `reference/notes/endpoints.txt`). The live payload was
//! not captured, so the items below are the subset the profile editor draws,
//! written by hand from the live picker (2026-11-09) until the shop endpoint is
//! wired. The artwork itself comes from `cdn::collectible`, keyed by sku.

use serde::Deserialize;

/// What the Change Nameplate dialog lists: what the user has and what the
/// shop sells.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct NameplateCatalog {
    pub owned: Vec<Nameplate>,
    pub shop: Vec<Nameplate>,
}

/// One nameplate. The real item is a collectible product with a sku, a label
/// and a palette the client uses to pick the artwork.
#[derive(Clone, Debug, Deserialize)]
pub struct Nameplate {
    pub sku_id: String,
    pub name: String,
    pub palette: String,
    pub locked: bool,
}

impl Nameplate {
    fn new(sku_id: &str, name: &str, palette: &str, locked: bool) -> Self {
        Self { sku_id: sku_id.to_owned(), name: name.to_owned(), palette: palette.to_owned(), locked }
    }
}

impl NameplateCatalog {
    /// The bundled catalog, used until the collectibles endpoints are wired.
    pub fn bundled() -> Self {
        Self {
            owned: vec![
                Nameplate::new("1250700107861811200", "Aurora", "purple", false),
                Nameplate::new("1250700107861811201", "Starfall", "violet", false),
            ],
            shop: vec![
                Nameplate::new("1250700107861811202", "Nebula", "blue", true),
                Nameplate::new("1250700107861811203", "Moonlit", "silver", true),
                Nameplate::new("1250700107861811204", "Ember", "ember", true),
                Nameplate::new("1250700107861811205", "Abyss", "abyss", true),
            ],
        }
    }

    pub fn find(&self, sku: &str) -> Option<&Nameplate> {
        self.owned.iter().chain(&self.shop).find(|n| n.sku_id == sku)
    }
}

/// What the Change Avatar Decoration dialog lists. The live account has no
/// owned decorations, so `owned` starts empty and the current one is added by
/// the dialog when the profile carries a sku.
#[derive(Clone, Debug, Default)]
pub struct DecorationCatalog {
    pub owned: Vec<Decoration>,
    pub nitro: Vec<Decoration>,
    pub shop: Vec<Decoration>,
}

/// One avatar decoration. The artwork is the CDN's picture for `sku_id`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decoration {
    pub sku_id: String,
    pub name: String,
    pub locked: bool,
}

impl Decoration {
    fn new(sku_id: &str, name: &str, locked: bool) -> Self {
        Self { sku_id: sku_id.to_owned(), name: name.to_owned(), locked }
    }
}

impl DecorationCatalog {
    /// The catalog captured from the live Change Avatar Decoration dialog on
    /// 2026-11-09 (1920 x 1080, dark theme).
    pub fn bundled() -> Self {
        Self {
            owned: Vec::new(),
            nitro: vec![
                Decoration::new(
                    "1144058522808614923",
                    "Futuristic Headphones, Blue, Animated",
                    true,
                ),
                Decoration::new(
                    "1144058844004233369",
                    "Futuristic Interface, Pink, Animated",
                    true,
                ),
                Decoration::new(
                    "1144059132517826601",
                    "Smoke with Green Shining Border, Animated",
                    true,
                ),
            ],
            shop: vec![
                Decoration::new(
                    "1564412220744671322",
                    "Deadpool lies on his stomach, kicking his feet as pink hearts float up.",
                    true,
                ),
                Decoration::new(
                    "1556801850206978138",
                    "Crows fly out of a dark, jagged crescent moon in red with smoke-like edges.",
                    true,
                ),
                Decoration::new(
                    "1212569433839636530",
                    "Cat ears and whiskers adorn the avatar, giving a perfectly adorable cat-like charm.",
                    true,
                ),
                Decoration::new(
                    "1333866045236314327",
                    "Your avatar is dressed as an angel with fluffy angel wings and a glowing halo.",
                    true,
                ),
                Decoration::new("1538991362601656420", "Pale, glowing butterflies flutter around the avatar", true),
                Decoration::new(
                    "1144308439720394944",
                    "Anger Symbol with Exclamation Mark Speech Bubble, Anime-Style, Red, Animated",
                    true,
                ),
                Decoration::new(
                    "1538990902306152558",
                    "Sparkling pale roses and delicate vines form a circular wreath",
                    true,
                ),
                Decoration::new(
                    "1436367668897775757",
                    "Animated red butterfly circling around avatar with glowing particle trail effect.",
                    true,
                ),
                Decoration::new("1545192075551703201", "Spotted snow leopard ears and a curled tail.", true),
                Decoration::new("1481388474673139855", "Glossy black tendrils creep inward", true),
                Decoration::new(
                    "1545191258564075630",
                    "Cat ears tied with pink bows above paws wearing a bell collar.",
                    true,
                ),
                Decoration::new("1538991167511994549", "A sparkling crescent moon peeks through soft grey clouds", true),
                Decoration::new("1545191973307154432", "Pink bunny ears downturned with a purple bow on top.", true),
                Decoration::new(
                    "1516559294819074088",
                    "Contorting creature with long hair and centipede legs crawls around avatar",
                    true,
                ),
                Decoration::new(
                    "1564417461074272356",
                    "Wolverine pops up at the top, unsheathes his claws, and slashes yellow claw marks.",
                    true,
                ),
                Decoration::new("1559628807718051933", "A ring of bones with a bat collar", true),
                Decoration::new("1556799118427553842", "A ring of jagged, claw-like blades.", true),
                Decoration::new(
                    "1556807371660206281",
                    "A ring of pulsing red veins webbed together with dark pods.",
                    true,
                ),
            ],
        }
    }

    /// The item with `sku`, looking in owned, Nitro and shop.
    pub fn find(&self, sku: &str) -> Option<&Decoration> {
        self.owned
            .iter()
            .chain(&self.nitro)
            .chain(&self.shop)
            .find(|d| d.sku_id == sku)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_decoration_catalog_matches_the_live_picker() {
        let catalog = DecorationCatalog::bundled();
        assert!(catalog.owned.is_empty());
        assert_eq!(catalog.nitro.len(), 3);
        assert_eq!(catalog.shop.len(), 18);
        assert!(catalog.nitro.iter().chain(&catalog.shop).all(|d| d.locked));
        let first = catalog.find("1144058522808614923").unwrap();
        assert_eq!(first.name, "Futuristic Headphones, Blue, Animated");
        assert!(catalog.find("nope").is_none());
    }

    #[test]
    fn the_bundled_catalog_lists_owned_and_shop_nameplates() {
        let catalog = NameplateCatalog::bundled();
        assert_eq!(catalog.owned.len(), 2);
        assert_eq!(catalog.shop.len(), 4);
        assert!(catalog.shop.iter().all(|n| n.locked));
        assert!(catalog.owned.iter().all(|n| !n.locked));
        let first = catalog.find("1250700107861811200").unwrap();
        assert_eq!(first.name, "Aurora");
        assert_eq!(first.palette, "purple");
        assert!(catalog.find("nope").is_none());
    }
}
