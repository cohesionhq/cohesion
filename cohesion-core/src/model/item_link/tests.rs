use super::*;

fn parsed(link: &str) -> Result<ItemLink, NotAnItemLink> {
    link.parse()
}

#[test]
fn every_kind_reads_and_writes_back_as_it_was() {
    for link in [
        "cohesion://task/3/42",
        "cohesion://task/42",
        "cohesion://document/3/7",
        "cohesion://document/7",
        "cohesion://resource/3/9",
        "cohesion://document-folder/3/5",
        "cohesion://contact/3/12",
        "cohesion://agent/3",
        "cohesion://actor-session/143",
        "cohesion://article/88",
    ] {
        assert_eq!(parsed(link).unwrap().to_string(), link);
    }
}

#[test]
fn a_task_link_names_its_workspace_then_its_id() {
    assert_eq!(
        parsed("cohesion://task/3/42"),
        Ok(ItemLink::Task {
            workspace_id: Some(3),
            id: 42
        })
    );
}

#[test]
fn anything_else_is_not_an_item_link() {
    for text in [
        "",
        "cohesion://",
        "cohesion://task",
        "cohesion://task/",
        "cohesion://task/3/42/1",
        "cohesion://task/-3",
        "cohesion://task/+3",
        "cohesion://task/3x",
        "cohesion://resource/9",
        "cohesion://document-folder/5",
        "cohesion://document-folder/3/5/21",
        // Databases became document folders; their links name nothing now.
        "cohesion://database/3/5",
        "cohesion://database-row/3/5/21",
        "cohesion://agent/3/4",
        "cohesion://article/1/2",
        "cohesion://contact/3",
        "cohesion://contact/3/12/1",
        "https://example.com/task/3",
        "cohesion://task/99999999999999999999",
    ] {
        assert_eq!(parsed(text), Err(NotAnItemLink), "{text}");
    }
}

#[test]
fn a_document_folder_link_names_its_workspace_then_its_id() {
    assert_eq!(
        parsed("cohesion://document-folder/3/5"),
        Ok(ItemLink::DocumentFolder {
            workspace_id: 3,
            id: 5
        })
    );
}

/// The workspace a link names, when it names one.
fn workspace_of(link: &ItemLink) -> Option<i64> {
    match *link {
        ItemLink::Task { workspace_id, .. } | ItemLink::Document { workspace_id, .. } => {
            workspace_id
        }
        ItemLink::Resource { workspace_id, .. }
        | ItemLink::DocumentFolder { workspace_id, .. }
        | ItemLink::Contact { workspace_id, .. } => Some(workspace_id),
        ItemLink::Agent { .. } | ItemLink::ActorSession { .. } | ItemLink::Article { .. } => None,
    }
}

/// The cases the web UI runs too (`link_cases.json`): what a link names
/// here is what it must name there.
#[test]
fn the_shared_link_cases_hold() {
    let shared: serde_json::Value = serde_json::from_str(include_str!("link_cases.json")).unwrap();
    for case in shared["links"].as_array().unwrap() {
        let href = case["href"].as_str().unwrap();
        let names = match parsed(href) {
            Err(NotAnItemLink) => serde_json::Value::Null,
            Ok(link) => serde_json::json!({
                "kind": link.target().kind,
                "workspace_id": workspace_of(&link),
                "id": link.target().id,
            }),
        };
        assert_eq!(names, case["names"], "{href}");
    }
}
