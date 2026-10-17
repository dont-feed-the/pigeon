//! Socket debug output preserves message kind while withholding private
//! payloads; intended wire serialization must continue carrying them.

use crate::{Arg, Message, Request, Response};

#[test]
fn socket_requests_hide_invitation_and_recovery_authority() {
    for value in [
        serde_json::json!({"type":"Join","invitation":"synthetic-request-authority"}),
        serde_json::json!({"type":"Recover","sheet":"synthetic-request-authority"}),
    ] {
        let request: Request = serde_json::from_value(value).expect("fixture request");
        for text in [format!("{request:?}"), format!("{request:#?}")] {
            assert!(
                !text.contains("synthetic-request"),
                "request authority appeared in Debug"
            );
            assert!(text.contains("Join") || text.contains("Recover"));
        }
        let wire = serde_json::to_string(&request).expect("request wire");
        assert!(wire.contains("synthetic-request-authority"));
        let restored: Request = serde_json::from_str(&wire).expect("request roundtrip");
        assert!(
            restored == request,
            "redaction changed request wire semantics"
        );
    }
}

#[test]
fn local_responses_hide_private_text_and_link_authority() {
    let message = Message {
        id: "fixture-link".into(),
        args: [(
            "url".into(),
            Arg::Text("synthetic-response-authority".into()),
        )]
        .into(),
    };
    for response in [
        Response::Said { message },
        Response::Text {
            text: "synthetic-response-authority".into(),
            size: 28,
            truncated: false,
        },
    ] {
        for text in [format!("{response:?}"), format!("{response:#?}")] {
            assert!(
                !text.contains("synthetic-response"),
                "response payload appeared in Debug"
            );
            assert!(text.contains("Said") || text.contains("Text"));
        }
        let wire = serde_json::to_string(&response).expect("response wire");
        assert!(wire.contains("synthetic-response-authority"));
        let restored: Response = serde_json::from_str(&wire).expect("response roundtrip");
        assert!(
            restored == response,
            "redaction changed response wire semantics"
        );
    }
}

#[test]
fn a_message_does_not_debug_format_arbitrary_link_arguments() {
    let message = Message {
        id: "fixture-message".into(),
        args: [(
            "token".into(),
            Arg::Text("synthetic-message-authority".into()),
        )]
        .into(),
    };
    let text = format!("{message:#?}");
    assert!(
        !text.contains("synthetic-message"),
        "message authority appeared in Debug"
    );
    assert!(text.contains("fixture-message"));
}

#[test]
fn a_text_argument_never_debug_formats_its_private_value() {
    let argument = Arg::Text("synthetic-argument-authority".into());
    let text = format!("{argument:?}");
    assert!(
        !text.contains("synthetic-argument"),
        "argument authority appeared in Debug"
    );
    assert!(text.contains("Text"));
    assert!(
        serde_json::to_string(&argument)
            .expect("argument wire")
            .contains("synthetic-argument-authority")
    );
}

#[test]
fn join_proofs_are_redacted_without_changing_the_join_wire() {
    let join: crate::join::Join = serde_json::from_value(serde_json::json!({"v":2,"invitation_hash":"fixture-hash","proof":"synthetic-join-proof","role":"laptop","device_name":"fixture-device","annex_uuid":"fixture-uuid"})).expect("fixture join");
    let text = format!("{join:#?}");
    assert!(
        !text.contains("synthetic-join"),
        "join proof appeared in Debug"
    );
    assert!(text.contains("fixture-device"));
    assert!(
        serde_json::to_string(&join)
            .expect("join wire")
            .contains("synthetic-join-proof")
    );
}

#[test]
fn erasure_confirmation_and_cancellation_tokens_stay_out_of_debug() {
    for value in [
        serde_json::json!({"action":"confirm","selection":{"kind":"path","value":"/fixture"},"token":"synthetic-erasure-authority"}),
        serde_json::json!({"action":"cancel","decision":"fixture-decision","token":"synthetic-erasure-authority"}),
    ] {
        let operation: crate::erase::Operation =
            serde_json::from_value(value).expect("fixture erasure operation");
        assert!(
            !format!("{operation:?}").contains("synthetic-erasure"),
            "erasure token appeared in Debug"
        );
        assert!(
            serde_json::to_string(&operation)
                .expect("erasure wire")
                .contains("synthetic-erasure-authority")
        );
    }
}

#[test]
fn erasure_previews_retain_scope_counts_but_hide_confirmation_tokens() {
    let preview: crate::erase::Preview = serde_json::from_value(serde_json::json!({"selection":{"kind":"path","value":"/fixture"},"token":"synthetic-preview-authority","current_files":2,"versions":3,"bytes":4,"shared_versions":0,"retained_elsewhere":[],"retention_days":30,"needs_approval":false,"readiness":{"ready":true,"can_prepare":false,"preparation_needed":false,"waiting":[],"open_decisions":[],"problem":null}})).expect("fixture preview");
    let text = format!("{preview:#?}");
    assert!(
        !text.contains("synthetic-preview"),
        "preview token appeared in Debug"
    );
    assert!(text.contains("versions"));
    assert!(
        serde_json::to_string(&preview)
            .expect("preview wire")
            .contains("synthetic-preview-authority")
    );
}

#[test]
fn erasure_cancellation_answers_hide_the_confirmation_token() {
    let decision: crate::erase::Decision = serde_json::from_value(serde_json::json!({"id":"fixture-decision","paths":[],"not_before":"fixture-time","waiting":[],"completed":false,"membership_changed":false})).expect("fixture decision");
    let answer = crate::erase::Answer::CancelPreview {
        decision,
        token: "synthetic-cancel-authority".into(),
    };
    assert!(
        !format!("{answer:?}").contains("synthetic-cancel"),
        "cancellation token appeared in Debug"
    );
    assert!(
        serde_json::to_string(&answer)
            .expect("cancel wire")
            .contains("synthetic-cancel-authority")
    );
}
