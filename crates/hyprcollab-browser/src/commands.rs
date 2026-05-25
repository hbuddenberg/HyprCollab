use std::{future::Future, pin::Pin};

use serde_json::Value;

use crate::{
    engine::BrowserEngine,
    errors::{BrowserError, Result},
};

pub trait BrowserCommand: Send + Sync {
    fn name(&self) -> &str;
    fn execute<'a>(
        &'a self,
        engine: &'a BrowserEngine,
        args: Value,
    ) -> Pin<Box<dyn Future<Output = Result<Value>> + Send + 'a>>;
}

fn require_str<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args[key]
        .as_str()
        .ok_or_else(|| BrowserError::Parse(format!("missing field: {key}")))
}

pub struct WebNavigate;

impl BrowserCommand for WebNavigate {
    fn name(&self) -> &str {
        "web_navigate"
    }

    fn execute<'a>(
        &'a self,
        engine: &'a BrowserEngine,
        args: Value,
    ) -> Pin<Box<dyn Future<Output = Result<Value>> + Send + 'a>> {
        Box::pin(async move {
            let session_id = require_str(&args, "session_id")?;
            let url = require_str(&args, "url")?;
            let snapshot = engine.navigate(session_id, url).await?;
            Ok(serde_json::to_value(snapshot)?)
        })
    }
}

pub struct WebScreenshot;

impl BrowserCommand for WebScreenshot {
    fn name(&self) -> &str {
        "web_screenshot"
    }

    fn execute<'a>(
        &'a self,
        engine: &'a BrowserEngine,
        args: Value,
    ) -> Pin<Box<dyn Future<Output = Result<Value>> + Send + 'a>> {
        Box::pin(async move {
            let session_id = require_str(&args, "session_id")?;
            let bytes = engine.screenshot(session_id).await?;
            Ok(serde_json::to_value(bytes)?)
        })
    }
}

pub struct WebExtract;

impl BrowserCommand for WebExtract {
    fn name(&self) -> &str {
        "web_extract"
    }

    fn execute<'a>(
        &'a self,
        engine: &'a BrowserEngine,
        args: Value,
    ) -> Pin<Box<dyn Future<Output = Result<Value>> + Send + 'a>> {
        Box::pin(async move {
            let session_id = require_str(&args, "session_id")?;
            let selector = require_str(&args, "selector")?;
            let text = engine.extract(session_id, selector).await?;
            Ok(Value::String(text))
        })
    }
}

pub struct WebInteract;

impl BrowserCommand for WebInteract {
    fn name(&self) -> &str {
        "web_interact"
    }

    fn execute<'a>(
        &'a self,
        engine: &'a BrowserEngine,
        args: Value,
    ) -> Pin<Box<dyn Future<Output = Result<Value>> + Send + 'a>> {
        Box::pin(async move {
            let session_id = require_str(&args, "session_id")?;
            let action = require_str(&args, "action")?;
            let target = require_str(&args, "target")?;
            let value = args["value"].as_str();
            engine.interact(session_id, action, target, value).await?;
            Ok(Value::Bool(true))
        })
    }
}

pub struct WebSearch;

impl BrowserCommand for WebSearch {
    fn name(&self) -> &str {
        "web_search"
    }

    fn execute<'a>(
        &'a self,
        engine: &'a BrowserEngine,
        args: Value,
    ) -> Pin<Box<dyn Future<Output = Result<Value>> + Send + 'a>> {
        Box::pin(async move {
            let query = require_str(&args, "query")?;
            let results = engine.search(query).await?;
            Ok(serde_json::to_value(results)?)
        })
    }
}

pub fn all_commands() -> Vec<Box<dyn BrowserCommand>> {
    vec![
        Box::new(WebNavigate),
        Box::new(WebScreenshot),
        Box::new(WebExtract),
        Box::new(WebInteract),
        Box::new(WebSearch),
    ]
}
