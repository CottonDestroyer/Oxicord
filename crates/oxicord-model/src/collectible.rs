//! Nameplates, the collectible drawn behind a display name.
//!
//! The client reads these from the collectibles catalog
//! (`COLLECTIBLES_SHOP`, `reference/notes/endpoints.txt`). The live payload was
//! not captured, so the items below are the subset the profile editor draws,
//! written by hand until the shop endpoint is wired.

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

#[cfg(test)]
mod tests {
    use super::*;

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
