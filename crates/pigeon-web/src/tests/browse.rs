//! The Files browser through the router: the table, sorting, the details
//! panel, the client's assets and the headers that confine
//! them (10 §10.3, ADR 0076); and the client's own tests, run by Node.

use super::demo::{Demo, ROOT};
use super::*;

async fn page_of(path: &str) -> String {
    let w = Web::new(Arc::new(Demo), "en".parse().unwrap(), 4321);
    let app = w.router();
    let cookie = logged_in(&app, &w).await;
    let r = app.oneshot(get(path, HOST, Some(&cookie))).await.unwrap();
    assert_eq!(r.status(), StatusCode::OK, "{path}");
    body(r).await
}

fn enc(s: &str) -> String {
    crate::browse::enc(s)
}

#[tokio::test]
async fn the_group_folder_lists_folders_first_with_sizes_and_states() {
    let page = page_of("/files").await;
    let budget = page.find("data-name=\"Budget\"").unwrap();
    let readme = page.find("data-name=\"README.md\"").unwrap();
    assert!(budget < readme, "folders first");
    assert!(page.contains("2 not safe yet"), "Photos sums up its files");
    assert!(page.contains("All safe"));
    assert!(
        page.contains("Not on this computer"),
        "Photos 2019 is freed"
    );
    assert!(page.contains("48 kB") && page.contains("940 MB"), "sizes");
    assert!(
        page.contains("ft ft-sheet") && page.contains("ft ft-folder"),
        "icons by kind"
    );
    assert!(page.contains("action=\"/files/keep\"") && page.contains("action=\"/files/free\""));
    assert!(page.contains("aria-sort=\"ascending\""), "sorted by name");
    assert!(page.contains("href=\"/files?sort=size\""), "sort by size");
}

#[tokio::test]
async fn a_folder_has_its_trail() {
    let page = page_of(&format!("/folder?path={}", enc(&format!("{ROOT}/Photos")))).await;
    assert!(page.contains("aria-label=\"Location\""), "{page}");
    assert!(page.contains(&format!(
        "href=\"/folder?path={}\" aria-current=\"page\"",
        enc(&format!("{ROOT}/Photos"))
    )));
    assert!(page.contains("data-name=\"2026-10-12\"") && page.contains("data-name=\"Vacances\""));
}

#[tokio::test]
async fn a_selected_file_shows_its_details_beside_the_table() {
    let file = format!("{ROOT}/budget-2026.xlsx");
    let page = page_of(&format!("/files?selected={}", enc(&file))).await;
    assert!(page.contains("class=\"browser has-details\""));
    assert!(page.contains("id=\"details-title\""));
    assert_eq!(
        page.matches("Restore this version").count(),
        2,
        "all but the newest"
    );
    assert!(page.contains("Version 3") && page.contains("current version"));
    assert!(
        page.contains("other changes are decided by Paul"),
        "the sharing rule"
    );
    assert!(page.contains("class=\"selected\""));
    assert!(
        page.contains("href=\"/files\" data-details"),
        "closing returns to the folder"
    );
    let none = page_of(&format!("/files?selected={}", enc("/elsewhere"))).await;
    assert!(
        !none.contains("id=\"details\""),
        "only a row of the table is selected"
    );
}

#[tokio::test]
async fn a_selected_folder_shows_its_rule_and_storage() {
    let folder = format!("{ROOT}/Recettes de Paul");
    let page = page_of(&format!(
        "/files?sort=size&dir=desc&selected={}",
        enc(&folder)
    ))
    .await;
    assert!(
        page.contains("Recettes de Paul") && page.contains("Paul"),
        "{page}"
    );
    assert!(page.contains("Kept on this computer"));
    assert!(page.contains("aria-sort=\"descending\""));
    assert!(
        page.contains("href=\"/files?sort=size&amp;dir=desc\" data-details"),
        "close keeps the order"
    );
}

#[tokio::test]
async fn pages_carry_a_strict_policy_and_assets_are_immutable() {
    let w = Web::new(Arc::new(Demo), "en".parse().unwrap(), 4321);
    let app = w.router();
    let cookie = logged_in(&app, &w).await;
    let r = app
        .clone()
        .oneshot(get("/files", HOST, Some(&cookie)))
        .await
        .unwrap();
    let csp = r.headers()[header::CONTENT_SECURITY_POLICY]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(
        csp.contains("default-src 'none'") && csp.contains("script-src 'self'"),
        "{csp}"
    );
    assert!(!csp.contains("unsafe"), "{csp}");
    let page = body(r).await;
    assert!(
        !page.contains("<script>") && !page.contains(" style=\""),
        "no inline script or style"
    );
    for asset in [
        &crate::assets::SCRIPT,
        &crate::assets::STYLE,
        &crate::assets::ICON,
    ] {
        assert!(
            page.contains(&format!("/assets/{}", asset.name)),
            "{}",
            asset.name
        );
        let r = app
            .clone()
            .oneshot(get(&format!("/assets/{}", asset.name), HOST, None))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::OK);
        assert!(
            r.headers()[header::CACHE_CONTROL]
                .to_str()
                .unwrap()
                .contains("immutable")
        );
        assert_eq!(
            axum::body::to_bytes(r.into_body(), 1 << 22).await.unwrap(),
            asset.bytes
        );
    }
    let missing = app
        .clone()
        .oneshot(get("/assets/app-0.js", HOST, None))
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    let elsewhere = app
        .oneshot(get(
            &format!("/assets/{}", crate::assets::SCRIPT.name),
            "evil.example",
            None,
        ))
        .await
        .unwrap();
    assert_eq!(
        elsewhere.status(),
        StatusCode::FORBIDDEN,
        "assets keep the host check"
    );
}

/// `client/test/*.test.ts`, bundled by `build.rs`, under `node --test`.
#[test]
fn the_client_logic_passes_its_tests() {
    let dir = std::path::Path::new(env!("PIGEON_WEB_CLIENT_TESTS"));
    let mut tests: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_string_lossy().ends_with(".test.mjs"))
        .collect();
    tests.sort();
    assert!(!tests.is_empty(), "no client test in {}", dir.display());
    let out = std::process::Command::new(env!("PIGEON_WEB_NODE"))
        .arg("--test")
        .args(&tests)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
