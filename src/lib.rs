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
        (Kind::Command, "catalog") => catalog(&req),
        (Kind::Command, "symbol-editor") => symbol_editor(&req),
        (Kind::Command, "modifier-editor") => modifier_editor(&req),
        (Kind::Command, "settings") => settings(&req),
        (Kind::Command, "symbol-editor-state") => symbol_editor_state(&req),
        (Kind::Command, "style-app6") => style_app6(&req),
        (Kind::Tool, "tag-layer") => tag_layer(&req),
        (_, other) => Err(format!("Military Symbols Library has no '{other}'")),
    }
}

rustgis_plugin::export!(handle);

// --- Commands -------------------------------------------------------------------------------

const APP6_CATALOG: &[(&str, &str)] = &[
    ("Land Unit", "Land, air, sea and civil units; main tactical symbol family."),
    ("Land Equipment", "Fixed and mobile equipment, installations and support assets."),
    ("Installations", "Command, logistics, engineering and infrastructure sites."),
    ("Control Measures", "Boundaries, fire support, barriers, alerts and planning graphics."),
    ("Sea Surface", "Naval surface units, platforms and tactical formations."),
    ("Air", "Air platforms and mission-related airborne symbols."),
    ("Space", "Space systems and satellites."),
    ("Activity", "Operational activities, events and exercises."),
];

const APP6_CONTEXT_LABELS: &[&str] = &["Reality", "Exercise", "Simulation"];
const APP6_MODIFIER_LABELS: &[&str] = &["None", "HQ", "TF", "Dummy"];
const APP6_SYMBOL_SET_TABLE: &[(&str, &str, &str)] = &[
    ("10", "Land Unit", "Land unit categories and formations"),
    ("15", "Land Equipment", "Equipment, vehicles and fixed assets"),
    ("20", "Installations", "Command, support and civil infrastructure"),
    ("25", "Control Measures", "Boundaries, fire support and plans"),
    ("30", "Sea Surface", "Naval surface platforms and units"),
    ("01", "Air", "Air platforms and airborne activity"),
    ("05", "Space", "Space systems and satellites"),
    ("40", "Activity", "Operations, activities and events"),
];
const APP6_ENTITY_TABLE: &[(&str, &str)] = &[
    ("10", "100100"),
    ("15", "170000"),
    ("20", "220000"),
    ("25", "250000"),
    ("30", "300000"),
    ("01", "011000"),
    ("05", "050000"),
    ("40", "400000"),
];
const APP6_QUALIFIER_TABLE: &[(&str, &str, &str)] = &[
    ("0", "None", "No qualifier"),
    ("1", "Feint/Dummy", "Feint or dummy indicator"),
    ("2", "Headquarters", "HQ / command node"),
    ("4", "Task Force", "Task force indicator"),
    ("6", "Task Force HQ", "Task force headquarters"),
];

fn symbol_set_code_for_family(family_index: usize) -> &'static str {
    APP6_SYMBOL_SET_TABLE
        .get(family_index.min(APP6_SYMBOL_SET_TABLE.len() - 1))
        .map(|(code, _, _)| *code)
        .unwrap_or("10")
}

fn entity_code_for_family(family_index: usize) -> &'static str {
    APP6_ENTITY_TABLE
        .get(family_index.min(APP6_ENTITY_TABLE.len() - 1))
        .map(|(_, code)| *code)
        .unwrap_or("100100")
}

fn qualifier_code_for_modifier(modifier_index: usize) -> &'static str {
    match modifier_index {
        0 => "0",
        1 => "2",
        2 => "4",
        3 => "1",
        _ => "0",
    }
}

fn build_sidc_from_values(family_index: usize, affiliation_index: usize, status_index: usize, modifier_index: usize, context_index: usize, amplifier_index: usize) -> String {
    let version = "10";
    let context = format!("{}", (context_index).min(APP6_CONTEXT_LABELS.len() - 1));
    let affiliation = format!("{}", (affiliation_index).min(6));
    let symbol_set = symbol_set_code_for_family(family_index);
    let status = format!("{}", (status_index).min(5));
    let qualifier = qualifier_code_for_modifier(modifier_index);
    let amplifier = format!("{:02}", (amplifier_index).min(99));
    let entity = entity_code_for_family(family_index);
    let modifier1 = format!("{:02}", ((status_index + 1) * (affiliation_index + 1)) % 100);
    let modifier2 = format!("{:02}", ((modifier_index + 1) * (family_index + 1)) % 100);
    format!("{version}{context}{affiliation}{symbol_set}{status}{qualifier}{amplifier}{entity}{modifier1}{modifier2}")
}

