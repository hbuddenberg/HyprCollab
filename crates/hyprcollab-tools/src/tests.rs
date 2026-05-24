//! Tests for hyprcollab-tools.

use hyprcollab_core::traits::Tool;
use serde_json;

mod shell_tests {
    use super::*;

    fn shell_tool() -> crate::shell::ShellTool {
        crate::shell::ShellTool::new().with_timeout(5)
    }

    #[tokio::test]
    async fn test_shell_echo() {
        let tool = shell_tool();
        let result = tool
            .execute(serde_json::json!({"command": "echo hello"}))
            .await
            .unwrap();
        let output: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(output["stdout"].as_str().unwrap().trim(), "hello");
        assert_eq!(output["exit_code"], 0);
    }

    #[tokio::test]
    async fn test_shell_stderr() {
        let tool = shell_tool();
        let result = tool
            .execute(serde_json::json!({"command": "echo error >&2"}))
            .await
            .unwrap();
        let output: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(output["stderr"].as_str().unwrap().trim(), "error");
    }

    #[tokio::test]
    async fn test_shell_nonzero_exit() {
        let tool = shell_tool();
        let result = tool
            .execute(serde_json::json!({"command": "exit 42"}))
            .await
            .unwrap();
        let output: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(output["exit_code"], 42);
    }

    #[tokio::test]
    async fn test_shell_name_and_description() {
        let tool = shell_tool();
        assert_eq!(tool.name(), "shell");
        assert!(!tool.description().is_empty());
    }

    #[tokio::test]
    async fn test_shell_invalid_params() {
        let tool = shell_tool();
        let result = tool.execute(serde_json::json!({})).await;
        assert!(result.is_err());
    }
}

mod file_ops_tests {
    use super::*;

    fn file_tool() -> crate::file_ops::FileOpsTool {
        crate::file_ops::FileOpsTool::new()
    }

    #[tokio::test]
    async fn test_file_write_and_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        let path_str = path.to_str().unwrap();

        let tool = file_tool();

        // Write
        let write_result = tool
            .execute(serde_json::json!({
                "action": "write",
                "path": path_str,
                "content": "Hello, HyprCollab!"
            }))
            .await
            .unwrap();
        assert!(write_result.contains("Successfully wrote"));

        // Read
        let read_result = tool
            .execute(serde_json::json!({
                "action": "read",
                "path": path_str
            }))
            .await
            .unwrap();
        assert_eq!(read_result, "Hello, HyprCollab!");
    }

    #[tokio::test]
    async fn test_file_exists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.txt");
        let path_str = path.to_str().unwrap();

        let tool = file_tool();
        let result = tool
            .execute(serde_json::json!({"action": "exists", "path": path_str}))
            .await
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["exists"], false);
    }

    #[tokio::test]
    async fn test_file_list() {
        let dir = tempfile::tempdir().unwrap();
        let tool = file_tool();

        // Create some files
        tokio::fs::write(dir.path().join("a.txt"), "a").await.unwrap();
        tokio::fs::write(dir.path().join("b.txt"), "b").await.unwrap();
        tokio::fs::create_dir(dir.path().join("subdir")).await.unwrap();

        let result = tool
            .execute(serde_json::json!({
                "action": "list",
                "path": dir.path().to_str().unwrap()
            }))
            .await
            .unwrap();

        assert!(result.contains("a.txt"));
        assert!(result.contains("b.txt"));
        assert!(result.contains("subdir/"));
    }

    #[tokio::test]
    async fn test_file_tool_metadata() {
        let tool = file_tool();
        assert_eq!(tool.name(), "file_ops");
        assert!(!tool.description().is_empty());
        let params = tool.parameters();
        assert!(params["properties"]["action"].is_object());
    }
}

mod memory_tests {
    use super::*;

    fn mem_tool() -> crate::memory_tools::MemoryTools {
        crate::memory_tools::MemoryTools::new()
    }

    #[tokio::test]
    async fn test_memory_store_and_search() {
        let tool = mem_tool();

        // Store a fact
        let store_result = tool
            .execute(serde_json::json!({
                "action": "store",
                "category": "preference",
                "content": "User prefers dark mode",
                "confidence": 0.95
            }))
            .await
            .unwrap();
        let fact: serde_json::Value = serde_json::from_str(&store_result).unwrap();
        assert_eq!(fact["category"], "preference");
        assert_eq!(fact["content"], "User prefers dark mode");

        // Search for it
        let search_result = tool
            .execute(serde_json::json!({
                "action": "search",
                "query": "dark mode"
            }))
            .await
            .unwrap();
        let results: Vec<serde_json::Value> = serde_json::from_str(&search_result).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["content"], "User prefers dark mode");
    }

    #[tokio::test]
    async fn test_memory_list() {
        let tool = mem_tool();

        tool.execute(serde_json::json!({
            "action": "store",
            "category": "fact",
            "content": "Rust is fast"
        }))
        .await
        .unwrap();

        tool.execute(serde_json::json!({
            "action": "store",
            "category": "preference",
            "content": "Vim bindings"
        }))
        .await
        .unwrap();

        let list_result = tool
            .execute(serde_json::json!({"action": "list"}))
            .await
            .unwrap();
        let facts: Vec<serde_json::Value> = serde_json::from_str(&list_result).unwrap();
        assert_eq!(facts.len(), 2);
    }

    #[tokio::test]
    async fn test_memory_delete() {
        let tool = mem_tool();

        let store_result = tool
            .execute(serde_json::json!({
                "action": "store",
                "category": "fact",
                "content": "Temporary fact"
            }))
            .await
            .unwrap();
        let fact: serde_json::Value = serde_json::from_str(&store_result).unwrap();
        let id = fact["id"].as_str().unwrap();

        let delete_result = tool
            .execute(serde_json::json!({"action": "delete", "id": id}))
            .await
            .unwrap();
        let del: serde_json::Value = serde_json::from_str(&delete_result).unwrap();
        assert_eq!(del["deleted"], true);
    }

    #[tokio::test]
    async fn test_memory_tool_metadata() {
        let tool = mem_tool();
        assert_eq!(tool.name(), "memory");
        assert!(!tool.description().is_empty());
    }
}

