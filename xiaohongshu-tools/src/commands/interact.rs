use crate::browser::human::HumanBehavior;
use crate::extract::note::NoteCard;
use crate::t;
use anyhow::{Result, anyhow};
use chromiumoxide::page::Page;
use rand::Rng;
use std::time::Duration;
use tracing::warn;

const CLOSE_BTN_SELECTOR: &str = "body > div.note-detail-mask > div.close-circle";

const COMMENT_CONTAINER_SELECTORS: &[&str] = &[
    "#noteContainer div.interaction-container > div.note-scroller",
    "#noteContainer div.interaction-container",
    "#noteContainer",
];

pub async fn open_note(page: &Page, card: &NoteCard) -> Result<()> {
    if let Some(note_id) = &card.id {
        let selector = format!("a.cover[href*='/{note_id}']");
        if let Ok(el) = page.find_element(&selector).await {
            el.click().await?;
            return Ok(());
        }
    }

    Err(anyhow!(t!(
        "explore.click_note_failed",
        e = "note element not found"
    )))
}

pub async fn browse_note(page: &Page, human: &mut HumanBehavior) -> Result<()> {
    human.random_delay(human.config.short_read.clone()).await;

    if let Err(e) = human
        .scroll_container(page, COMMENT_CONTAINER_SELECTORS)
        .await
    {
        warn!(
            "{}",
            t!("explore.scroll_comments_failed", e = e.to_string())
        );
    }

    human.random_delay(human.config.human_delay.clone()).await;

    close_note_detail(page).await;

    let wait = {
        let mut rng = rand::rng();
        rng.random_range(1000u64..2000)
    };
    tokio::time::sleep(Duration::from_millis(wait)).await;

    Ok(())
}

pub async fn close_note_detail(page: &Page) {
    if let Ok(btn) = page.find_element(CLOSE_BTN_SELECTOR).await
        && btn.click().await.is_ok()
    {
        return;
    }

    let js = r#"
        const btn = document.querySelector('.note-detail-mask .close-circle')
            || document.querySelector('.close-circle');
        if (btn) { btn.click(); }
    "#;
    let _ = page.evaluate_expression(js).await;

    let esc_js = r#"
        document.dispatchEvent(new KeyboardEvent('keydown', {key:'Escape',keyCode:27,bubbles:true}));
    "#;
    let _ = page.evaluate_expression(esc_js).await;
}
