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

/// What the Change Profile Effect dialog lists. The live account has no owned
/// effects, so `owned` starts empty and the current one is added by the dialog
/// when the profile carries a sku.
#[derive(Clone, Debug, Default)]
pub struct EffectCatalog {
    pub owned: Vec<Effect>,
    pub nitro: Vec<Effect>,
    pub shop: Vec<Effect>,
    /// The preset picture drawn behind every effect tile.
    pub background: String,
}

/// One profile effect. The artwork is the still picture the picker draws.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Effect {
    pub sku_id: String,
    pub name: String,
    pub preview: String,
    pub locked: bool,
}

impl Effect {
    fn new(sku_id: &str, name: &str, preview: &str, locked: bool) -> Self {
        Self {
            sku_id: sku_id.to_owned(),
            name: name.to_owned(),
            preview: preview.to_owned(),
            locked,
        }
    }
}

impl EffectCatalog {
    /// The catalog captured from the live Change Profile Effect dialog on
    /// 2026-11-09 (1920 x 1080, dark theme).
    pub fn bundled() -> Self {
        let art = |hash: &str| {
            format!("https://cdn.discordapp.com/assets/content/{hash}?width=100&height=195")
        };
        let shop_art = |hash: &str| {
            format!("https://cdn.discordapp.com/media/v1/collectibles-shop/{hash}?width=100&height=195")
        };
        Self {
            owned: Vec::new(),
            background: "https://cdn.discordapp.com/assets/content/a859611882903f1102a796c08c68278917821af3e87eb9191df74bf78426971d.png".to_owned(),
            nitro: vec![
                Effect::new(
                    "1139323098370424932",
                    "Boost Relic",
                    &art("5699f6cc98b986d671478ca70c5942b8d7fa22f41d366f7d60cccea92ef91b4f"),
                    true,
                ),
                Effect::new(
                    "1139323103193878569",
                    "Cyberspace",
                    &art("3f62a90f9288e62fbffb7181fb5ee1c0d72feeae389da03a59cc001985e8750f"),
                    true,
                ),
            ],
            shop: vec![
                Effect::new(
                    "1556809105601990721",
                    "Dark Moon Crows",
                    &shop_art("ec0936f6f6a116d3e9af10bcc74e278a23bf94efe17c09b51d3bd9190731316b"),
                    true,
                ),
                Effect::new(
                    "1516560866609467452",
                    "Trapped Souls",
                    &shop_art("e2813c660991d5673001a44e5ab7756439dc269531da642f4cf9aa9482e8b2b9"),
                    true,
                ),
                Effect::new(
                    "1462116613745676445",
                    "Nevermore (Midnight)",
                    &art("cabac17518f20cc7c642dbeec8f6cec45e98820c4e9b8a78cceb44412a3985e8"),
                    true,
                ),
                Effect::new(
                    "1538992498695872543",
                    "Shattered Wings",
                    &shop_art("dc424970b232e08726c601206010cca8fb4e37b8124e4e15f24af32bb18742ba"),
                    true,
                ),
                Effect::new(
                    "1564419597501989044",
                    "Deadpool",
                    &shop_art("3cb1a113675ef687485e8b96f427c0fdf9bb5f7b6727601429b7a08cc79b4bb6"),
                    true,
                ),
                Effect::new(
                    "1488246427707768867",
                    "Hello Kitty",
                    &shop_art("80ee7cc461db7f7493816a03af3b03a620c8edc248f188f8b2913ffbf7266371"),
                    true,
                ),
                Effect::new(
                    "1538992070226608158",
                    "Whispering Rose",
                    &shop_art("fa324b1c3c17941dd4edb3cbeedc41d288660aa7cae440e1e61fd54651d78055"),
                    true,
                ),
                Effect::new(
                    "1559647861375766689",
                    "Pumpkin King",
                    &shop_art("6aeb427ac508cf460f7e81a82d8afdfbf990b04e44c0ff90eed2e4d0cdfe3102"),
                    true,
                ),
                Effect::new(
                    "1365410896196669480",
                    "Darth Vader Arrives",
                    &art("74e2a228eeb1d5539e158b3ff11ae6033bffe06ed65d1aa0b962baeb5f34d72b"),
                    true,
                ),
                Effect::new(
                    "1564420850902507540",
                    "Wolverine",
                    &shop_art("0b6ecc92fcd612b9d6a82f633e8aab4107f81b1f450c2df3f1e2cdecc2b5d0a6"),
                    true,
                ),
                Effect::new(
                    "1556803454863937566",
                    "Hook",
                    &shop_art("cab577b01a214b6b011a12711f9be71cbaa12cd422318075c6ccf06e3060d9d9"),
                    true,
                ),
                Effect::new(
                    "1556808801229873322",
                    "Dark Moon Crows",
                    &shop_art("e2d44b2af33aa880400d78215773e80c6965da0abb2a17099d75c2b12e212e57"),
                    true,
                ),
                Effect::new(
                    "1556809408984391730",
                    "Bloodweb",
                    &shop_art("5ef5ab33e52bf089d61928b5bb407d474e2812b37c3c4a2e0d92cc66522338e9"),
                    true,
                ),
                Effect::new(
                    "1561805807509569566",
                    "Papel Picado",
                    &shop_art("231e59dbbbb079cf28597c6532f49cdeb2be83da9fdf65180f868306cdedb691"),
                    true,
                ),
                Effect::new(
                    "1559633476419059792",
                    "Pumpkin King",
                    &shop_art("398dd05aaa9c58ef6318699e4173130230767dfbf7de5f4496c20453ca664a31"),
                    true,
                ),
                Effect::new(
                    "1545538298884259930",
                    "Odette",
                    &shop_art("2ea54999168210cb9b989fa7942891211248431b9f807b24ba51eb43e8a4d819"),
                    true,
                ),
                Effect::new(
                    "1545536891753857144",
                    "Autumn Garden",
                    &shop_art("665d257e9ff3475f7d5ca301191bfc7d95180dd1c119293fbc9fb0303abd0a98"),
                    true,
                ),
                Effect::new(
                    "1544102836437123182",
                    "Mer-Bubbles",
                    &shop_art("cbcc272d92c12625b122c3ac045007cbf203f4bc03d5e5a18271d711066642ed"),
                    true,
                ),
                Effect::new(
                    "1544103361148755998",
                    "Mermaid Muse",
                    &shop_art("76bde72f5d4d168f04e97f1ecaf729e29fe0bc0904deb90b19dd18b9191accaa"),
                    true,
                ),
                Effect::new(
                    "1544103047083466772",
                    "Dancing Dolphins",
                    &shop_art("72a3f56de03275f1f3e9fc4cac238e29984a7336961f79b2f27a979f01cdeb75"),
                    true,
                ),
                Effect::new(
                    "1541462269999972432",
                    "Mothman",
                    &shop_art("88278b3768b7a4e2dc71af8f77c136b07169c7eafb0cc752969bb07ada33e112"),
                    true,
                ),
                Effect::new(
                    "1541461252587397270",
                    "Moonflowers (Jackalope)",
                    &shop_art("ed46943222a428a7a2c26fd78cc36f3a17f31cd9e6f30855422f016f45ef8189"),
                    true,
                ),
                Effect::new(
                    "1541458389949874236",
                    "Jersey Devil",
                    &shop_art("c630a6338341e384a575da9bf2571237ae6b7317f8e77d47216d28b25a58270b"),
                    true,
                ),
                Effect::new(
                    "1541462491387662336",
                    "La Llorona",
                    &shop_art("daf7dc5e029a14dc6a1b5ae1bc000606d76cd212e2cb232e57f52baf66eda3e5"),
                    true,
                ),
            ],
        }
    }

