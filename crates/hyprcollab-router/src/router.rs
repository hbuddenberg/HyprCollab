use std::collections::HashMap;
use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;

use hyprcollab_core::errors::{CoreError, Result};
use hyprcollab_core::traits::LlmProvider;
use hyprcollab_core::types::*;

/// Multi-provider LLM router.
///
/// Routes requests to the appropriate provider based on model name.
/// Supports `provider/model` format (e.g. `anthropic/claude-sonnet-4`, `ollama/llama3`).
/// Falls back to a default provider if no prefix is given.
pub struct LlmRouter {
    providers: HashMap<String, Box<dyn LlmProvider>>,
    default_provider: Option<String>,
    /// Model name aliases (e.g. "gpt4" -> "openai/gpt-4o")
    aliases: HashMap<String, String>,
}

impl LlmRouter {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            default_provider: None,
            aliases: HashMap::new(),
        }
    }

    /// Register a provider with a given name.
    pub fn register(&mut self, name: impl Into<String>, provider: Box<dyn LlmProvider>) {
        self.providers.insert(name.into(), provider);
    }

    /// Set the default provider (used when model has no provider prefix).
    pub fn set_default(&mut self, name: impl Into<String>) {
        self.default_provider = Some(name.into());
    }

    /// Add a model alias.
    pub fn add_alias(&mut self, alias: impl Into<String>, target: impl Into<String>) {
        self.aliases.insert(alias.into(), target.into());
    }

    /// Resolve a model string to (provider_name, model_name).
    ///
    /// Format: `provider/model` or just `model`.
    /// Aliases are resolved first.
    fn resolve(&self, model: &str) -> Result<(String, String)> {
        // Resolve alias first
        let resolved = self
            .aliases
            .get(model)
            .map(|s| s.as_str())
            .unwrap_or(model);

        // Check for provider/model format
        if let Some((provider, rest)) = resolved.split_once('/') {
            if self.providers.contains_key(provider) {
                return Ok((provider.to_string(), rest.to_string()));
            }
        }

        // Try to find a provider that knows this model
        for (_name, provider) in &self.providers {
            // Simple heuristic: if the model name contains the provider name, use it
            let provider_prefix = provider.name();
            if resolved.starts_with(provider_prefix) {
                continue; // Skip self-matches
            }
        }

        // Fall back to default provider
        if let Some(default) = &self.default_provider {
            return Ok((default.clone(), resolved.to_string()));
        }

        // Last resort: try each provider
        if let Some((name, _)) = self.providers.iter().next() {
            return Ok((name.clone(), resolved.to_string()));
        }

        Err(CoreError::Llm(format!(
            "No provider found for model: {}",
            model
        )))
    }

    /// Get a provider by name.
    pub fn get_provider(&self, name: &str) -> Option<&dyn LlmProvider> {
        self.providers.get(name).map(|p| p.as_ref())
    }

    /// List all registered provider names.
    pub fn provider_names(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    /// List all available models from all providers.
    pub async fn all_models(&self) -> Vec<ModelInfo> {
        let mut all = Vec::new();
        for (_name, provider) in &self.providers {
            match provider.models().await {
                Ok(models) => all.extend(models),
                Err(_) => continue,
            }
        }
        all
    }
}

impl Default for LlmRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmProvider for LlmRouter {
    async fn chat_completion(&self, request: ChatRequest) -> Result<ChatResponse> {
        let (provider_name, model_name) = self.resolve(&request.model)?;
        let provider = self
            .providers
            .get(&provider_name)
            .ok_or_else(|| CoreError::Llm(format!("Provider not found: {}", provider_name)))?;

        let mut req = request.clone();
        req.model = model_name;

        provider.chat_completion(req).await
    }

    fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<TokenChunk>> + Send>> {
        // Clone what we need to resolve in the stream
        match self.resolve(&request.model) {
            Ok((provider_name, model_name)) => {
                if let Some(provider) = self.providers.get(&provider_name) {
                    let mut req = request.clone();
                    req.model = model_name;
                    provider.chat_stream(req)
                } else {
                    let err = CoreError::Llm(format!("Provider not found: {}", provider_name));
                    Box::pin(futures::stream::once(async move { Err(err) }))
                }
            }
            Err(e) => {
                Box::pin(futures::stream::once(async move { Err(e) }))
            }
        }
    }

    async fn embeddings(&self, input: &str) -> Result<Vec<f32>> {
        // Use default provider for embeddings, or first available
        let provider_name = self
            .default_provider
            .as_deref()
            .or_else(|| self.providers.keys().next().map(|s| s.as_str()))
            .ok_or_else(|| CoreError::Llm("No providers registered".to_string()))?;

        let provider = self
            .providers
            .get(provider_name)
            .ok_or_else(|| CoreError::Llm(format!("Provider not found: {}", provider_name)))?;

        provider.embeddings(input).await
    }

    async fn models(&self) -> Result<Vec<ModelInfo>> {
        Ok(self.all_models().await)
    }

    fn name(&self) -> &str {
        "router"
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn supports_tools(&self) -> bool {
        self.providers.values().any(|p| p.supports_tools())
    }
}

// -- Tests --

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock provider for testing routing logic.
    struct MockProvider {
        name: String,
        supports_tools: bool,
    }

