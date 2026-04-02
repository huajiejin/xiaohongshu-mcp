use crate::t;
use anyhow::Result;
use chromiumoxide::page::Page;

pub async fn extract_initial_state(page: &Page) -> Result<serde_json::Value> {
    let js = r#"(() => {
        const s = window.__INITIAL_STATE__;
        if (!s) return null;
		const get_val = (o) => o?.value || o?._value || o?._rawValue;
		const feed_feeds = get_val(s?.feed?.feeds);
		const search_feeds = get_val(s?.search?.feeds);
        return JSON.parse(JSON.stringify({feed_feeds, search_feeds}));
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