fn panel_values(req: &Request) -> Map<String, Value> {
    req.params
        .get("panel")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn about(req: &Request) -> Result<Value, String> {
    host::notify(&req.tr(
        MANIFEST,
        "MilSymbols: APP-6D SIDC tagging. Run 'Tag Layer With APP-6D SIDC', then 'Style by Affiliation'.",
        &[],
    ));
    Ok(json!({"plugin": "milsymb", "standard": "APP-6D", "status": "ready"}))
}

fn symbol_editor_state(req: &Request) -> Result<Value, String> {
    let panel = panel_values(req);
    let family_index = panel
        .get("catalog-family")
        .and_then(Value::as_i64)
        .unwrap_or(0) as usize;
    let affiliation_index = panel
        .get("affiliation")
        .and_then(Value::as_i64)
        .unwrap_or(3) as usize;
    let status_index = panel
        .get("status")
        .and_then(Value::as_i64)
        .unwrap_or(0) as usize;
    let modifier_index = panel
        .get("modifier")
        .and_then(Value::as_i64)
        .unwrap_or(0) as usize;
    let context_index = panel
        .get("context")
        .and_then(Value::as_i64)
        .unwrap_or(0) as usize;
    let amplifier_index = panel
        .get("amplifier")
        .and_then(Value::as_i64)
        .unwrap_or(0) as usize;
    let strict_validation = panel
        .get("strict-validation")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    let family = APP6_CATALOG.get(family_index).map(|(name, _)| *name).unwrap_or("Land Unit");
    let affiliation = ["Pending", "Unknown", "Assumed Friend", "Friend", "Neutral", "Suspect", "Hostile"][affiliation_index.min(6)];
    let status = ["Present", "Planned", "Fully Capable", "Damaged", "Destroyed", "Full To Capacity"][status_index.min(5)];
    let modifier = APP6_MODIFIER_LABELS[modifier_index.min(APP6_MODIFIER_LABELS.len() - 1)];
    let sidc = build_sidc_from_values(family_index, affiliation_index, status_index, modifier_index, context_index, amplifier_index);

    Ok(json!({
        "panel": {
            "values": {
                "catalog-family": family_index,
                "context": context_index,
                "affiliation": affiliation_index,
                "status": status_index,
                "modifier": modifier_index,
                "amplifier": amplifier_index,
                "strict-validation": strict_validation
            },
            "items": {
                "catalog-family": {"min": 0, "max": APP6_CATALOG.len() as i64 - 1, "labels": APP6_CATALOG.iter().map(|(name, _)| *name).collect::<Vec<_>>()},
                "context": {"min": 0, "max": 2, "labels": APP6_CONTEXT_LABELS},
                "affiliation": {"min": 0, "max": 6, "labels": ["Pending", "Unknown", "Assumed Friend", "Friend", "Neutral", "Suspect", "Hostile"]},
                "status": {"min": 0, "max": 5, "labels": ["Present", "Planned", "Fully Capable", "Damaged", "Destroyed", "Full To Capacity"]},
                "modifier": {"min": 0, "max": 3, "labels": APP6_MODIFIER_LABELS},
                "amplifier": {"min": 0, "max": 99, "labels": ["00", "01", "02", "03", "04", "05", "06", "07", "08", "09"]}
            },
            "text": {
                "catalog-section": format!("Selected family: {}", family),
                "editor-section": format!("Selected context: {} / affiliation: {} / status: {} / modifier: {}", APP6_CONTEXT_LABELS[context_index.min(APP6_CONTEXT_LABELS.len() - 1)], affiliation, status, modifier),
                "sidc-preview": format!("Generated SIDC: {}", sidc)
            }
        },
        "selectedFamily": family,
        "selectedSymbolSetCode": symbol_set_code_for_family(family_index),
        "selectedEntityCode": entity_code_for_family(family_index),
        "qualifierCode": qualifier_code_for_modifier(modifier_index),
        "context": APP6_CONTEXT_LABELS[context_index.min(APP6_CONTEXT_LABELS.len() - 1)],
        "affiliation": affiliation,
        "status": status,
        "modifier": modifier,
        "sidc": sidc,
        "symbolSetTable": APP6_SYMBOL_SET_TABLE,
        "entityTable": APP6_ENTITY_TABLE,
        "qualifierTable": APP6_QUALIFIER_TABLE,
        "strictValidation": strict_validation
    }))
}

fn catalog(req: &Request) -> Result<Value, String> {
    let panel = panel_values(req);
    let family_index = panel
        .get("catalog-family")
        .and_then(Value::as_i64)
        .unwrap_or(0) as usize;
    let entries: Vec<Value> = APP6_CATALOG
        .iter()
        .enumerate()
        .map(|(index, (family, description))| {
            json!({
                "index": index,
                "family": *family,
                "description": *description,
                "selected": index == family_index,
                "source": "APP-6D",
                "rendering": "schematic"
            })
        })
        .collect();
    host::notify(&req.tr(MANIFEST, "APP-6D symbol catalog loaded.", &[]));
    Ok(json!({
        "catalog": entries,
        "selectedFamily": APP6_CATALOG.get(family_index).map(|(name, _)| *name).unwrap_or("Land Unit"),
        "totalFamilies": entries.len(),
        "standard": "APP-6D",
        "renderingMode": "schematic-affiliation",
        "panel": {
            "values": { "catalog-family": family_index }
        }
    }))
}

fn symbol_editor(req: &Request) -> Result<Value, String> {
    let panel = panel_values(req);
    let family_index = panel
        .get("catalog-family")
        .and_then(Value::as_i64)
        .unwrap_or(0) as usize;
    let affiliation_index = panel
        .get("affiliation")
        .and_then(Value::as_i64)
        .unwrap_or(3) as usize;
    let status_index = panel
        .get("status")
        .and_then(Value::as_i64)
        .unwrap_or(0) as usize;
    let modifier_index = panel
        .get("modifier")
        .and_then(Value::as_i64)
        .unwrap_or(0) as usize;
    let context_index = panel
        .get("context")
        .and_then(Value::as_i64)
        .unwrap_or(0) as usize;
    let amplifier_index = panel
        .get("amplifier")
        .and_then(Value::as_i64)
        .unwrap_or(0) as usize;
    let generated_sidc = build_sidc_from_values(family_index, affiliation_index, status_index, modifier_index, context_index, amplifier_index);
    let parsed = classify(&generated_sidc).map_err(|e| format!("generated SIDC invalid: {e}"))?;
    let affiliation = ["Pending", "Unknown", "Assumed Friend", "Friend", "Neutral", "Suspect", "Hostile"][affiliation_index.min(6)];
    let status = ["Present", "Planned", "Fully Capable", "Damaged", "Destroyed", "Full To Capacity"][status_index.min(5)];

    host::notify(&req.tr(MANIFEST, "APP-6D symbol editor ready: generated SIDC {generated_sidc}", &[("generated_sidc", &generated_sidc)]));
    Ok(json!({
        "editor": "symbol-editor",
        "sampleSidc": generated_sidc,
        "generatedSidc": generated_sidc,
        "selectedFamily": APP6_CATALOG.get(family_index).map(|(name, _)| *name).unwrap_or("Land Unit"),
        "selectedSymbolSetCode": symbol_set_code_for_family(family_index),
        "selectedEntityCode": entity_code_for_family(family_index),
        "selectedAffiliation": affiliation,
        "selectedStatus": status,
        "selectedModifier": APP6_MODIFIER_LABELS[modifier_index.min(APP6_MODIFIER_LABELS.len() - 1)],
        "qualifierCode": qualifier_code_for_modifier(modifier_index),
        "parsed": parsed,
        "panel": {
            "values": {
                "catalog-family": family_index,
                "context": context_index,
                "affiliation": affiliation_index,
                "status": status_index,
                "modifier": modifier_index,
                "amplifier": amplifier_index
            }
        },
        "symbolSetTable": APP6_SYMBOL_SET_TABLE,
        "entityTable": APP6_ENTITY_TABLE,
        "qualifierTable": APP6_QUALIFIER_TABLE,
        "fields": [
            "sidc",
            "context",
            "affiliation",
            "symbol_set",
            "status",
            "hqtfd",
            "amplifier_code",
            "entity_code",
            "modifier1_code",
            "modifier2_code"
        ]
    }))
}

fn modifier_editor(req: &Request) -> Result<Value, String> {
    let panel = panel_values(req);
    let family_index = panel
        .get("catalog-family")
        .and_then(Value::as_i64)
        .unwrap_or(0) as usize;
    let affiliation_index = panel
        .get("affiliation")
        .and_then(Value::as_i64)
        .unwrap_or(3) as usize;
    let status_index = panel
        .get("status")
        .and_then(Value::as_i64)
        .unwrap_or(0) as usize;
    let modifier_index = panel
        .get("modifier")
        .and_then(Value::as_i64)
        .unwrap_or(0) as usize;
    let context_index = panel
        .get("context")
        .and_then(Value::as_i64)
        .unwrap_or(0) as usize;
    let amplifier_index = panel
        .get("amplifier")
        .and_then(Value::as_i64)
        .unwrap_or(0) as usize;
    let modifier = APP6_MODIFIER_LABELS[modifier_index.min(APP6_MODIFIER_LABELS.len() - 1)];
    let sidc = build_sidc_from_values(family_index, affiliation_index, status_index, modifier_index, context_index, amplifier_index);
    let modifiers = json!([
        {"code": "0", "label": "None", "description": "No qualifier"},
        {"code": "2", "label": "HQ", "description": "Headquarters / command indicator"},
        {"code": "4", "label": "TF", "description": "Task force indicator"},
        {"code": "1", "label": "Dummy", "description": "Feint / dummy indicator"}
    ]);
    host::notify(&req.tr(MANIFEST, "APP-6D modifier editor ready: SIDC {sidc}", &[("sidc", &sidc)]));
    Ok(json!({
        "editor": "modifier-editor",
        "selectedModifier": modifier,
        "selectedQualifierCode": qualifier_code_for_modifier(modifier_index),
        "generatedSidc": sidc,
        "modifiers": modifiers,
        "qualifierTable": APP6_QUALIFIER_TABLE,
        "panel": {
            "values": {
                "catalog-family": family_index,
                "context": context_index,
                "affiliation": affiliation_index,
                "status": status_index,
                "modifier": modifier_index,
                "amplifier": amplifier_index
            }
        },
        "supportedFields": ["hq_tf_dummy", "amplifier_code", "modifier1_code", "modifier2_code"]
    }))
}

fn settings(req: &Request) -> Result<Value, String> {
    let panel = panel_values(req);
    let family_index = panel
        .get("catalog-family")
        .and_then(Value::as_i64)
        .unwrap_or(0) as usize;
    let affiliation_index = panel
        .get("affiliation")
        .and_then(Value::as_i64)
        .unwrap_or(3) as usize;
    let status_index = panel
        .get("status")
        .and_then(Value::as_i64)
        .unwrap_or(0) as usize;
    let modifier_index = panel
        .get("modifier")
        .and_then(Value::as_i64)
        .unwrap_or(0) as usize;
    let context_index = panel
        .get("context")
        .and_then(Value::as_i64)
        .unwrap_or(0) as usize;
    let amplifier_index = panel
        .get("amplifier")
        .and_then(Value::as_i64)
        .unwrap_or(0) as usize;
    let strict_validation = panel
        .get("strict-validation")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let generated_sidc = build_sidc_from_values(family_index, affiliation_index, status_index, modifier_index, context_index, amplifier_index);
    let defaults = json!({
        "standard": "APP-6D",
        "defaultColorMode": "affiliation",
        "sidcValidation": if strict_validation { "strict-20-digit" } else { "relaxed" },
        "allowOverrideExistingFields": true,
        "defaultLabelField": "symbol_label",
        "defaultSidcField": "sidc",
        "symbolRenderMode": "schematic-affiliation",
        "generatedSidc": generated_sidc
    });
    host::notify(&req.tr(MANIFEST, "APP-6D settings loaded with SIDC {generated_sidc}.", &[("generated_sidc", &generated_sidc)]));
    Ok(json!({
        "settings": defaults,
        "generatedSidc": generated_sidc,
        "strictValidation": strict_validation,
        "plugin": "milsymb",
        "version": "0.2.0",
        "panel": { "values": { "context": context_index, "affiliation": affiliation_index, "status": status_index, "modifier": modifier_index, "amplifier": amplifier_index, "strict-validation": strict_validation } }
    }))
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
    fn builds_a_complete_sidc_from_editor_state() {
        let sidc = build_sidc_from_values(0, 3, 0, 1, 0, 0);
        assert_eq!(sidc.len(), 20);
        assert!(is_valid_app6d(&sidc));
        assert_eq!(&sidc[3..4], "3");
        assert_eq!(&sidc[4..6], "10");
        assert_eq!(&sidc[6..7], "2");
        assert_eq!(entity_code_for_family(0), "100100");
        assert_eq!(qualifier_code_for_modifier(1), "2");
    }

    #[test]
    fn rejects_empty_and_malformed() {
        assert!(classify("").is_err());
        assert!(classify("123").is_err());
        assert!(classify("SFGPUCI-------------").is_err());
    }
}
