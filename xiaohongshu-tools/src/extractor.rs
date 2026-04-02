use crate::t;
use anyhow::Result;
use chromiumoxide::page::Page;

pub async fn extract_initial_state(page: &Page) -> Result<serde_json::Value> {
    let js = r#"(() => {
        const s = window.__INITIAL_STATE__;
        if (!s) return null;
        return JSON.parse(JSON.stringify(s));
    })()"#;

    let value = page
        .evaluate_expression(js)
        .await?
        .into_value::<serde_json::Value>()?;

    if value.is_null() {
        anyhow::bail!("{}", t!("extractor.initial_state_null"));
    }

    Ok(value)
}

pub fn resolve_vue_value(value: &serde_json::Value) -> &serde_json::Value {
    if let Some(obj) = value.as_object() {
        if let Some(v) = obj.get("value")
            && !v.is_null()
        {
            return v;
        }
        if let Some(v) = obj.get("_value")
            && !v.is_null()
        {
            return v;
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_resolve_vue_value_with_value_field() {
        let input = json!({"value": 42, "_value": 99});
        let result = resolve_vue_value(&input);
        assert_eq!(result, &json!(42));
    }

    #[test]
    fn test_resolve_vue_value_fallback_to_underscore() {
        let input = json!({"value": null, "_value": 99});
        let result = resolve_vue_value(&input);
        assert_eq!(result, &json!(99));
    }

    #[test]
    fn test_resolve_vue_value_no_vue_proxy() {
        let input = json!({"name": "test"});
        let result = resolve_vue_value(&input);
        assert_eq!(result, &input);
    }

    #[test]
    fn test_resolve_vue_value_primitive() {
        let input = json!(42);
        let result = resolve_vue_value(&input);
        assert_eq!(result, &json!(42));
    }
}
