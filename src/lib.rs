//! Military Symbols Library: APP-6D (MIL-STD-2525D) SIDC tagging and schematic styling.
//!
//! RustGIS has no image/SVG point symbols (its markers are circle, square, triangle, diamond,
//! star), so true milsymbol icons can't be drawn on the map. This plugin instead:
//!
//! - `tag-layer` (tool): parses a 20-digit APP-6D SIDC and writes its semantics as feature
//!   properties (round-tripped through GeoJSON and saved with the project).
//! - `style-app6` (command): applies Unique Values symbology on the `affiliation` field and
//!   colours each class with its APP-6 standard colour (blue/red/green/yellow). Needs `edit`.
//! - `about` (command): a one-line summary.
//!
//! Tools make no host requests, so styling lives in a command, not in the tool.

use rustgis_plugin::{Kind, Map, Request, ToolOutput, Value, geojson, host, json};

/// `plugin.json`, for its translations.
const MANIFEST: &str = include_str!("../plugin.json");

/// APP-6 affiliation label → standard colour, used by `style-app6`.
const AFFILIATION_COLORS: &[(&str, &str)] = &[
    ("Friend", "#3399FF"),
    ("Assumed Friend", "#3399FF"),
    ("Neutral", "#2ECC40"),
    ("Hostile", "#FF3B3B"),
    ("Suspect", "#FF3B3B"),
    ("Unknown", "#FFDC00"),
    ("Pending", "#FFDC00"),
];

fn handle(req: Request) -> Result<Value, String> {
    match (req.kind, req.id.as_str()) {
        (Kind::Command, "about") => about(&req),
        (Kind::Command, "style-app6") => style_app6(&req),
        (Kind::Tool, "tag-layer") => tag_layer(&req),
        (_, other) => Err(format!("Military Symbols Library has no '{other}'")),
    }
}

rustgis_plugin::export!(handle);

// --- Commands -------------------------------------------------------------------------------

fn about(req: &Request) -> Result<Value, String> {
    host::notify(&req.tr(
        MANIFEST,
        "MilSymbols: APP-6D SIDC tagging. Run 'Tag Layer With APP-6D SIDC', then 'Style by Affiliation'.",
        &[],
    ));
    Ok(json!({"plugin": "milsymb", "standard": "APP-6D", "status": "ready"}))
}

fn style_app6(req: &Request) -> Result<Value, String> {
    let layer = match req.str("input").map(str::trim).filter(|s| !s.is_empty()) {
        Some(name) => name.to_string(),
        None => active_vector_layer()?,
    };
    host::run("layer.symbology", json!({"layer": layer, "type": "unique", "field": "affiliation"}))?;
    // Colour each affiliation class; a class absent from the layer just errors, so ignore those.
    let mut styled = 0;
    for (value, color) in AFFILIATION_COLORS {
        if host::run("layer.classStyle", json!({"layer": layer, "value": value, "color": color})).is_ok() {
            styled += 1;
        }
    }
    host::notify(&req.tr(MANIFEST, "Styled '{layer}' by APP-6D affiliation.", &[("layer", &layer)]));
    Ok(json!({"layer": layer, "classesStyled": styled}))
}

/// The active vector layer's name, else the first vector layer.
fn active_vector_layer() -> Result<String, String> {
    let layers = host::layers()?;
    let all = layers.as_array().ok_or("the map has no layers")?;
    all.iter()
        .find(|l| l["active"] == true && l["raster"] != true)
        .or_else(|| all.iter().find(|l| l["raster"] != true))
        .and_then(|l| l["name"].as_str())
        .map(str::to_string)
        .ok_or_else(|| "no vector layer; select one or pass 'input'".into())
}

// --- Tool -----------------------------------------------------------------------------------

