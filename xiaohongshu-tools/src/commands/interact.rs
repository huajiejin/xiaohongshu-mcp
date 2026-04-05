use crate::browser::human::HumanBehavior;
use crate::extract::note::NoteCard;
use crate::selectors::{COMMENT_CONTAINER_SELECTORS, Selector};
use crate::t;
use anyhow::{Result, anyhow};
use chromiumoxide::page::Page;
use rand::Rng;
use std::time::Duration;
use tracing::warn;

pub async fn open_note(page: &Page, card: &NoteCard) -> Result<()> {
    if let Some(note_id) = &card.id {
        let selector = crate::selectors::note_cover_by_id_selector(note_id);
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
        .scroll_container(
            page,
            &COMMENT_CONTAINER_SELECTORS
                .iter()
                .map(|s| s.css())
                .collect::<Vec<_>>(),
        )
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
    if let Ok(btn) = page.find_element(Selector::CloseBtn.css()).await
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