    /// The item with `sku`, looking in owned, Nitro and shop.
    pub fn find(&self, sku: &str) -> Option<&Effect> {
        self.owned
            .iter()
            .chain(&self.nitro)
            .chain(&self.shop)
            .find(|e| e.sku_id == sku)
    }
}

/// What the Change Profile Frame dialog lists. The live account has no owned
/// frames, so `owned` starts empty and the current one is added by the dialog
/// when the profile carries a sku.
#[derive(Clone, Debug, Default)]
pub struct FrameCatalog {
    pub owned: Vec<FramePreset>,
    pub shop: Vec<FramePreset>,
    /// The preset portrait drawn inside every frame tile.
    pub sample: String,
}

/// One profile frame. The live tile draws a preset portrait inside a preview
/// container whose padding comes from the frame's overflow, then overlays the
/// frame's top and bottom pictures.
#[derive(Clone, Debug, PartialEq)]
pub struct FramePreset {
    pub sku_id: String,
    pub name: String,
    pub locked: bool,
    pub pad_top: f32,
    pub pad_x: f32,
    pub pad_bottom: f32,
    pub top: Option<FrameArtwork>,
    pub bottom: Option<FrameArtwork>,
}

/// One frame picture and the size it was authored at, used to keep its ratio.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameArtwork {
    pub url: String,
    pub width: u32,
    pub height: u32,
}

