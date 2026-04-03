use crate::t;
use anyhow::Result;
use chromiumoxide::page::Page;

pub async fn extract_initial_state(page: &Page) -> Result<serde_json::Value> {
    let js = r#"(() => {
        const s = window.__INITIAL_STATE__;
        if (!s) return null;
		const get_val = (o) => o?.value || o?._value || o?._rawValue;
		const flatten = (arr) => Array.isArray(arr) && arr.some(Array.isArray) ? arr.flat() : arr;
		const feed_feeds = flatten(get_val(s?.feed?.feeds));
		const search_feeds = flatten(get_val(s?.search?.feeds));
		const user_notes = flatten(get_val(s?.user?.notes));
		const user_data = get_val(s?.user?.userPageData);
        return JSON.parse(JSON.stringify({feed_feeds, search_feeds, user_notes, user_data}));
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