    impl MockProvider {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                supports_tools: name != "ollama",
            }
        }
    }

    #[async_trait]
    impl LlmProvider for MockProvider {
        async fn chat_completion(&self, request: ChatRequest) -> Result<ChatResponse> {
            Ok(ChatResponse {
                id: format!("{}-resp", self.name),
                message: Message {
                    id: MessageId::new(),
                    chat_id: request.messages.first().map(|m| m.chat_id).unwrap_or_default(),
                    role: MessageRole::Assistant,
                    content: format!("Response from {}", self.name),
                    tool_calls: vec![],
                    artifacts: vec![],
                    timestamp: chrono::Utc::now(),
                    metadata: serde_json::Value::Null,
                },
                usage: TokenUsage {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    total_tokens: 15,
                },
                model: request.model,
                finish_reason: FinishReason::Stop,
            })
        }

        fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> Pin<Box<dyn Stream<Item = Result<TokenChunk>> + Send>> {
            let name = self.name.clone();
            Box::pin(futures::stream::once(async move {
                Ok(TokenChunk {
                    delta: format!("stream from {}", name),
                    finish_reason: Some(FinishReason::Stop),
                    usage: None,
                })
            }))
        }

        async fn embeddings(&self, _input: &str) -> Result<Vec<f32>> {
            Ok(vec![0.1, 0.2, 0.3])
        }

        async fn models(&self) -> Result<Vec<ModelInfo>> {
            Ok(vec![ModelInfo {
                id: format!("{}-model-1", self.name),
                name: format!("{} Model 1", self.name),
                provider: self.name.clone(),
                context_length: 8192,
                supports_streaming: true,
                supports_tools: self.supports_tools,
                supports_vision: false,
            }])
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn supports_streaming(&self) -> bool {
            true
        }

        fn supports_tools(&self) -> bool {
            self.supports_tools
        }
    }

    fn make_request(model: &str) -> ChatRequest {
        ChatRequest {
            model: model.to_string(),
            messages: vec![Message {
                id: MessageId::new(),
                chat_id: ChatId::new(),
                role: MessageRole::User,
                content: "test".to_string(),
                tool_calls: vec![],
                artifacts: vec![],
                timestamp: chrono::Utc::now(),
                metadata: serde_json::Value::Null,
            }],
            tools: vec![],
            temperature: None,
            max_tokens: None,
            stream: false,
            persona: None,
            agent_role: None,
        }
    }

    #[test]
    fn test_resolve_provider_model_format() {
        let mut router = LlmRouter::new();
        router.register("openai", Box::new(MockProvider::new("openai")));
        router.register("anthropic", Box::new(MockProvider::new("anthropic")));

        let (provider, model) = router.resolve("openai/gpt-4o").unwrap();
        assert_eq!(provider, "openai");
        assert_eq!(model, "gpt-4o");

        let (provider, model) = router.resolve("anthropic/claude-sonnet-4").unwrap();
        assert_eq!(provider, "anthropic");
        assert_eq!(model, "claude-sonnet-4");
    }

    #[test]
    fn test_resolve_default_fallback() {
        let mut router = LlmRouter::new();
        router.register("openai", Box::new(MockProvider::new("openai")));
        router.set_default("openai");

        let (provider, model) = router.resolve("gpt-4o").unwrap();
        assert_eq!(provider, "openai");
        assert_eq!(model, "gpt-4o");
    }

    #[test]
    fn test_resolve_alias() {
        let mut router = LlmRouter::new();
        router.register("openai", Box::new(MockProvider::new("openai")));
        router.add_alias("gpt4", "openai/gpt-4o");

        let (provider, model) = router.resolve("gpt4").unwrap();
        assert_eq!(provider, "openai");
        assert_eq!(model, "gpt-4o");
    }

    #[tokio::test]
    async fn test_router_chat_completion() {
        let mut router = LlmRouter::new();
        router.register("openai", Box::new(MockProvider::new("openai")));
        router.register("anthropic", Box::new(MockProvider::new("anthropic")));

        let resp = router.chat_completion(make_request("openai/gpt-4o")).await.unwrap();
        assert_eq!(resp.id, "openai-resp");
        assert!(resp.message.content.contains("openai"));

        let resp = router.chat_completion(make_request("anthropic/claude-sonnet-4")).await.unwrap();
        assert_eq!(resp.id, "anthropic-resp");
    }

    #[tokio::test]
    async fn test_router_streaming() {
        let mut router = LlmRouter::new();
        router.register("ollama", Box::new(MockProvider::new("ollama")));

        use futures::StreamExt;
        let stream = router.chat_stream(make_request("ollama/llama3"));
        let chunks: Vec<_> = stream.collect().await;
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].as_ref().unwrap().delta.contains("ollama"));
    }

    #[tokio::test]
    async fn test_router_models() {
        let mut router = LlmRouter::new();
        router.register("openai", Box::new(MockProvider::new("openai")));
        router.register("ollama", Box::new(MockProvider::new("ollama")));

        let models = router.models().await.unwrap();
        assert_eq!(models.len(), 2);
    }

    #[test]
    fn test_router_supports_tools() {
        let mut router = LlmRouter::new();
        router.register("openai", Box::new(MockProvider::new("openai")));
        router.register("ollama", Box::new(MockProvider::new("ollama")));

        assert!(router.supports_tools()); // openai supports tools
    }

    #[test]
    fn test_provider_names() {
        let mut router = LlmRouter::new();
        router.register("openai", Box::new(MockProvider::new("openai")));
        router.register("anthropic", Box::new(MockProvider::new("anthropic")));

        let mut names = router.provider_names();
        names.sort();
        assert_eq!(names, vec!["anthropic", "openai"]);
    }

    #[tokio::test]
    async fn test_embeddings_uses_default() {
        let mut router = LlmRouter::new();
        router.register("openai", Box::new(MockProvider::new("openai")));
        router.set_default("openai");

        let emb = router.embeddings("hello").await.unwrap();
        assert_eq!(emb.len(), 3);
    }

    #[test]
    fn test_no_provider_error() {
        let router = LlmRouter::new();
        assert!(router.resolve("gpt-4o").is_err());
    }
}
