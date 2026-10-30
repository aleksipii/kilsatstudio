//! Write a prop card SVG for a stick-studio catalog item.
//!
//! ```sh
//! cargo run -p motion-rig --example prop_card -- kahvikuppi target/prop-cards/kahvikuppi.svg
//! ```
//!
//! The item is shown in every state, held in the right hand of the original
//! Veikko stick figure at its own scale.
use motion_rig::{prop_card_svg, stick_studio_hand_props, HandSide, RigDefinition};

const VEIKKO: &str = include_str!("../../../examples/stick-studio/characters/veikko.kilsat");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let catalog = stick_studio_hand_props()?;
    let known = || {
        catalog
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let Some(id) = args.next() else {
        return Err(format!("usage: prop_card <item> [out.svg]; items: {}", known()).into());
    };
    let item = catalog
        .items
        .iter()
        .find(|i| i.id == id)
        .ok_or_else(|| format!("unknown item {id}; items: {}", known()))?;
    let out = args
        .next()
        .unwrap_or_else(|| format!("target/prop-cards/{id}.svg"));
    let project: serde_json::Value = serde_json::from_str(VEIKKO)?;
    let source: serde_json::Value = serde_json::from_str(
        project["source"]
            .as_str()
            .ok_or("veikko.kilsat has no source")?,
    )?;
    let figure: RigDefinition =
        serde_json::from_value(source["characters"]["VEIKKO"]["rig"].clone())?;
    let card = prop_card_svg(item, &figure, HandSide::Right)?;
    if let Some(parent) = std::path::Path::new(&out).parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&out, &card.svg)?;
    println!(
        "{out}: {} x {} px, states {}",
        card.width,
        card.height,
        item.state_ids().join(", ")
    );
    Ok(())
}
