use super::*;

const DOC: LinkTarget = LinkTarget {
    kind: LinkKind::Document,
    id: 7,
};

fn target(kind: LinkKind, id: i64) -> LinkTarget {
    LinkTarget { kind, id }
}

fn written(text: &str) -> Vec<&str> {
    item_links(text).into_iter().map(|(_, link)| link).collect()
}

#[test]
fn every_way_markdown_writes_a_link_is_one() {
    let text = "[Fix](cohesion://task/3/42)\n\
                See [Plan](cohesion://document/3/8 \"the plan\") today.\n\
                - <cohesion://agent/2>\n\
                [notes]: cohesion://resource/3/9";
    assert_eq!(
        written(text),
        [
            "cohesion://task/3/42",
            "cohesion://document/3/8",
            "cohesion://agent/2",
            "cohesion://resource/3/9"
        ]
    );
}

#[test]
fn a_link_in_code_or_not_to_an_item_is_none() {
    let text = "```\n[Fix](cohesion://task/3/42)\n```\n\
                `[Fix](cohesion://task/3/43)` and ``[a](cohesion://task/3/44)``\n\
                [Odd](cohesion://task/x) and cohesion://task/3/45 bare\n\
                a lone ` then [Kept](cohesion://task/3/46)";
    assert_eq!(written(text), ["cohesion://task/3/46"]);
}

#[test]
fn outgoing_links_name_each_item_once_and_never_the_source() {
    let text = "[Fix](cohesion://task/3/42) and [again](cohesion://task/42)\n\
                [Me](cohesion://document/3/7)\n\
                [Plan](cohesion://document/8)";
    assert_eq!(
        outgoing_links(DOC, text),
        [
            OutgoingLink {
                target: target(LinkKind::Task, 42),
                link: "cohesion://task/3/42".to_string()
            },
            OutgoingLink {
                target: target(LinkKind::Document, 8),
                link: "cohesion://document/8".to_string()
            }
        ]
    );
}

#[test]
fn a_task_and_a_document_of_the_same_id_are_different_targets() {
    let text = "[A](cohesion://task/3/7) [B](cohesion://document/3/8)";
    let targets: Vec<LinkTarget> = outgoing_links(DOC, text)
        .into_iter()
        .map(|link| link.target)
        .collect();
    assert_eq!(
        targets,
        [target(LinkKind::Task, 7), target(LinkKind::Document, 8)]
    );
}
