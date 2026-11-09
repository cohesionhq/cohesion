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
        "cohesion://agent/3/4",
        "cohesion://article/1/2",
        "cohesion://contact/3",
        "https://example.com/task/3",
        "cohesion://task/99999999999999999999",
    ] {
        assert_eq!(parsed(text), Err(NotAnItemLink), "{text}");
    }
}
