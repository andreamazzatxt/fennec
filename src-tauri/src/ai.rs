use serde::{Deserialize, Serialize};

use crate::config::FennecConfig;

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessageContent,
}

#[derive(Deserialize)]
struct ChatMessageContent {
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

pub async fn call_ai(config: &FennecConfig, prompt: &str) -> Result<String, String> {
    let client = reqwest::Client::new();

    let request = ChatRequest {
        model: config.model.clone(),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: prompt.into(),
        }],
        temperature: 0.3,
    };

    let response = client
        .post(&config.endpoint)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", config.api_key))
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(format!("AI Gateway error: {}", body));
    }

    let data: ChatResponse = response
        .json()
        .await
        .map_err(|e| format!("Parse error: {}", e))?;

    Ok(data.choices[0].message.content.trim().to_string())
}

pub async fn call_ai_with_retry(config: &FennecConfig, prompt: &str) -> Result<String, String> {
    match call_ai(config, prompt).await {
        Ok(result) => Ok(result),
        Err(e) => {
            if e.contains("BLOCKED") || e.contains("guardrail") {
                return Err(e);
            }
            // Retry once
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            call_ai(config, prompt).await
        }
    }
}