impl FrameCatalog {
    /// The catalog captured from the live Change Profile Frame dialog on
    /// 2026-11-09 (1920 x 1080, dark theme).
    pub fn bundled() -> Self {
        let art = |sku: &str, layer: &str, width: u32, height: u32| FrameArtwork {
            url: format!("https://cdn.discordapp.com/media/v1/collectibles-shop/{sku}/{layer}/static"),
            width,
            height,
        };
        let frame = |sku: &str,
                     name: &str,
                     pads: (f32, f32, f32),
                     top: Option<(&str, u32, u32)>,
                     bottom: Option<(&str, u32, u32)>| FramePreset {
            sku_id: sku.to_owned(),
            name: name.to_owned(),
            locked: true,
            pad_top: pads.0,
            pad_x: pads.1,
            pad_bottom: pads.2,
            top: top.map(|(layer, width, height)| art(sku, layer, width, height)),
            bottom: bottom.map(|(layer, width, height)| art(sku, layer, width, height)),
        };
        Self {
            owned: Vec::new(),
            sample: "https://cdn.discordapp.com/assets/content/a859611882903f1102a796c08c68278917821af3e87eb9191df74bf78426971d.png".to_owned(),
            shop: vec![
                frame(
                    "1556809649808736266",
                    "Crows perch on branches with a full moon behind them.",
                    (14.0156, 2.4375, 9.7656),
                    Some(("1556809813197983765", 1306, 716)),
                    Some(("1556809825642614794", 1306, 424)),
                ),
                frame(
                    "1564421748202410054",
                    "Deadpool lounges on his side along the top edge, above a soft blue glow.",
                    (14.0156, 2.57812, 0.0),
                    Some(("1567643983323340861", 1312, 716)),
                    None,
                ),
                frame(
                    "1551750280960286770",
                    "Glowing eyes and white smiling teeth peer out from above and below the profile.",
                    (13.875, 2.48438, 9.71875),
                    Some(("1551750347515633674", 1305, 708)),
                    Some(("1551750351496159263", 1305, 414)),
                ),
                frame(
                    "1491907428344795276",
                    "A soft white cartoon cat face peeks over the top edge with rounded pink ears and rosy cheeks, its paws resting along the upper corners",
                    (13.875, 2.48438, 6.76562),
                    Some(("1522378688971411657", 1307, 713)),
                    Some(("1522378692112809994", 1307, 359)),
                ),
                frame(
                    "1536493354450288680",
                    "Jagged black symbiote tendrils line the top and bottom edges of the profile",
                    (14.0156, 2.57812, 9.76562),
                    Some(("1536493443591708682", 1312, 716)),
                    Some(("1536493448750825562", 1312, 424)),
                ),
                frame(
                    "1517303399799062668",
                    "A glowing white border with delicate butterfly wings and sparkling light trails on top of the profile",
                    (13.7344, 2.57812, 4.92188),
                    Some(("1524824219844739122", 1312, 710)),
                    Some(("1524824224559268011", 1312, 319)),
                ),
                frame(
                    "1531413285243719880",
                    "A dark eclipse with dramatic clouds crowns the top of the profile, with glowing fiery light trailing down both sides",
                    (14.0156, 2.57812, 9.76562),
                    Some(("1533986938368036915", 1312, 716)),
                    Some(("1533986943057137715", 1312, 424)),
                ),
                frame(
                    "1538992680552636548",
                    "Pale white roses and curling vines with a soft lavender glow surround the profile",
                    (14.0156, 2.57812, 9.76562),
                    Some(("1538992839197720769", 1312, 716)),
                    Some(("1538992843312464012", 1312, 424)),
                ),
                frame(
                    "1524191647854891039",
                    "A pastel border adorned with a purple ribbon bow, small white bunnies, stars, and crescent moons frames the profile",
                    (5.79688, 2.57812, 5.34375),
                    Some(("1524192390502551724", 1312, 538)),
                    Some(("1524192394801713222", 1312, 328)),
                ),
                frame(
                    "1545538607811534988",
                    "A black cat peeks out from a pile of leaves at the top with more autumn leaves along the bottom.",
                    (14.0156, 2.57812, 9.76562),
                    Some(("1549900374805184522", 1312, 716)),
                    Some(("1549900379196624936", 1312, 424)),
                ),
                frame(
                    "1545193750261923880",
                    "Sparkling snow leopard print fur borders the profile, with a spotted tail curling along the bottom.",
                    (6.07812, 2.57812, 9.76562),
                    Some(("1545193824370954351", 1312, 544)),
                    Some(("1545193829689200710", 1312, 424)),
                ),
                frame(
                    "1524193380492509345",
                    "Soft white planets and dreamy cosmic clouds float above the profile against a pale sky",
                    (14.0156, 2.57812, 0.0),
                    Some(("1526660358712852691", 1312, 716)),
                    None,
                ),
                frame(
                    "1526697977513509007",
                    "Three white bunnies peeking over the top holding strawberries surrounded by more strawberries and white flowers",
                    (14.0156, 2.57812, 6.45312),
                    Some(("1526698280728133723", 1312, 716)),
                    Some(("1526698286248099932", 1312, 352)),
                ),
                frame(
                    "1564422684522188830",
                    "Weathered red border styled like Deadpool's suit, with his mask eyes peering down from the top.",
                    (10.7344, 2.57812, 2.53125),
                    Some(("1564422782006337607", 1312, 645)),
                    Some(("1564422786892566568", 1312, 267)),
                ),
                frame(
                    "1564422476820258846",
                    "Weathered yellow border styled like Wolverine's suit, with his pointed mask peering down from the top.",
                    (13.8281, 2.57812, 2.53125),
                    Some(("1564422584391565332", 1312, 712)),
                    Some(("1564422590662049932", 1312, 267)),
                ),
                frame(
                    "1564422284972654632",
                    "Wolverine leans over the top edge with his claws out, above a soft blue glow.",
                    (14.0156, 2.57812, 0.0),
                    Some(("1564422391415836784", 1312, 716)),
                    None,
                ),
                frame(
                    "1556803891289526392",
                    "Jagged, claw-like blades with glowing orange embers border the top and bottom of the profile.",
                    (14.0156, 2.57812, 9.76562),
                    Some(("1556804055202795591", 1307, 716)),
                    Some(("1556804059498029207", 1312, 424)),
                ),
                frame(
                    "1561806070656270466",
                    "An iridescent quetzal wraps around the top and bottom of the profile, its tail feathers trailing into leaves.",
                    (14.0156, 2.57812, 9.76562),
                    Some(("1561806203603132476", 1312, 716)),
                    Some(("1561806207197642893", 1312, 424)),
                ),
                frame(
                    "1545193867710562406",
                    "A raccoon peeks over the top of the profile, with a striped tail along the bottom.",
                    (14.0156, 2.57812, 9.76562),
                    Some(("1545565659092623491", 1312, 716)),
                    Some(("1545565664134037606", 1312, 424)),
                ),
                frame(
                    "1551647831360671905",
                    "Tilcayo cat lying down with paws stretched forward.",
                    (14.0156, 2.57812, 0.0),
                    Some(("1551648251701100665", 1312, 716)),
                    None,
                ),
                frame(
                    "1545538988813844600",
                    "Sunflowers grow along the top with gourds lining the bottom.",
                    (14.0156, 2.57812, 9.76562),
                    Some(("1549900243699503185", 1312, 716)),
                    Some(("1549900247415787601", 1312, 424)),
                ),
                frame(
                    "1544103549892300830",
                    "Iridescent pearls line the top and bottom of the profile.",
                    (8.75, 2.57812, 7.32812),
                    Some(("1547368619091042364", 1312, 602)),
                    Some(("1547368622840746045", 1312, 371)),
                ),
                frame(
                    "1541464541966114826",
                    "Bare, gnarled tree branches form a border around the proflie",
                    (9.90625, 2.57812, 6.125),
                    Some(("1541464621884510258", 1312, 627)),
                    Some(("1541464626057711706", 1312, 345)),
                ),
                frame(
                    "1541464049919983667",
                    "Pale moonflowers and vines form a border around the profile",
                    (13.9688, 2.53125, 9.76562),
                    Some(("1541464161576423504", 1310, 715)),
                    Some(("1541464165909012671", 1310, 424)),
                ),
                frame(
                    "1541462746032381982",
                    "Dark thorny branches form a spiked border around the profile",
                    (12.4375, 2.57812, 0.0),
                    Some(("1541463501657215026", 1312, 682)),
                    None,
                ),
                frame(
                    "1533925498961264871",
                    "A row of grey skulls crowns the top of the profile with blue flames erupting from them",
                    (14.0156, 2.57812, 6.8125),
                    Some(("1533925847021518939", 1311, 716)),
                    Some(("1533925855338827867", 1311, 360)),
                ),
                frame(
                    "1533925887098224730",
                    "A row of grey skulls crowns the top of the profile with red flames erupting from them",
                    (14.0156, 2.57812, 6.8125),
                    Some(("1534992785206411324", 1311, 716)),
                    Some(("1534992807885017249", 1311, 360)),
                ),
                frame(
                    "1531413463623139328",
                    "An ornate gold frame with celestial elements and dangling sun charm pendants",
                    (14.0156, 2.57812, 9.76562),
                    Some(("1532536724981547221", 1312, 716)),
                    Some(("1532536729821511811", 1312, 424)),
                ),
                frame(
                    "1524190883036266636",
                    "A deep navy and gold border with crescent moons, stars, and diamond accents frames the profile with an ornate celestial crest at the top",
                    (14.0156, 2.57812, 5.84375),
                    Some(("1524191337790836907", 1312, 716)),
                    Some(("1524191342538915931", 1312, 339)),
                ),
                frame(
                    "1526698360080306276",
                    "Bright blue illustrated water crests over the top and pools along the bottom of the profile",
                    (14.0156, 2.57812, 9.76562),
                    Some(("1526698477919146044", 1312, 716)),
                    Some(("1526698483346706442", 1312, 424)),
                ),
            ],
        }
    }