mod web_fetch_tests {
    use super::*;

    #[test]
    fn test_web_fetch_metadata() {
        let tool = crate::web_fetch::WebFetchTool::new();
        assert_eq!(tool.name(), "web_fetch");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_html_strip() {
        let html = "<html><head><title>Test</title></head><body><p>Hello <b>World</b></p></body></html>";
        let stripped = crate::web_fetch::WebFetchTool::strip_html(html);
        assert!(stripped.contains("Hello"));
        assert!(stripped.contains("World"));
        assert!(!stripped.contains('<'));
        assert!(!stripped.contains('>'));
    }

    #[test]
    fn test_html_strip_script_removal() {
        let html = "<p>Text</p><script>alert('xss')</script><p>More</p>";
        let stripped = crate::web_fetch::WebFetchTool::strip_html(html);
        assert!(stripped.contains("Text"));
        assert!(stripped.contains("More"));
        assert!(!stripped.contains("alert"));
    }
}

mod web_search_tests {
    use super::*;

    #[test]
    fn test_web_search_metadata() {
        let tool = crate::web_search::WebSearchTool::new();
        assert_eq!(tool.name(), "web_search");
        assert!(!tool.description().is_empty());
        let params = tool.parameters();
        assert!(params["properties"]["query"].is_object());
    }
}

mod shell_enhanced_tests {
    use super::*;

    #[tokio::test]
    async fn test_enhanced_run_echo() {
        let tool = crate::shell_enhanced::EnhancedShellTool::new();
        let result = tool
            .execute(serde_json::json!({"action": "run", "command": "echo enhanced"}))
            .await
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["stdout"].as_str().unwrap().trim(), "enhanced");
        assert_eq!(parsed["exit_code"], 0);
    }

    #[tokio::test]
    async fn test_enhanced_get_cwd() {
        let tool = crate::shell_enhanced::EnhancedShellTool::new();
        let result = tool
            .execute(serde_json::json!({"action": "get_cwd"}))
            .await
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed["cwd"].is_string());
    }

    #[tokio::test]
    async fn test_enhanced_set_and_list_env() {
        let tool = crate::shell_enhanced::EnhancedShellTool::new();

        // Set env
        let set_result = tool
            .execute(serde_json::json!({"action": "set_env", "key": "HYPRTEST", "value": "123"}))
            .await
            .unwrap();
        assert!(set_result.contains("HYPRTEST"));

        // List env
        let list_result = tool
            .execute(serde_json::json!({"action": "list_env"}))
            .await
            .unwrap();
        assert!(list_result.contains("HYPRTEST"));

        // Use env in command
        let run_result = tool
            .execute(serde_json::json!({"action": "run", "command": "echo $HYPRTEST"}))
            .await
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&run_result).unwrap();
        assert_eq!(parsed["stdout"].as_str().unwrap().trim(), "123");

        // Unset
        let unset_result = tool
            .execute(serde_json::json!({"action": "unset_env", "key": "HYPRTEST"}))
            .await
            .unwrap();
        assert!(unset_result.contains("HYPRTEST"));
    }

    #[tokio::test]
    async fn test_enhanced_background_process() {
        let tool = crate::shell_enhanced::EnhancedShellTool::new();

        // Start background process
        let start_result = tool
            .execute(serde_json::json!({
                "action": "start",
                "command": "echo bg_output && sleep 0.1",
                "id": "test-bg-1"
            }))
            .await
            .unwrap();
        assert!(start_result.contains("test-bg-1"));

        // Give it time to run
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Check output
        let output_result = tool
            .execute(serde_json::json!({"action": "output", "id": "test-bg-1"}))
            .await
            .unwrap();
        assert!(output_result.contains("bg_output"));

        // Stop
        let stop_result = tool
            .execute(serde_json::json!({"action": "stop", "id": "test-bg-1"}))
            .await
            .unwrap();
        assert!(stop_result.contains("test-bg-1"));
    }

    #[tokio::test]
    async fn test_enhanced_tool_metadata() {
        let tool = crate::shell_enhanced::EnhancedShellTool::new();
        assert_eq!(tool.name(), "shell");
        assert!(!tool.description().is_empty());
        let params = tool.parameters();
        assert!(params["properties"]["action"].is_object());
    }

    #[tokio::test]
    async fn test_enhanced_run_with_timeout_error() {
        let tool = crate::shell_enhanced::EnhancedShellTool::new().with_timeout(1);
        let result = tool
            .execute(serde_json::json!({"action": "run", "command": "sleep 10"}))
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("timed out"));
    }
}
