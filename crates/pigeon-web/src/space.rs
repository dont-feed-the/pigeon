//! Read-only Space page (08 §8.9), from the same capacity response as CLI.
use crate::{Web, pages};
use axum::{extract::State, response::Html};
use maud::html;
use pigeon_proto::Request;
use std::sync::Arc;

pub(crate) async fn page(State(web): State<Arc<Web>>) -> Html<String> {
    let r = web.renderer();
    let lines = pages::lines(&web, &r, Request::Capacity).await;
    pages::page(
        &r,
        "/space",
        &r.id("space-title"),
        &html! { div class="capacity-details" { (pages::list(&lines)) } },
    )
}