fn tag_layer(req: &Request) -> Result<Value, String> {
    let features = req.features("input")?;
    let input_name = req.str("input").unwrap_or("Layer");
    let sidc_default = normalize_sidc(req.str("sidc").unwrap_or(""));
    let sidc_field = req.str("sidcField").unwrap_or("sidc").trim().to_string();
    let label = req.str("label").unwrap_or("").trim().to_string();
    let label_field = req.str("labelField").unwrap_or("symbol_label").trim().to_string();
    let override_existing = req.bool("overrideExisting").unwrap_or(true);

    if sidc_default.is_empty() && sidc_field.is_empty() {
        return Err("a default 'sidc' or a 'sidcField' is required".into());
    }

    let mut tagged: Vec<Value> = Vec::with_capacity(features.len());
    let mut rejected: Vec<Value> = Vec::new();
    let (mut valid, mut invalid) = (0u64, 0u64);

    for f in features {
        let mut props: Map<String, Value> = match f.get("properties") {
            Some(Value::Object(m)) => m.clone(),
            _ => Map::new(),
        };

        // Prefer the per-feature SIDC field, fall back to the tool's default.
        let mut sidc_value = String::new();
        if !sidc_field.is_empty() {
            if let Some(v) = props.get(&sidc_field) {
                let raw = match v {
                    Value::String(s) => s.clone(),
                    Value::Null => String::new(),
                    other => other.to_string(),
                };
                sidc_value = normalize_sidc(&raw);
            }
        }
        if sidc_value.is_empty() {
            sidc_value = sidc_default.clone();
        }

        let mut feature = f.clone();
        match classify(&sidc_value) {
            Ok(parsed) => {
                valid += 1;
                if override_existing || !props.contains_key("sidc") {
                    props.insert("sidc".into(), json!(sidc_value));
                }
                if let Value::Object(fields) = parsed {
                    for (k, v) in fields {
                        if override_existing || !props.contains_key(&k) {
                            props.insert(k, v);
                        }
                    }
                }
                let computed_label = if !label.is_empty() {
                    label.clone()
                } else {
                    format!("{} {}", affiliation(&sidc_value[3..4]), symbol_set(&sidc_value[4..6]))
                };
                if !label_field.is_empty()
                    && !computed_label.is_empty()
                    && (override_existing || !props.contains_key(&label_field))
                {
                    props.insert(label_field.clone(), json!(computed_label));
                }
                feature["properties"] = Value::Object(props);
                tagged.push(feature);
            }
            Err(e) => {
                invalid += 1;
                props.insert("sidc_valid".into(), json!(false));
                props.insert("sidc_error".into(), json!(e));
                if !sidc_value.is_empty() {
                    props.insert("sidc".into(), json!(sidc_value));
                }
                feature["properties"] = Value::Object(props);
                rejected.push(feature.clone());
                tagged.push(feature);
            }
        }
    }

    let total = valid + invalid;
    let mut out = ToolOutput::new()
        .layer(&format!("{input_name} (MilSymbols)"), geojson::collection(tagged))
        .message(req.tr(
            MANIFEST,
            "Tagged {total} features: {valid} valid APP-6D SIDC, {invalid} invalid.",
            &[
                ("total", &total.to_string()),
                ("valid", &valid.to_string()),
                ("invalid", &invalid.to_string()),
            ],
        ));
    if !rejected.is_empty() {
        out = out.layer(&format!("{input_name} (MilSymbols Invalid SIDC)"), geojson::collection(rejected));
    }
    Ok(out.into())
}

// --- APP-6D SIDC parsing --------------------------------------------------------------------

/// Uppercase and drop whitespace. APP-6D SIDCs are 20 digits; validation happens in `classify`.
fn normalize_sidc(raw: &str) -> String {
    raw.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase()
}

/// An APP-6D SIDC is exactly 20 ASCII digits (which also means it can't start with '-').
fn is_valid_app6d(sidc: &str) -> bool {
    sidc.len() == 20 && sidc.bytes().all(|b| b.is_ascii_digit())
}

