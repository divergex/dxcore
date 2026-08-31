use reqwest::blocking::Client;
use reqwest::Url;
use serde::Deserialize;

use crate::Error;

const API_BASE: &str = "https://content.guardianapis.com";

#[derive(Debug, Clone, Deserialize)]
pub struct GuardianArticle {
    pub id: String,
    #[serde(rename = "type")]
    pub article_type: String,
    pub section_id: Option<String>,
    pub section_name: Option<String>,
    #[serde(rename = "webPublicationDate")]
    pub web_publication_date: String,
    #[serde(rename = "webTitle")]
    pub web_title: String,
    #[serde(rename = "webUrl")]
    pub web_url: String,
    #[serde(rename = "apiUrl")]
    pub api_url: String,
    #[serde(rename = "pillarName")]
    pub pillar_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GuardianBlock {
    pub id: String,
    #[serde(rename = "bodyHtml")]
    pub body_html: String,
    #[serde(rename = "bodyTextSummary")]
    pub body_text_summary: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GuardianArticleBody {
    pub id: String,
    #[serde(rename = "webPublicationDate")]
    pub web_publication_date: String,
    #[serde(rename = "webTitle")]
    pub web_title: String,
    #[serde(rename = "webUrl")]
    pub web_url: String,
    pub blocks: Option<GuardianBlocks>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GuardianBlocks {
    pub body: Option<Vec<GuardianBlock>>,
}

#[derive(Debug, Deserialize)]
struct SearchResults {
    results: Vec<GuardianArticle>,
}

#[derive(Debug, Deserialize)]
struct ContentWrapper {
    content: GuardianArticleBody,
}

#[derive(Debug, Deserialize)]
struct ApiEnvelope<T> {
    response: T,
}

pub fn search(
    query: &str,
    api_key: &str,
    page_size: usize,
) -> Result<Vec<GuardianArticle>, Error> {
    let url = Url::parse_with_params(
        &format!("{API_BASE}/search"),
        &[
            ("api-key", api_key),
            ("q", query),
            ("page-size", &page_size.to_string()),
            ("order-by", "newest"),
        ],
    )
    .map_err(|e| Error::Http(format!("invalid URL: {e}")))?;

    let client = Client::new();
    let envelope: ApiEnvelope<SearchResults> = client
        .get(url)
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.json())
        .map_err(|e| Error::Http(e.to_string()))?;

    Ok(envelope.response.results)
}

pub fn get_article(id: &str, api_key: &str) -> Result<GuardianArticleBody, Error> {
    let url = Url::parse_with_params(
        &format!("{API_BASE}/{id}"),
        &[
            ("api-key", api_key),
            ("show-blocks", "body"),
        ],
    )
    .map_err(|e| Error::Http(format!("invalid URL: {e}")))?;

    let client = Client::new();
    let envelope: ApiEnvelope<ContentWrapper> = client
        .get(url)
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.json())
        .map_err(|e| Error::Http(e.to_string()))?;

    Ok(envelope.response.content)
}
