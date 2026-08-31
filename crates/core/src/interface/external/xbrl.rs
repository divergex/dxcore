use reqwest::blocking::Client;
use reqwest::Url;
use serde::Deserialize;
use serde_json::Value;

use crate::Error;

const API_URL: &str = "https://filings.xbrl.org/api/filings";
const BASE_URL: &str = "https://filings.xbrl.org";

#[derive(Debug, Clone)]
pub struct XbrlFiling {
    pub entity_name: String,
    pub language: String,
    pub country: String,
    pub period_end: String,
    pub date_added: String,
    pub view_url: String,
}

#[derive(Debug, Deserialize)]
struct JsonApiResource {
    id: String,
    #[serde(rename = "type")]
    resource_type: String,
    attributes: Option<Value>,
    relationships: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct JsonApiResponse {
    data: Vec<JsonApiResource>,
    #[serde(default)]
    included: Vec<JsonApiResource>,
}

pub fn query_filings(queries: &[&str], limit: usize) -> Result<Vec<XbrlFiling>, Error> {
    let filter = build_filter(queries);
    let url = build_url(&filter, limit)?;

    let client = Client::new();
    let resp = client
        .get(url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (X11; Linux x86_64; rv:150.0) Gecko/20100101 Firefox/150.0",
        )
        .header("Accept", "application/json, text/javascript, */*; q=0.01")
        .header("X-Requested-With", "XMLHttpRequest")
        .header("Referer", "https://filings.xbrl.org/")
        .send()
        .map_err(|e| Error::Http(e.to_string()))?;

    let payload: JsonApiResponse = resp
        .json()
        .map_err(|e| Error::Http(format!("failed to parse response: {e}")))?;

    Ok(flatten_filings(payload))
}

fn build_filter(queries: &[&str]) -> Vec<Value> {
    let mut or_conditions = Vec::with_capacity(queries.len() * 4);
    for &q in queries {
        or_conditions.push(serde_json::json!({"name": "entity.name", "op": "ilike", "val": format!("%{q}%")}));
        or_conditions.push(serde_json::json!({"name": "entity.identifier", "op": "eq", "val": q}));
        or_conditions.push(serde_json::json!({"name": "language.name", "op": "ilike", "val": format!("{q}%")}));
        or_conditions.push(serde_json::json!({"name": "sha256", "op": "eq", "val": q}));
    }
    vec![
        serde_json::json!({"or": or_conditions}),
        serde_json::json!({"name": "input_filing.filing_source.program", "op": "ne", "val": "UAIFRS"}),
    ]
}

fn build_url(filter: &[Value], limit: usize) -> Result<Url, Error> {
    let filter_json =
        serde_json::to_string(filter).map_err(|e| Error::Http(format!("filter serialization: {e}")))?;

    Url::parse_with_params(
        API_URL,
        &[
            ("include", "entity,language"),
            ("filter", &filter_json),
            ("sort", "-date_added"),
            ("page[size]", &limit.to_string()),
            ("page[number]", "1"),
        ],
    )
    .map_err(|e| Error::Http(format!("invalid URL: {e}")))
}

fn flatten_filings(payload: JsonApiResponse) -> Vec<XbrlFiling> {
    let lookup: Vec<(&str, &JsonApiResource)> = payload
        .included
        .iter()
        .map(|r| (r.resource_type.as_str(), r))
        .collect();

    payload
        .data
        .into_iter()
        .map(|item| {
            let attrs = item.attributes.as_ref();
            let rels = item.relationships.as_ref();

            let entity_attrs = resolve_related(rels, "entity", &lookup);
            let language_attrs = resolve_related(rels, "language", &lookup);

            let entity_name = entity_attrs
                .and_then(|a| a.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("\u{2014}");
            let country = attrs
                .and_then(|a| a.get("country"))
                .and_then(|v| v.as_str())
                .or_else(|| entity_attrs.and_then(|a| a.get("country")).and_then(|v| v.as_str()))
                .unwrap_or("\u{2014}");
            let language = language_attrs
                .and_then(|a| a.get("name"))
                .and_then(|v| v.as_str())
                .or_else(|| attrs.and_then(|a| a.get("language")).and_then(|v| v.as_str()))
                .unwrap_or("\u{2014}");
            let period_end = attrs
                .and_then(|a| a.get("period_end"))
                .and_then(|v| v.as_str())
                .unwrap_or("\u{2014}");
            let date_added = attrs
                .and_then(|a| a.get("date_added"))
                .and_then(|v| v.as_str())
                .unwrap_or("\u{2014}");

            XbrlFiling {
                entity_name: entity_name.to_string(),
                language: language.to_string(),
                country: country.to_string(),
                period_end: period_end.to_string(),
                date_added: date_added.to_string(),
                view_url: format!("{BASE_URL}/{}", item.id),
            }
        })
        .collect()
}

fn resolve_related<'a>(
    rels: Option<&Value>,
    name: &str,
    lookup: &[(&str, &'a JsonApiResource)],
) -> Option<&'a Value> {
    let data = rels?
        .get(name)?
        .get("data")?;
    let rel_type = data.get("type")?.as_str()?;
    let rel_id = data.get("id")?.as_str()?;
    lookup
        .iter()
        .find(|(t, r)| *t == rel_type && r.id == rel_id)
        .and_then(|(_, r)| r.attributes.as_ref())
}