/// Parses a 20-digit APP-6D SIDC into a properties object, or returns why it is invalid.
fn classify(sidc: &str) -> Result<Value, String> {
    if sidc.is_empty() {
        return Err("SIDC is empty".into());
    }
    if !is_valid_app6d(sidc) {
        return Err("APP-6D SIDC must be exactly 20 digits (0-9)".into());
    }
    Ok(json!({
        "sidc_valid": true,
        "sidc_version": &sidc[0..2],
        "context_code": &sidc[2..3],
        "context": context(&sidc[2..3]),
        "affiliation_code": &sidc[3..4],
        "affiliation": affiliation(&sidc[3..4]),
        "symbol_set_code": &sidc[4..6],
        "symbol_set": symbol_set(&sidc[4..6]),
        "status_code": &sidc[6..7],
        "status": status(&sidc[6..7]),
        "hqtfd_code": &sidc[7..8],
        "hqtfd": hq_tf_dummy(&sidc[7..8]),
        "amplifier_code": &sidc[8..10],
        "entity_code": &sidc[10..16],
        "modifier1_code": &sidc[16..18],
        "modifier2_code": &sidc[18..20],
    }))
}

fn context(c: &str) -> &'static str {
    match c {
        "0" => "Reality",
        "1" => "Exercise",
        "2" => "Simulation",
        _ => "Unspecified",
    }
}

fn affiliation(c: &str) -> &'static str {
    match c {
        "0" => "Pending",
        "1" => "Unknown",
        "2" => "Assumed Friend",
        "3" => "Friend",
        "4" => "Neutral",
        "5" => "Suspect",
        "6" => "Hostile",
        _ => "Unspecified",
    }
}

fn symbol_set(code: &str) -> &'static str {
    match code {
        "01" => "Air",
        "02" => "Air Missile",
        "05" => "Space",
        "06" => "Space Missile",
        "10" => "Land Unit",
        "11" => "Land Civilian Unit/Organization",
        "15" => "Land Equipment",
        "20" => "Land Installation",
        "25" => "Control Measure",
        "27" => "Dismounted Individual",
        "30" => "Sea Surface",
        "35" => "Sea Subsurface",
        "36" => "Mine Warfare",
        "40" => "Activity",
        "45" => "Meteorological - Oceanographic",
        "50" => "Meteorological - Atmospheric",
        "51" => "Signals Intelligence - Space",
        "52" => "Signals Intelligence - Air",
        "53" => "Signals Intelligence - Land",
        "54" => "Signals Intelligence - Surface",
        "55" => "Signals Intelligence - Subsurface",
        "60" => "Cyberspace",
        _ => "Unspecified",
    }
}

fn status(c: &str) -> &'static str {
    match c {
        "0" => "Present",
        "1" => "Planned/Anticipated/Suspect",
        "2" => "Present/Fully Capable",
        "3" => "Present/Damaged",
        "4" => "Present/Destroyed",
        "5" => "Present/Full To Capacity",
        _ => "Unspecified",
    }
}

fn hq_tf_dummy(c: &str) -> &'static str {
    match c {
        "0" => "Not Applicable",
        "1" => "Feint/Dummy",
        "2" => "Headquarters",
        "3" => "Feint/Dummy Headquarters",
        "4" => "Task Force",
        "5" => "Feint/Dummy Task Force",
        "6" => "Task Force Headquarters",
        "7" => "Feint/Dummy Task Force Headquarters",
        _ => "Unspecified",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_20_digit_sidc() {
        assert!(is_valid_app6d("10031000121103000000"));
        assert!(!is_valid_app6d("1003100012110300000")); // 19 digits
        assert!(!is_valid_app6d("SFGPUCI-------------")); // legacy letter form
        assert!(!is_valid_app6d("-0031000121103000000")); // can't start with '-'
    }

    #[test]
    fn normalizes_whitespace_and_case() {
        assert_eq!(normalize_sidc("  1003 1000 1211 0300 0000 "), "10031000121103000000");
    }

    #[test]
    fn classifies_a_friendly_land_unit() {
        let v = classify("10031000121103000000").unwrap();
        assert_eq!(v["sidc_valid"], true);
        assert_eq!(v["context"], "Reality");
        assert_eq!(v["affiliation"], "Friend");
        assert_eq!(v["symbol_set"], "Land Unit");
        assert_eq!(v["status"], "Present");
    }

    #[test]
    fn rejects_empty_and_malformed() {
        assert!(classify("").is_err());
        assert!(classify("123").is_err());
        assert!(classify("SFGPUCI-------------").is_err());
    }
}
