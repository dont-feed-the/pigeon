//! People & machines from the browser usability study (09 §9.7, §9.9):
//! removing a backup machine says plainly when the files stop being safe,
//! removing a computer says what becomes of its files, and a paper
//! recovery sheet shows the day it was printed.

use super::*;

/// A group of K = `copies` with this laptop, Paul's laptop, one backup
/// machine and one recovery sheet.
struct Group {
    copies: u32,
}

impl Backend for Group {
    fn request(&self, request: pigeon_proto::Request) -> Answer {
        let machine = |name: &str, custodian, recovery| Machine {
            name: name.into(),
            device: "d".into(),
            custodian,
            recovery,
            heard_at: None,
            this_device: name == "mine",
        };
        let answer = match request {
            pigeon_proto::Request::Machines => vec![Response::Machines {
                machines: vec![
                    machine("mine", false, false),
                    machine("paul-laptop", false, false),
                    machine("nas", true, false),
                    machine("2026-10-16-abc123", false, true),
                ],
                copies: Some(self.copies),
            }],
            _ => vec![Response::Done],
        };
        Box::pin(async move { answer })
    }
}

/// The page that asks before removing `name`, in `lang`, for a group of
/// K = `copies`.
async fn asked(lang: &str, copies: u32, name: &str) -> (String, String) {
    let w = Web::new(Arc::new(Group { copies }), lang.parse().unwrap(), 4321);
    let app = w.router();
    let cookie = logged_in(&app, &w).await;
    let page = body(
        app.clone()
            .oneshot(get("/machines", HOST, Some(&cookie)))
            .await
            .unwrap(),
    )
    .await;
    let form = form_of(&page);
    let ask = app
        .oneshot(post(
            "/machines/remove",
            &cookie,
            &format!("form={form}&name={name}"),
        ))
        .await
        .unwrap();
    (page, body(ask).await)
}

#[tokio::test]
async fn removing_the_last_backup_machine_says_the_files_stop_being_safe() {
    let (page, ask) = asked("en", 1, "nas").await;
    assert!(
        page.contains("2026-10-16-abc123: paper recovery sheet printed on 2026-10-16"),
        "{page}"
    );
    assert!(
        ask.contains("Remove the backup machine nas from the group?"),
        "{ask}"
    );
    assert!(
        ask.contains("the group will have no backup machine. None of your files will be safe until you add one."),
        "{ask}"
    );
    assert!(
        ask.contains("role=\"alert\"") && ask.contains("name=\"yes\""),
        "{ask}"
    );
    let (_, ask) = asked("fr", 2, "nas").await;
    assert!(ask.contains("plus aucune machine de sauvegarde"), "{ask}");
}

#[tokio::test]
async fn removing_a_computer_says_its_files_stay_and_it_can_come_back() {
    let (_, ask) = asked("en", 1, "paul-laptop").await;
    assert!(
        ask.contains("Its files stay on it but are no longer shared with the group"),
        "{ask}"
    );
    assert!(ask.contains("You can invite it again later."), "{ask}");
    assert!(!ask.contains("role=\"alert\""), "{ask}");
    let (_, ask) = asked("en", 1, "2026-10-16-abc123").await;
    assert!(ask.contains("Revoke 2026-10-16-abc123?"), "{ask}");
}
