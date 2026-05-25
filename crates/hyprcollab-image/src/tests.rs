#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use crate::{
        a1111::A1111Provider,
        comfyui::ComfyUiProvider,
        fal::FalProvider,
        openai::OpenAiProvider,
        provider::{ImageProvider, ImageRouter},
        types::{
            GeneratedImage, ImageError, ImageProviderType, ImageQuality, ImageRequest, ImageResponse,
            ImageSize, ImageStyle,
        },
    };

    fn basic_request() -> ImageRequest {
        ImageRequest::new("a beautiful mountain landscape")
    }

    // ── types ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_image_size_square_dimensions() {
        assert_eq!(ImageSize::Square1024.dimensions(), (1024, 1024));
    }

    #[test]
    fn test_image_size_landscape_dimensions() {
        assert_eq!(ImageSize::Landscape1792x1024.dimensions(), (1792, 1024));
    }

    #[test]
    fn test_image_size_portrait_dimensions() {
        assert_eq!(ImageSize::Portrait1024x1792.dimensions(), (1024, 1792));
    }

    #[test]
    fn test_image_size_custom_dimensions() {
        let size = ImageSize::Custom { width: 512, height: 768 };
        assert_eq!(size.dimensions(), (512, 768));
    }

    #[test]
    fn test_image_size_to_openai_str() {
        assert_eq!(ImageSize::Square1024.to_openai_str(), "1024x1024");
        assert_eq!(ImageSize::Landscape1792x1024.to_openai_str(), "1792x1024");
        assert_eq!(ImageSize::Portrait1024x1792.to_openai_str(), "1024x1792");
    }

    #[test]
    fn test_image_size_custom_falls_back_to_square() {
        let size = ImageSize::Custom { width: 300, height: 400 };
        assert_eq!(size.to_openai_str(), "1024x1024");
    }

    #[test]
    fn test_image_request_defaults() {
        let req = ImageRequest::new("test");
        assert_eq!(req.prompt, "test");
        assert_eq!(req.n, 1);
        assert!(req.model.is_none());
        assert!(req.seed.is_none());
        assert!(req.negative_prompt.is_none());
    }

    #[test]
    fn test_image_style_to_openai_str() {
        assert_eq!(ImageStyle::Vivid.to_openai_str(), "vivid");
        assert_eq!(ImageStyle::Natural.to_openai_str(), "natural");
    }

    #[test]
    fn test_image_quality_to_openai_str() {
        assert_eq!(ImageQuality::Standard.to_openai_str(), "standard");
        assert_eq!(ImageQuality::Hd.to_openai_str(), "hd");
    }

    #[test]
    fn test_image_size_default() {
        assert_eq!(ImageSize::default(), ImageSize::Square1024);
    }

    // ── mock provider ─────────────────────────────────────────────────────────

    struct MockProvider {
        provider_name: String,
        fail: bool,
    }

    impl MockProvider {
        fn ok(name: &str) -> Self {
            Self { provider_name: name.to_string(), fail: false }
        }
        fn err(name: &str) -> Self {
            Self { provider_name: name.to_string(), fail: true }
        }
    }

    #[async_trait]
    impl ImageProvider for MockProvider {
        fn name(&self) -> &str {
            &self.provider_name
        }
        fn provider_type(&self) -> ImageProviderType {
            ImageProviderType::OpenAi
        }
        fn supported_sizes(&self) -> &[ImageSize] {
            &[ImageSize::Square1024]
        }
        fn default_model(&self) -> &str {
            "mock"
        }
        async fn generate(&self, _req: ImageRequest) -> crate::types::Result<ImageResponse> {
            if self.fail {
                return Err(ImageError::Provider("mock failure".to_string()));
            }
            Ok(ImageResponse {
                images: vec![GeneratedImage {
                    url: Some("http://mock.example.com/img.png".into()),
                    data: None,
                    revised_prompt: None,
                    width: 1024,
                    height: 1024,
                    seed: None,
                }],
                model: "mock".to_string(),
                provider: self.provider_name.clone(),
                created: chrono::Utc::now(),
            })
        }
    }

    // ── router ────────────────────────────────────────────────────────────────

    #[test]
    fn test_router_add_and_has() {
        let mut router = ImageRouter::new();
        router.add(MockProvider::ok("p1"));
        assert!(router.has_provider("p1"));
        assert!(!router.has_provider("p2"));
    }

    #[test]
    fn test_router_set_default_ok() {
        let mut router = ImageRouter::new();
        router.add(MockProvider::ok("p1"));
        assert!(router.set_default("p1"));
    }

    #[test]
    fn test_router_set_default_missing() {
        let mut router = ImageRouter::new();
        assert!(!router.set_default("ghost"));
    }

    #[test]
    fn test_router_provider_names() {
        let mut router = ImageRouter::new();
        router.add(MockProvider::ok("a"));
        router.add(MockProvider::ok("b"));
        let names = router.provider_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"a"));
        assert!(names.contains(&"b"));
    }

    #[tokio::test]
    async fn test_router_no_default_error() {
        let router = ImageRouter::new();
        let result = router.generate(basic_request()).await;
        assert!(matches!(result, Err(ImageError::NoDefaultProvider)));
    }

    #[tokio::test]
    async fn test_router_unknown_provider_error() {
        let router = ImageRouter::new();
        let result = router.generate_with("ghost", basic_request()).await;
        assert!(matches!(result, Err(ImageError::ProviderNotFound(_))));
    }

    #[tokio::test]
    async fn test_router_generate_default() {
        let mut router = ImageRouter::new();
        router.add(MockProvider::ok("mock"));
        router.set_default("mock");
        let result = router.generate(basic_request()).await.expect("generate");
        assert_eq!(result.provider, "mock");
        assert_eq!(result.images.len(), 1);
    }

    #[tokio::test]
    async fn test_router_generate_with_named() {
        let mut router = ImageRouter::new();
        router.add(MockProvider::ok("p1"));
        router.add(MockProvider::ok("p2"));
        let result = router.generate_with("p2", basic_request()).await.expect("generate");
        assert_eq!(result.provider, "p2");
    }

    #[tokio::test]
    async fn test_router_error_propagated() {
        let mut router = ImageRouter::new();
        router.add(MockProvider::err("fail"));
        router.set_default("fail");
        let result = router.generate(basic_request()).await;
        assert!(matches!(result, Err(ImageError::Provider(_))));
    }

    #[tokio::test]
    async fn test_router_multiple_providers_independent() {
        let mut router = ImageRouter::new();
        router.add(MockProvider::ok("good"));
        router.add(MockProvider::err("bad"));
        assert!(router.generate_with("good", basic_request()).await.is_ok());
        assert!(router.generate_with("bad", basic_request()).await.is_err());
    }

    // ── OpenAI ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_openai_generate_success() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("POST", "/v1/images/generations")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"created":1700000000,"data":[{"url":"https://cdn.example.com/out.png","revised_prompt":"enhanced"}]}"#,
            )
            .create_async()
            .await;

        let provider = OpenAiProvider::with_base_url("test-key", server.url());
        let resp = provider.generate(basic_request()).await.expect("generate");
        assert_eq!(resp.provider, "openai");
        assert_eq!(resp.images.len(), 1);
        assert!(resp.images[0].url.as_deref().unwrap_or("").contains("example.com"));
        assert_eq!(resp.images[0].revised_prompt.as_deref(), Some("enhanced"));
    }

    #[tokio::test]
    async fn test_openai_generate_api_error() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("POST", "/v1/images/generations")
            .with_status(400)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error":{"message":"invalid_request","type":"bad","code":null}}"#)
            .create_async()
            .await;

        let provider = OpenAiProvider::with_base_url("test-key", server.url());
        let result = provider.generate(basic_request()).await;
        assert!(matches!(result, Err(ImageError::ApiError { status: 400, .. })));
    }

    #[test]
    fn test_openai_provider_metadata() {
        let p = OpenAiProvider::new("k");
        assert_eq!(p.name(), "openai");
        assert_eq!(p.default_model(), "dall-e-3");
        assert!(!p.supported_sizes().is_empty());
        assert!(matches!(p.provider_type(), ImageProviderType::OpenAi));
    }

    // ── FAL ───────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_fal_generate_success() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("POST", mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"images":[{"url":"https://fal.cdn.example.com/img.jpg","width":1024,"height":1024,"content_type":"image/jpeg"}],"seed":99}"#)
            .create_async()
            .await;

        let provider = FalProvider::with_base_url("test-key", server.url());
        let resp = provider.generate(basic_request()).await.expect("generate");
        assert_eq!(resp.provider, "fal");
        assert_eq!(resp.images.len(), 1);
        assert_eq!(resp.images[0].seed, Some(99));
        assert!(resp.images[0].url.is_some());
    }

    #[tokio::test]
    async fn test_fal_generate_api_error() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("POST", mockito::Matcher::Any)
            .with_status(422)
            .with_header("content-type", "application/json")
            .with_body(r#"{"detail":"bad prompt"}"#)
            .create_async()
            .await;

        let provider = FalProvider::with_base_url("test-key", server.url());
        let result = provider.generate(basic_request()).await;
        assert!(matches!(result, Err(ImageError::ApiError { status: 422, .. })));
    }

    #[test]
    fn test_fal_provider_metadata() {
        let p = FalProvider::new("k");
        assert_eq!(p.name(), "fal");
        assert_eq!(p.default_model(), "fal-ai/flux/schnell");
        assert!(!p.supported_sizes().is_empty());
        assert!(matches!(p.provider_type(), ImageProviderType::Fal));
    }

    // ── ComfyUI ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_comfyui_generate_success() {
        let mut server = mockito::Server::new_async().await;
        let _pm = server
            .mock("POST", "/prompt")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"prompt_id":"abc123","number":1}"#)
            .create_async()
            .await;
        let _hm = server
            .mock("GET", "/history/abc123")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"abc123":{"outputs":{"9":{"images":[{"filename":"HyprCollab_00001_.png","subfolder":"","type":"output"}]}}}}"#)
            .create_async()
            .await;

        let provider = ComfyUiProvider::with_base_url(server.url());
        let resp = provider.generate(basic_request()).await.expect("generate");
        assert_eq!(resp.provider, "comfyui");
        assert_eq!(resp.images.len(), 1);
        assert!(resp.images[0].url.as_deref().unwrap_or("").contains("HyprCollab_00001_.png"));
    }

    #[tokio::test]
    async fn test_comfyui_generate_prompt_error() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("POST", "/prompt")
            .with_status(500)
            .with_body("internal error")
            .create_async()
            .await;

        let provider = ComfyUiProvider::with_base_url(server.url());
        let result = provider.generate(basic_request()).await;
        assert!(matches!(result, Err(ImageError::ApiError { status: 500, .. })));
    }

    #[test]
    fn test_comfyui_provider_metadata() {
        let p = ComfyUiProvider::new();
        assert_eq!(p.name(), "comfyui");
        assert_eq!(p.default_model(), "comfyui");
        assert!(!p.supported_sizes().is_empty());
        assert!(matches!(p.provider_type(), ImageProviderType::ComfyUi));
    }

    // ── A1111 ─────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_a1111_generate_success() {
        let mut server = mockito::Server::new_async().await;
        // 1×1 white PNG, base64-encoded
        let tiny_b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwADhQGAWjR9awAAAABJRU5ErkJggg==";
        let body = format!(r#"{{"images":["{}"],"info":"{{\"all_seeds\":[12345]}}"}}"#, tiny_b64);
        let _mock = server
            .mock("POST", "/sdapi/v1/txt2img")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create_async()
            .await;

        let provider = A1111Provider::with_base_url(server.url());
        let resp = provider.generate(basic_request()).await.expect("generate");
        assert_eq!(resp.provider, "a1111");
        assert_eq!(resp.images.len(), 1);
        assert!(resp.images[0].data.is_some());
        assert!(resp.images[0].url.is_none());
        assert_eq!(resp.images[0].seed, Some(12345));
    }

    #[tokio::test]
    async fn test_a1111_generate_api_error() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("POST", "/sdapi/v1/txt2img")
            .with_status(422)
            .with_body(r#"{"detail":"Model not loaded"}"#)
            .create_async()
            .await;

        let provider = A1111Provider::with_base_url(server.url());
        let result = provider.generate(basic_request()).await;
        assert!(matches!(result, Err(ImageError::ApiError { status: 422, .. })));
    }

    #[test]
    fn test_a1111_provider_metadata() {
        let p = A1111Provider::new();
        assert_eq!(p.name(), "a1111");
        assert_eq!(p.default_model(), "a1111");
        assert!(!p.supported_sizes().is_empty());
        assert!(matches!(p.provider_type(), ImageProviderType::Automatic1111));
    }

    #[test]
    fn test_a1111_with_api_key() {
        let p = A1111Provider::new().with_api_key("secret");
        assert_eq!(p.name(), "a1111");
    }
}
