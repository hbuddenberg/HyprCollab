use std::collections::HashMap;

use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkInfo {
    pub text: String,
    pub href: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageSnapshot {
    pub url: String,
    pub title: Option<String>,
    pub text_content: String,
    pub html: Option<String>,
    pub links: Vec<LinkInfo>,
    pub metadata: HashMap<String, String>,
    pub screenshot: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

pub(crate) fn parse_html(html: &str, url: &str) -> PageSnapshot {
    let document = Html::parse_document(html);
    PageSnapshot {
        url: url.to_string(),
        title: extract_title(&document),
        text_content: extract_text(&document),
        links: extract_links(&document, url),
        metadata: extract_metadata(&document),
        html: Some(html.to_string()),
        screenshot: None,
    }
}

fn extract_title(document: &Html) -> Option<String> {
    let sel = Selector::parse("title").expect("valid selector");
    document
        .select(&sel)
        .next()
        .map(|el| el.text().collect::<String>())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn extract_text(document: &Html) -> String {
    let sel =
        Selector::parse("p, h1, h2, h3, h4, h5, h6, li, td, th, article, main, section")
            .expect("valid selector");
    document
        .select(&sel)
        .map(|el| {
            el.text()
                .collect::<Vec<_>>()
                .join(" ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn extract_links(document: &Html, base_url: &str) -> Vec<LinkInfo> {
    let sel = Selector::parse("a[href]").expect("valid selector");
    let base = Url::parse(base_url).ok();
    document
        .select(&sel)
        .filter_map(|el| {
            let href = el.value().attr("href")?;
            let text = el.text().collect::<String>().trim().to_string();
            let resolved = match &base {
                Some(b) => b.join(href).map(|u| u.to_string()).unwrap_or_else(|_| href.to_string()),
                None => href.to_string(),
            };
            Some(LinkInfo { text, href: resolved })
        })
        .collect()
}

fn extract_metadata(document: &Html) -> HashMap<String, String> {
    let sel = Selector::parse("meta").expect("valid selector");
    let mut meta = HashMap::new();
    for el in document.select(&sel) {
        let name = el.value().attr("name").or_else(|| el.value().attr("property"));
        let content = el.value().attr("content");
        if let (Some(n), Some(c)) = (name, content) {
            meta.insert(n.to_string(), c.to_string());
        }
    }
    meta
}

pub(crate) fn parse_search_results(html: &str) -> Vec<SearchResult> {
    let document = Html::parse_document(html);
    let result_sel = Selector::parse(".result").expect("valid selector");
    let title_sel = Selector::parse(".result__title a").expect("valid selector");
    let snippet_sel = Selector::parse(".result__snippet").expect("valid selector");

    document
        .select(&result_sel)
        .filter_map(|result| {
            let title_el = result.select(&title_sel).next()?;
            let title = title_el.text().collect::<String>().trim().to_string();
            let url = title_el.value().attr("href")?.to_string();
            let snippet = result
                .select(&snippet_sel)
                .next()
                .map(|el| el.text().collect::<String>().trim().to_string())
                .unwrap_or_default();
            Some(SearchResult { title, url, snippet })
        })
        .take(10)
        .collect()
}
