//! Actual settings-router confirmations from M6's independent usability
//! sessions: free-form values are text, not names of catalogue messages.

use super::*;

#[tokio::test]
async fn free_form_resource_values_are_readable_in_both_languages() {
    for (lang, expected, none) in [
        ("en", "Maximum upload", "None"),
        ("fr", "Envoi maximal", "Aucune"),
    ] {
        let web = Web::new(Arc::new(Fixed), lang.parse().unwrap(), 4321);
        let app = web.router();
        let cookie = logged_in(&app, &web).await;
        let page = body(
            app.clone()
                .oneshot(get("/settings", HOST, Some(&cookie)))
                .await
                .unwrap(),
        )
        .await;
        let form = form_of(&page);
        for (key, value, shown) in [
            ("upload", "2+MB%2Fs", "2 MB/s"),
            ("battery_percent", "15", "15"),
            ("full_speed", "22%3A00-23%3A00", "22:00-23:00"),
            ("full_speed", "", none),
        ] {
            let result = body(
                app.clone()
                    .oneshot(post(
                        "/settings",
                        &cookie,
                        &format!("form={form}&key=resources.{key}&value={value}"),
                    ))
                    .await
                    .unwrap(),
            )
            .await;
            assert!(result.contains(shown), "{result}");
            assert!(
                !result.contains("setting-2 MB/s")
                    && !result.contains("setting-15")
                    && !result.contains("setting-22:00"),
                "{result}"
            );
            if key == "upload" {
                assert!(result.contains(expected), "{result}");
            }
        }
    }
}

struct Invalid;

impl Backend for Invalid {
    fn request(&self, request: pigeon_proto::Request) -> Answer {
        if matches!(request, pigeon_proto::Request::SetSettings { .. }) {
            Box::pin(async {
                vec![Response::Problem(pigeon_proto::Problem {
                    code: "setting-value".into(),
                    message: Message {
                        id: "problem-setting-value".into(),
                        args: Default::default(),
                    },
                    hint: None,
                    details: Some("an internal parser detail".into()),
                })]
            })
        } else {
            Fixed.request(request)
        }
    }
}

#[tokio::test]
async fn an_invalid_resource_value_uses_the_field_label_and_keeps_the_person_in_the_web() {
    for (lang, label, unchanged) in [
        ("en", "Maximum upload", "Nothing changed"),
        ("fr", "Envoi maximal", "Rien n’a changé"),
    ] {
        let web = Web::new(Arc::new(Invalid), lang.parse().unwrap(), 4321);
        let app = web.router();
        let cookie = logged_in(&app, &web).await;
        let page = body(
            app.clone()
                .oneshot(get("/settings", HOST, Some(&cookie)))
                .await
                .unwrap(),
        )
        .await;
        let result = body(
            app.oneshot(post(
                "/settings",
                &cookie,
                &format!(
                    "form={}&key=resources.upload&value=2+bananas%2Fs",
                    form_of(&page)
                ),
            ))
            .await
            .unwrap(),
        )
        .await;
        assert!(
            result.contains(label) && result.contains(unchanged) && result.contains("2 bananas/s"),
            "{result}"
        );
        assert!(
            !result.contains("resources.upload")
                && !result.contains("pigeon settings")
                && !result.contains("internal parser"),
            "{result}"
        );
    }
}