    /// The item with `sku`, looking in owned and the shop.
    pub fn find(&self, sku: &str) -> Option<&FramePreset> {
        self.owned.iter().chain(&self.shop).find(|f| f.sku_id == sku)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frame_catalog_matches_the_live_picker() {
        let catalog = FrameCatalog::bundled();
        assert!(catalog.owned.is_empty());
        assert_eq!(catalog.shop.len(), 30);
        assert!(catalog.shop.iter().all(|f| f.locked));
        assert!(!catalog.sample.is_empty());
        let first = catalog.find("1556809649808736266").unwrap();
        assert_eq!(first.name, "Crows perch on branches with a full moon behind them.");
        assert!(first.top.as_ref().unwrap().url.contains("/1556809649808736266/"));
        assert_eq!(first.top.as_ref().unwrap().width, 1306);
        assert!(catalog.find("nope").is_none());
    }

    #[test]
    fn the_effect_catalog_matches_the_live_picker() {
        let catalog = EffectCatalog::bundled();
        assert!(catalog.owned.is_empty());
        assert_eq!(catalog.nitro.len(), 2);
        assert_eq!(catalog.shop.len(), 24);
        assert!(catalog.nitro.iter().chain(&catalog.shop).all(|e| e.locked));
        assert!(!catalog.background.is_empty());
        let first = catalog.find("1139323098370424932").unwrap();
        assert_eq!(first.name, "Boost Relic");
        assert!(first.preview.ends_with("?width=100&height=195"));
        assert!(catalog.find("nope").is_none());
    }

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
