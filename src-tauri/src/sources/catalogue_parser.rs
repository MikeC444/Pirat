use crate::models::CatalogueGame;
use serde_json::Value;

#[derive(Debug, Default)]
pub struct ParseResult {
    pub games: Vec<CatalogueGame>,
    /// Human readable, non-fatal problems found while parsing (missing/mistyped
    /// optional fields, skipped entries, ...). The catalogue is still usable even
    /// when this is non-empty.
    pub errors: Vec<String>,
}

/// Parses a raw catalogue JSON payload into a list of games.
///
/// Treats the payload as untrusted input: unknown/extra fields are ignored,
/// missing optional fields fall back to sane defaults, and a single malformed
/// game entry is skipped (with a recorded error) rather than failing the
/// whole catalogue. Only a structurally invalid top level (not an object, or
/// no `games` array) is a hard failure.
pub fn parse_catalogue(raw: &str) -> Result<ParseResult, String> {
    let value: Value = serde_json::from_str(raw)
        .map_err(|e| format!("catalogue is not valid JSON: {e}"))?;

    let root = value
        .as_object()
        .ok_or_else(|| "catalogue root must be a JSON object".to_string())?;

    let games_value = root
        .get("games")
        .ok_or_else(|| "catalogue is missing required top-level \"games\" array".to_string())?;

    let games_array = games_value
        .as_array()
        .ok_or_else(|| "\"games\" must be an array".to_string())?;

    let mut result = ParseResult::default();

    for (idx, entry) in games_array.iter().enumerate() {
        match parse_game(idx, entry, &mut result.errors) {
            Some(game) => result.games.push(game),
            None => continue,
        }
    }

    Ok(result)
}

fn parse_game(idx: usize, entry: &Value, errors: &mut Vec<String>) -> Option<CatalogueGame> {
    let obj = match entry.as_object() {
        Some(obj) => obj,
        None => {
            errors.push(format!("game[{idx}]: expected an object, skipping entry"));
            return None;
        }
    };

    let external_id = match obj.get("id").and_then(Value::as_str) {
        Some(s) if !s.trim().is_empty() => s.to_string(),
        _ => {
            errors.push(format!("game[{idx}]: missing or invalid required field \"id\", skipping entry"));
            return None;
        }
    };

    let title = match obj.get("title").and_then(Value::as_str) {
        Some(s) if !s.trim().is_empty() => s.to_string(),
        _ => {
            errors.push(format!(
                "game[{idx}] (id={external_id}): missing or invalid required field \"title\", skipping entry"
            ));
            return None;
        }
    };

    let description = string_field(obj, "description", idx, &external_id, errors).unwrap_or_default();
    let cover_url = optional_string_field(obj, "cover_url", idx, &external_id, errors);
    let version = optional_string_field(obj, "version", idx, &external_id, errors);
    let release_date = optional_string_field(obj, "release_date", idx, &external_id, errors);
    let developer = optional_string_field(obj, "developer", idx, &external_id, errors);
    let publisher = optional_string_field(obj, "publisher", idx, &external_id, errors);
    let download_url = optional_string_field(obj, "download_url", idx, &external_id, errors);
    let checksum = optional_string_field(obj, "checksum", idx, &external_id, errors);
    let checksum_algo = optional_string_field(obj, "checksum_algo", idx, &external_id, errors);

    let size_bytes = match obj.get("size_bytes") {
        None | Some(Value::Null) => None,
        Some(v) => match v.as_i64() {
            Some(n) if n >= 0 => Some(n),
            _ => {
                errors.push(format!(
                    "game[{idx}] (id={external_id}): \"size_bytes\" must be a non-negative number, ignoring value"
                ));
                None
            }
        },
    };

    let genres = match obj.get("genres") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => {
            let mut out = Vec::new();
            for item in items {
                match item.as_str() {
                    Some(s) => out.push(s.to_string()),
                    None => errors.push(format!(
                        "game[{idx}] (id={external_id}): non-string entry in \"genres\", skipping that entry"
                    )),
                }
            }
            out
        }
        Some(_) => {
            errors.push(format!(
                "game[{idx}] (id={external_id}): \"genres\" must be an array of strings, defaulting to []"
            ));
            Vec::new()
        }
    };

    Some(CatalogueGame {
        external_id,
        title,
        description,
        cover_url,
        version,
        size_bytes,
        release_date,
        genres,
        developer,
        publisher,
        download_url,
        checksum,
        checksum_algo,
    })
}

fn string_field(
    obj: &serde_json::Map<String, Value>,
    key: &str,
    idx: usize,
    id: &str,
    errors: &mut Vec<String>,
) -> Option<String> {
    match obj.get(key) {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.clone()),
        Some(_) => {
            errors.push(format!(
                "game[{idx}] (id={id}): \"{key}\" must be a string, ignoring value"
            ));
            None
        }
    }
}

fn optional_string_field(
    obj: &serde_json::Map<String, Value>,
    key: &str,
    idx: usize,
    id: &str,
    errors: &mut Vec<String>,
) -> Option<String> {
    string_field(obj, key, idx, id, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_well_formed_catalogue() {
        let raw = r#"{
            "games": [
                {"id": "g1", "title": "Alpha", "genres": ["Action"], "size_bytes": 1024}
            ]
        }"#;
        let result = parse_catalogue(raw).unwrap();
        assert_eq!(result.games.len(), 1);
        assert!(result.errors.is_empty());
        assert_eq!(result.games[0].title, "Alpha");
        assert_eq!(result.games[0].size_bytes, Some(1024));
    }

    #[test]
    fn fills_in_missing_optional_fields() {
        let raw = r#"{"games": [{"id": "g1", "title": "Minimal"}]}"#;
        let result = parse_catalogue(raw).unwrap();
        assert_eq!(result.games.len(), 1);
        assert!(result.errors.is_empty());
        let g = &result.games[0];
        assert_eq!(g.description, "");
        assert_eq!(g.genres, Vec::<String>::new());
        assert_eq!(g.size_bytes, None);
    }

    #[test]
    fn skips_entries_missing_required_fields_but_keeps_rest() {
        let raw = r#"{"games": [
            {"title": "No id here"},
            {"id": "g2", "title": "Valid"}
        ]}"#;
        let result = parse_catalogue(raw).unwrap();
        assert_eq!(result.games.len(), 1);
        assert_eq!(result.games[0].external_id, "g2");
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].contains("missing or invalid required field \"id\""));
    }

    #[test]
    fn tolerates_wrong_typed_optional_fields() {
        let raw = r#"{"games": [
            {"id": "g1", "title": "Weird", "size_bytes": "not a number", "genres": "not an array"}
        ]}"#;
        let result = parse_catalogue(raw).unwrap();
        assert_eq!(result.games.len(), 1);
        assert_eq!(result.games[0].size_bytes, None);
        assert_eq!(result.games[0].genres, Vec::<String>::new());
        assert_eq!(result.errors.len(), 2);
    }

    #[test]
    fn rejects_invalid_json() {
        let err = parse_catalogue("not json").unwrap_err();
        assert!(err.contains("not valid JSON"));
    }

    #[test]
    fn rejects_missing_games_array() {
        let err = parse_catalogue(r#"{"foo": 1}"#).unwrap_err();
        assert!(err.contains("missing required top-level"));
    }

    #[test]
    fn rejects_non_array_games() {
        let err = parse_catalogue(r#"{"games": "nope"}"#).unwrap_err();
        assert!(err.contains("must be an array"));
    }
}
